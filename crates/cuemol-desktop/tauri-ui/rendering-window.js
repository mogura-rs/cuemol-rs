"use strict";

const tauri = window.__TAURI__;
const invoke = tauri.core.invoke;
const dialog = tauri.dialog;
const appWindow = tauri.webviewWindow.getCurrentWebviewWindow();
const geometryKey = "cuemol.rendering-window.geometry.v1";
let geometryTimer, lifecycle = Promise.resolve();

function showWindowError(error) {
  let message = document.querySelector('.rendering-window-error');
  if (!message) {
    message = document.createElement('pre');
    message.className = 'rendering-window-error';
    document.body.append(message);
  }
  message.textContent = String(error);
  void invoke('render_window_request', {operation: 'show'}).catch(console.error);
}

function transition(action) {
  lifecycle = lifecycle.then(action).catch(showWindowError);
  return lifecycle;
}

async function chooseOutputPath(defaultPath, name, extensions) {
  return dialog.save({defaultPath, filters: [{name, extensions}]});
}

async function showConfirmationDialog({title, message, confirmLabel}) {
  return dialog.confirm(message, {title, kind: 'warning', okLabel: confirmLabel, cancelLabel: 'Cancel'});
}

async function saveGeometry() {
  clearTimeout(geometryTimer);
  const [size, position] = await Promise.all([appWindow.innerSize(), appWindow.outerPosition()]);
  localStorage.setItem(geometryKey, JSON.stringify({width: size.width, height: size.height, x: position.x, y: position.y}));
}

async function restoreGeometry() {
  let geometry;
  try { geometry = JSON.parse(localStorage.getItem(geometryKey) || 'null'); } catch { return; }
  if (!geometry) return;
  if (Number.isFinite(geometry.width) && Number.isFinite(geometry.height)) {
    await appWindow.setSize(new tauri.dpi.PhysicalSize(Math.min(8192, Math.max(480, geometry.width)), Math.min(8192, Math.max(400, geometry.height))));
  }
  if (Number.isFinite(geometry.x) && Number.isFinite(geometry.y)) {
    const monitors = await tauri.window.availableMonitors();
    const visible = monitors.some(({position, size}) => geometry.x + 100 > position.x && geometry.x < position.x + size.width - 100 && geometry.y >= position.y && geometry.y < position.y + size.height - 40);
    if (visible) await appWindow.setPosition(new tauri.dpi.PhysicalPosition(geometry.x, geometry.y));
  }
}

async function showRendering() {
  const preferences = await invoke('get_preferences');
  document.documentElement.dataset.theme = preferences.darkMode ? 'dark' : 'light';
  await CueMolRendering.open();
  await invoke('render_window_request', {operation: 'show'});
}

async function startRenderingWindow() {
  await CueMolApplicationMenu.start({action: async id => {
    if (id === 'close-tab') await transition(() => CueMolRendering.close());
    else if (id === 'undo' || id === 'redo') await CueMolRendering.historyAction(id);
  }});
  CueMolRendering.onClose = async () => {
    try { await saveGeometry(); } catch (error) { console.warn('Cannot save Rendering window geometry:', error); }
    await invoke('render_window_request', {operation: 'hide'});
  };
  await tauri.event.listen('cuemol-render-open', () => transition(showRendering));
  await tauri.event.listen('cuemol-render-close', () => transition(() => CueMolRendering.close()));
  window.addEventListener('storage', event => {
    if (event.key !== 'cuemol3.shell.v1') return;
    try {
      const theme = JSON.parse(event.newValue)?.theme;
      if (theme === 'light' || theme === 'dark') document.documentElement.dataset.theme = theme;
    } catch { /* Ignore invalid stored shell settings. */ }
  });
  await restoreGeometry();
  const saveSoon = () => { clearTimeout(geometryTimer); geometryTimer = setTimeout(() => void saveGeometry().catch(showWindowError), 200); };
  await appWindow.onMoved(saveSoon);
  await appWindow.onResized(saveSoon);
  document.addEventListener('keydown', event => {
    if (!(event.ctrlKey || event.metaKey) || event.altKey || event.isComposing) return;
    const key = event.key.toLowerCase();
    if (key === 'w') {
      event.preventDefault();event.stopPropagation();
      void transition(() => CueMolRendering.close());
    } else if (key === 'q') {
      event.preventDefault();event.stopPropagation();
      void invoke('render_window_request', {operation: 'quit'}).catch(showWindowError);
    }
  }, true);
  await transition(showRendering);
}

void startRenderingWindow().catch(showWindowError);

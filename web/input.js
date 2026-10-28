export function installViewInput(canvas, send, status) {
  const isMac = /Mac/.test(navigator.platform);
  let pressed = false;
  const inside = event => {
    const ui = status().ui;
    for (const r of ui?.overlays ?? []) {
      if (event.clientX >= r[0] && event.clientX <= r[0]+r[2] && event.clientY >= r[1] && event.clientY <= r[1]+r[3]) return false;
    }
    const rects = ui?.rects;
    for (const name of ['Navigate', 'Rect Select', 'Measure selection', 'Fit scene']) {
      const r = rects?.[name];
      if (r && event.clientX >= r[0] && event.clientX <= r[0]+r[2] && event.clientY >= r[1] && event.clientY <= r[1]+r[3]) return false;
    }
    const rect = rects?.Viewport;
    return rect && event.clientX >= rect[0] && event.clientX <= rect[0] + rect[2]
      && event.clientY >= rect[1] && event.clientY <= rect[1] + rect[3];
  };
  const keys = (event, alt = false) => (event.shiftKey ? 1 : 0) | (event.ctrlKey ? 2 : 0) | (alt && event.altKey ? 4 : 0);
  const buttons = value => ((value & 1) ? 8 : 0) | ((value & 4) ? 16 : 0) | ((value & 2) ? 32 : 0);
  const emit = (event, kind, extra = {}) => send(JSON.stringify({kind, time: event.timeStamp,
    x: event.clientX, y: event.clientY, modifiers: keys(event) | buttons(event.buttons),
    button: [8, 16, 32][event.button] ?? 0, isMac, ...extra}));
  canvas.addEventListener('pointerdown', event => {
    if (!inside(event)) return;
    pressed = true;
    emit(event, 'down');
  }, true);
  document.addEventListener('pointermove', event => { if (pressed) emit(event, 'move'); }, true);
  document.addEventListener('pointerup', event => {
    if (!pressed) return;
    emit(event, 'up');
    pressed = event.buttons !== 0;
  }, true);
  const cancel = () => { pressed = false; send(JSON.stringify({kind:'cancel'})); };
  document.addEventListener('pointercancel', cancel, true);
  window.addEventListener('blur', cancel);
  canvas.addEventListener('wheel', event => {
    if (!inside(event)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.ctrlKey) emit(event, 'gesture', {axis:6, delta:event.deltaY, modifiers:keys(event,true) & ~2});
    else emit(event, 'wheel', {dx:event.deltaX, dy:event.deltaY, mode:event.deltaMode, modifiers:keys(event,true)});
  }, {capture:true, passive:false});
  let scale = 1;
  let rotation = 0;
  canvas.addEventListener('gesturestart', event => {
    if (!inside(event)) return;
    event.preventDefault(); scale = event.scale; rotation = event.rotation;
  }, {capture:true, passive:false});
  canvas.addEventListener('gesturechange', event => {
    if (!inside(event)) return;
    event.preventDefault();
    emit(event, 'gesture', {axis:6, delta:-100 * Math.log(event.scale / scale), modifiers:keys(event,true)});
    emit(event, 'gesture', {axis:7, delta:event.rotation - rotation, modifiers:keys(event,true)});
    scale = event.scale; rotation = event.rotation;
  }, {capture:true, passive:false});
}

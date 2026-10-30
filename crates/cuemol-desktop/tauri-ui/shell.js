"use strict";

let cueMolActivity = "explorer";
let cueMolInspectorTarget = "";
let cueMolInspectorClosed = false;
let cueMolPaletteCollapsed = false;
const CUE_MOL_SHELL_KEY = "cuemol3.shell.v1";
const cueMolIcons = {
  explorer: '<path d="M3 6h7l2 3h9v11H3z"/><path d="M3 6V3h7l2 3h8v3"/>',
  view: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  selection: '<path d="M4 9V4h5m6 0h5v5m0 6v5h-5M9 20H4v-5"/><path d="m9 9 8 3-4 1-1 4z"/>',
  crystal: '<path d="m12 2 9 5v10l-9 5-9-5V7zM3 7l9 5 9-5M12 12v10M12 2v10"/>',
  actions: '<path d="m14 2-9 12h6l-1 8 9-12h-6z"/>',
  settings: '<path d="m9 3-1 3-3 1 1 3-2 2 2 2-1 3 3 1 1 3h6l1-3 3-1-1-3 2-2-2-2 1-3-3-1-1-3z"/><circle cx="12" cy="12" r="3"/>',
  new: '<path d="M6 3h8l4 4v14H6zM14 3v5h4M9 14h6m-3-3v6"/>',
  open: '<path d="M3 7V4h7l2 3h8v4M3 7v13h15l4-9H7z"/>',
  save: '<path d="M4 3h14l3 3v15H3V3zM7 3v6h10V3M7 21v-8h10v8"/>',
  reload: '<path d="M20 10a8 8 0 1 0-1 8M20 3v7h-7"/>',
  undo: '<path d="M8 5 3 10l5 5M3 10h11a6 6 0 0 1 0 12"/>',
  redo: '<path d="m16 5 5 5-5 5M21 10H10a6 6 0 0 0 0 12"/>',
  download: '<path d="M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4"/>',
  render: '<path d="M3 8h5l2-4h4l2 4h5v13H3z"/><circle cx="12" cy="14" r="4"/>',
  theme: '<circle cx="12" cy="12" r="8"/><path d="M12 4v16"/><path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor"/>',
  fit: '<path d="M3 9V3h6m6 0h6v6m0 6v6h-6M9 21H3v-6"/><circle cx="12" cy="12" r="3"/>',
  rotate: '<path d="M20 10a8 8 0 1 0-1 8M20 3v7h-7"/>',
  rect: '<path stroke-dasharray="3 2" d="M3 3h18v18H3z"/>',
  distance: '<path d="m3 17 14-14 4 4L7 21zM7 13l4 4m0-8 4 4"/>',
  angle: '<path d="m5 3 1 16h15M7 13a6 6 0 0 1 6 6"/>',
  torsion: '<path d="m3 17 6-9 6 8 6-9"/><circle cx="9" cy="8" r="2"/><circle cx="15" cy="16" r="2"/>',
};

function cueMolIcon(name) {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${cueMolIcons[name] || cueMolIcons.actions}</svg>`;
}

function saveCueMolShell() {
  try {
    localStorage.setItem(CUE_MOL_SHELL_KEY, JSON.stringify({
      activity: cueMolActivity,
      theme: document.documentElement.dataset.theme,
      inspectorWidth: contentHolder.style.getPropertyValue("--inspector-width"),
      paletteCollapsed: cueMolPaletteCollapsed,
    }));
  } catch {
    // Storage can be unavailable in restricted webviews.
  }
}

function setCueMolPaletteCollapsed(collapsed, save = true) {
  cueMolPaletteCollapsed = Boolean(collapsed);
  document.querySelector('.viewport-palette').classList.toggle('collapsed', cueMolPaletteCollapsed);
  const cap = document.querySelector('.viewport-palette [data-toggle-palette]');
  cap.textContent = cueMolPaletteCollapsed ? '›' : '⌄';
  cap.setAttribute('aria-label', cueMolPaletteCollapsed ? 'Show tool palette' : 'Hide tool palette');
  cap.setAttribute('aria-expanded', String(!cueMolPaletteCollapsed));
  document.querySelector('.menu [data-toggle-palette]').classList.toggle('checked', !cueMolPaletteCollapsed);
  if (save) saveCueMolShell();
}

function selectCueMolActivity(name, toggle = true) {
  cueMolActivity = toggle && cueMolActivity === name ? null : name;
  document.querySelectorAll("[data-activity]").forEach((item) => {
    const active = item.dataset.activity === cueMolActivity;
    item.classList.toggle("active", active);
    item.setAttribute("aria-pressed", String(active));
  });
  document.querySelectorAll("[data-activity-panel]").forEach((panel) => {
    const active = panel.dataset.activityPanel === cueMolActivity;
    panel.classList.toggle("activity-inactive", !active);
    if (active && panel.hasAttribute("data-side-card")) {
      panel.hidden = false;
      const title = sidePanelCardName(panel);
      if (snapshot && panel.classList.contains("open")) renderSidePanel(title);
    }
  });
  document.getElementById("activity-title").textContent = cueMolActivity
    ? cueMolActivity[0].toUpperCase() + cueMolActivity.slice(1) : "Explorer";
  if (cueMolActivity && sidePanelWidth === 0) setSidePanelWidth(expandedSidePanelWidth);
  syncSidePanelVisibility();
  saveCueMolShell();
  if (cueMolActivity === "actions") refreshActionsPane(true);
  if (snapshot) scheduleRender(0);
}

function setCueMolInspectorWidth(value) {
  const maximum = Math.max(200, Math.min(600, contentHolder.clientWidth - 480));
  const width = Math.min(maximum, Math.max(200, value));
  contentHolder.style.setProperty("--inspector-width", `${width}px`);
  const splitter = document.querySelector(".inspector-splitter");
  splitter.setAttribute("aria-valuenow", String(Math.round(width)));
  splitter.setAttribute("aria-valuemin", "200");
  splitter.setAttribute("aria-valuemax", String(Math.round(maximum)));
}

function renderTritiumInspector(owner, camera, style) {
  if (cueMolActivity === "actions" && renderActionInspector()) return;
  const rendererUid = workspaceSelectionKind === "densityRenderer" ? selectedDensityRendererUid
    : workspaceSelectionKind === "surfaceRenderer" ? selectedSurfaceRendererUid
    : workspaceSelectionKind === "lwRenderer" ? selectedLwRendererUid
    : ["renderer", "rendererGroup"].includes(workspaceSelectionKind) ? selectedRendererUid : null;
  const target = owner?.renderers?.find((item) => item.uid === rendererUid) || owner || camera || style;
  const key = target ? `${workspaceSelectionKind}:${target.uid ?? target.name}` : "";
  if (key !== cueMolInspectorTarget) {
    cueMolInspectorTarget = key;
    cueMolInspectorClosed = false;
  }
  contentHolder.classList.toggle("has-inspector", Boolean(target) && !cueMolInspectorClosed);
  document.getElementById("inspector-title").textContent = target?.name || "No object";
  const fields = document.getElementById("inspector-fields");
  fields.replaceChildren();
  if (!target) return;
  const coordinates = selectedWorkspaceRenameCoordinates();
  if (coordinates && !["camera", "style"].includes(workspaceSelectionKind)) {
    const label = document.createElement("label");
    label.textContent = "Name";
    const input = document.createElement("input");
    input.value = target.name || "";
    input.setAttribute("aria-label", "Inspected name");
    input.addEventListener("change", () => void mutateWithoutViewport("rename_workspace_node", {
      input: { kind: workspaceSelectionKind, ...coordinates, name: input.value },
    }));
    label.append(input);
    fields.append(label);
    if (typeof target.visible === "boolean") {
      const visibleLabel = document.createElement("label");
      visibleLabel.className = "inspector-check";
      const visible = document.createElement("input");
      visible.type = "checkbox";
      visible.checked = target.visible;
      visible.disabled = target.locked;
      visible.addEventListener("change", () => void mutate("set_workspace_visible", {
        target: { kind: workspaceSelectionKind, ...coordinates }, visible: visible.checked,
      }));
      visibleLabel.append(visible, "Visible");
      fields.append(visibleLabel);
    }
  }
}

function renderTritiumShell() {
  const scene = snapshot?.scenes.find((item) => item.active);
  const title = scene?.name ? `${scene.modified ? "● " : ""}${scene.name} — cuemol-rs` : "cuemol-rs";
  if (document.title !== title) {
    document.title = title;
    const current = tauri?.webviewWindow?.getCurrentWebviewWindow?.();
    if (current?.setTitle) void current.setTitle(title).catch(() => {});
  }
  const cameras = document.getElementById("camera-list");
  cameras.replaceChildren();
  const available = (snapshot?.cameras || []).filter((camera) => camera.sceneUid === scene?.uid);
  for (const camera of available) {
    cameras.append(button(camera.name, "camera-item", () => void mutate("load_camera", { name: camera.name })));
  }
  if (!available.length) {
    const empty = document.createElement("p");
    empty.textContent = "No saved cameras";
    cameras.append(empty);
  }
}

function setupTritiumShell() {
  let stored = {};
  try { stored = JSON.parse(localStorage.getItem(CUE_MOL_SHELL_KEY) || "{}"); } catch { /* Use defaults. */ }
  if (!stored || typeof stored !== "object" || Array.isArray(stored)) stored = {};
  document.documentElement.dataset.theme = stored.theme === "light" ? "light" : "dark";
  setCueMolPaletteCollapsed(stored.paletteCollapsed === true, false);
  for (const item of document.querySelectorAll('[data-toggle-palette]')) {
    item.addEventListener('click', event => {
      event.stopPropagation();
      setCueMolPaletteCollapsed(!cueMolPaletteCollapsed);
      item.closest('details')?.removeAttribute('open');
    });
  }
  if (Number.isFinite(parseFloat(stored.inspectorWidth))) setCueMolInspectorWidth(parseFloat(stored.inspectorWidth));
  for (const item of document.querySelectorAll("[data-icon]")) item.innerHTML = cueMolIcon(item.dataset.icon);
  const iconActions = { "new-scene": "new", open: "open", save: "save", "open-scene": "open", undo: "undo", redo: "redo" };
  for (const item of document.querySelectorAll(".tool-button")) {
    const icon = item.querySelector(".tool-icon");
    const label = item.querySelector("span:last-child")?.textContent.trim() || "";
    const name = iconActions[item.dataset.action] || (item.dataset.localAction === "reload-scene" ? "reload" : null)
      || ({ rotate: "rotate", "rect-select": "rect" })[item.dataset.interaction]
      || ({ Distance: "distance", Angle: "angle", Torsion: "torsion", "Save As": "save" })[label];
    if (icon && name) icon.innerHTML = cueMolIcon(name);
    if (label) { item.title = label; item.setAttribute("aria-label", label); }
  }
  for (const [name, label] of [["Workspace", "Scene"], ["Coloring", "Color"], ["MolStruct", "Molecule"]]) {
    sidePanelCard(name).querySelector(".side-title span:nth-child(2)").textContent = label;
  }
  document.querySelectorAll("[data-activity]").forEach((item) => {
    item.addEventListener("click", () => selectCueMolActivity(item.dataset.activity));
  });
  document.querySelector('[data-shell-action="theme"]').addEventListener("click", () => {
    document.documentElement.dataset.theme = document.documentElement.dataset.theme === "light" ? "dark" : "light";
    saveCueMolShell();
  });
  document.querySelector('[data-shell-action="close-inspector"]').addEventListener("click", () => {
    cueMolInspectorClosed = true;
    contentHolder.classList.remove("has-inspector");
  });
  document.getElementById("inspector-properties").addEventListener("click", () => workspaceProperties.click());
  const splitter = document.querySelector(".inspector-splitter");
  splitter.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const initial = document.querySelector(".inspector-panel").clientWidth;
    const startX = event.clientX;
    splitter.setPointerCapture(event.pointerId);
    const move = (next) => setCueMolInspectorWidth(initial + startX - next.clientX);
    const finish = () => {
      splitter.removeEventListener("pointermove", move);
      splitter.removeEventListener("pointerup", finish);
      splitter.removeEventListener("pointercancel", finish);
      saveCueMolShell();
    };
    splitter.addEventListener("pointermove", move);
    splitter.addEventListener("pointerup", finish);
    splitter.addEventListener("pointercancel", finish);
  });
  splitter.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault();
    setCueMolInspectorWidth(document.querySelector(".inspector-panel").clientWidth + (event.key === "ArrowLeft" ? 10 : -10));
    saveCueMolShell();
  });
  const savedActivity = ["explorer", "view", "selection", "crystal", "actions", null].includes(stored.activity) ? stored.activity : "explorer";
  selectCueMolActivity(sidePanelWidth === 0 ? null : savedActivity, false);
}

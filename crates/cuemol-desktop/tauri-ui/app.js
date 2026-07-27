"use strict";

const appShell = document.querySelector(".app-shell");
const viewport = document.getElementById("viewport");
const viewportImage = document.getElementById("viewport-image");
const sceneTabs = document.getElementById("scene-tabs");
const sceneTree = document.getElementById("scene-tree");
const rendererList = document.getElementById("renderer-list");
const selectionInput = document.getElementById("selection-input");
const statusText = document.getElementById("status-text");
const statusHistory = document.getElementById("status-history");
const metrics = document.getElementById("scene-metrics");
const projectionReadout = document.getElementById("projection-readout");
const inspectorTitle = document.getElementById("inspector-title");
const objectStats = document.getElementById("object-stats");
const renderSize = document.getElementById("render-size");
const dragReadout = document.getElementById("drag-readout");
const toast = document.getElementById("toast");

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const dialog = tauri?.dialog;

let snapshot = null;
let selectedObjectUid = null;
let renderTimer = null;
let wheelTimer = null;
let toastTimer = null;
let requestSequence = 0;
let drag = null;
let statusTrail = [];

function setBusy(value) {
  appShell.classList.toggle("busy", value);
}

function showError(error) {
  const message = typeof error === "string" ? error : error?.message || String(error);
  toast.textContent = message;
  toast.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove("show"), 4200);
}

function recordStatus(message) {
  if (!message || statusTrail.at(-1) === message) return;
  statusTrail.push(message);
  statusTrail = statusTrail.slice(-4);
  statusHistory.textContent = statusTrail.slice(0, -1).join("  /  ") || "CueMol2 Rust core";
}

function button(label, className, onClick) {
  const element = document.createElement("button");
  element.type = "button";
  element.textContent = label;
  if (className) element.className = className;
  element.addEventListener("click", onClick);
  return element;
}

function activeObjects() {
  if (!snapshot) return [];
  return snapshot.objects.filter((object) => object.sceneUid === snapshot.activeSceneUid);
}

function selectedObject() {
  return activeObjects().find((object) => object.uid === selectedObjectUid) || null;
}

function normalizeSelection() {
  const objects = activeObjects();
  if (!objects.some((object) => object.uid === selectedObjectUid)) {
    selectedObjectUid = objects[0]?.uid ?? null;
  }
}

function renderSceneTabs() {
  sceneTabs.replaceChildren();
  for (const scene of snapshot.scenes) {
    const tab = button(scene.name || `Scene ${scene.uid}`, `scene-tab${scene.active ? " active" : ""}`, () => {
      if (!scene.active) mutate("activate_scene", { sceneUid: scene.uid });
    });
    tab.title = `${scene.objectCount} objects / ${scene.mapCount} maps`;
    sceneTabs.append(tab);
  }
}

function renderSceneTree() {
  sceneTree.replaceChildren();
  if (snapshot.scenes.length === 0) {
    const empty = document.createElement("div");
    empty.className = "tree-empty";
    empty.textContent = "No scenes";
    sceneTree.append(empty);
    return;
  }
  for (const scene of snapshot.scenes) {
    const sceneRow = document.createElement("div");
    sceneRow.className = "tree-scene";
    sceneRow.textContent = scene.name || `Scene ${scene.uid}`;
    sceneTree.append(sceneRow);
    for (const object of snapshot.objects.filter((item) => item.sceneUid === scene.uid)) {
      const objectRow = button(object.name, `tree-object${object.uid === selectedObjectUid ? " active" : ""}`, () => {
        selectedObjectUid = object.uid;
        renderShell();
      });
      const count = document.createElement("small");
      count.textContent = `${object.atomCount} AT`;
      objectRow.append(count);
      sceneTree.append(objectRow);
      for (const renderer of object.renderers) {
        const rendererRow = document.createElement("div");
        rendererRow.className = "tree-renderer";
        rendererRow.textContent = `${renderer.rendererType} · ${renderer.name}`;
        sceneTree.append(rendererRow);
      }
    }
  }
}

function renderInspector() {
  const object = selectedObject();
  inspectorTitle.textContent = object?.name || "No object";
  const values = object
    ? [object.atomCount, object.residueCount, object.bondCount]
    : ["—", "—", "—"];
  objectStats.querySelectorAll("dd").forEach((node, index) => {
    node.textContent = String(values[index]);
  });
  rendererList.replaceChildren();
  if (!object) {
    const empty = document.createElement("div");
    empty.className = "renderer-empty";
    empty.textContent = "Select a molecular object.";
    rendererList.append(empty);
    return;
  }
  if (object.renderers.length === 0) {
    const empty = document.createElement("div");
    empty.className = "renderer-empty";
    empty.textContent = "No render representations.";
    rendererList.append(empty);
  }
  for (const renderer of object.renderers) {
    const row = document.createElement("label");
    row.className = "renderer-row";
    const visible = document.createElement("input");
    visible.type = "checkbox";
    visible.checked = renderer.visible;
    visible.addEventListener("change", () =>
      mutate("set_renderer_visible", {
        objectUid: object.uid,
        rendererUid: renderer.uid,
        visible: visible.checked,
      }),
    );
    const description = document.createElement("span");
    const name = document.createElement("strong");
    name.textContent = renderer.name;
    const type = document.createElement("small");
    type.textContent = renderer.rendererType;
    description.append(name, type);
    row.append(visible, description);
    rendererList.append(row);
  }
}

function renderShell() {
  if (!snapshot) return;
  normalizeSelection();
  selectionInput.value = snapshot.selection;
  projectionReadout.textContent = snapshot.perspective ? "PERSPECTIVE" : "ORTHO";
  statusText.textContent = snapshot.status;
  recordStatus(snapshot.status);
  metrics.textContent = `${snapshot.scenes.length} SCENES / ${snapshot.objects.length} OBJECTS`;
  renderSceneTabs();
  renderSceneTree();
  renderInspector();
}

function viewportDimensions() {
  const bounds = viewport.getBoundingClientRect();
  return {
    width: Math.max(64, Math.round(bounds.width)),
    height: Math.max(64, Math.round(bounds.height)),
  };
}

function applyViewport(result, sequence) {
  if (sequence !== requestSequence) return;
  viewport.classList.toggle("has-image", Boolean(result.image));
  if (result.image) viewportImage.src = result.image;
  renderSize.textContent = `${result.width} × ${result.height}`;
  if (result.status) {
    statusText.textContent = result.status;
    recordStatus(result.status);
  }
}

async function renderViewport() {
  if (!invoke) return;
  const size = viewportDimensions();
  const sequence = ++requestSequence;
  try {
    const result = await invoke("render_viewport", size);
    applyViewport(result, sequence);
  } catch (error) {
    showError(error);
  }
}

function scheduleRender(delay = 100) {
  clearTimeout(renderTimer);
  renderTimer = setTimeout(renderViewport, delay);
}

async function mutate(command, args = {}) {
  if (!invoke) {
    showError("Tauri IPC is unavailable. Run this frontend through the Tauri binary.");
    return;
  }
  setBusy(true);
  try {
    snapshot = await invoke(command, args);
    renderShell();
    await renderViewport();
  } catch (error) {
    showError(error);
  } finally {
    setBusy(false);
  }
}

async function cameraCommand(action, x = 0, y = 0, refreshSnapshot = false) {
  if (!invoke) return;
  const size = viewportDimensions();
  const sequence = ++requestSequence;
  try {
    const result = await invoke("update_camera", { action, x, y, ...size });
    applyViewport(result, sequence);
    if (refreshSnapshot) {
      snapshot = await invoke("get_snapshot");
      renderShell();
    }
  } catch (error) {
    showError(error);
  }
}

async function openFiles() {
  if (!dialog?.open) {
    showError("The Tauri dialog plugin is unavailable.");
    return;
  }
  const chosen = await dialog.open({
    multiple: true,
    directory: false,
    filters: [
      {
        name: "CueMol structures and scenes",
        extensions: ["pdb", "ent", "cif", "mmcif", "qsc", "qdfpdb", "pse", "sdf", "mol2", "gro", "dx", "map", "mrc", "ccp4", "obj", "ply"],
      },
    ],
  });
  if (!chosen) return;
  const paths = Array.isArray(chosen) ? chosen : [chosen];
  await mutate("open_paths", { paths });
}

async function saveScene() {
  const path = await dialog?.save?.({
    defaultPath: "scene.qsc",
    filters: [{ name: "CueMol scene", extensions: ["qsc"] }],
  });
  if (path) await mutate("save_scene", { path });
}

async function exportPng() {
  const path = await dialog?.save?.({
    defaultPath: "cuemol.png",
    filters: [{ name: "PNG image", extensions: ["png"] }],
  });
  if (!path) return;
  await mutate("export_png", { path, ...viewportDimensions() });
}

async function handleAction(action) {
  switch (action) {
    case "open":
      await openFiles();
      break;
    case "save":
      await saveScene();
      break;
    case "export":
      await exportPng();
      break;
    case "new-scene":
      await mutate("new_scene", { name: null });
      break;
    case "close-scene":
      await mutate("close_active_scene");
      break;
    case "fit":
      await cameraCommand("fit");
      break;
    case "projection":
      await cameraCommand("projection", 0, 0, true);
      break;
    case "select":
      await mutate("set_selection", { selection: selectionInput.value });
      break;
    case "clear-selection":
      await mutate("set_selection", { selection: "" });
      break;
    case "add-renderer": {
      const object = selectedObject();
      if (!object) {
        showError("Select a molecular object first.");
        return;
      }
      const rendererType = document.getElementById("renderer-type").value;
      await mutate("add_renderer", { objectUid: object.uid, rendererType });
      break;
    }
  }
}

document.querySelectorAll("[data-action]").forEach((element) => {
  element.addEventListener("click", () => handleAction(element.dataset.action));
});

selectionInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") handleAction("select");
});

viewport.addEventListener("contextmenu", (event) => event.preventDefault());
viewport.addEventListener("pointerdown", (event) => {
  if (event.button !== 0 && event.button !== 2) return;
  viewport.setPointerCapture(event.pointerId);
  drag = {
    pointerId: event.pointerId,
    x: event.clientX,
    y: event.clientY,
    dx: 0,
    dy: 0,
    action: event.button === 2 || event.shiftKey ? "pan" : "rotate",
  };
  viewport.classList.add("dragging");
  dragReadout.textContent = drag.action.toUpperCase();
});

viewport.addEventListener("pointermove", (event) => {
  if (!drag || drag.pointerId !== event.pointerId) return;
  drag.dx = event.clientX - drag.x;
  drag.dy = event.clientY - drag.y;
  dragReadout.textContent = `${drag.action.toUpperCase()}  ${Math.round(drag.dx)}, ${Math.round(drag.dy)}`;
});

function finishDrag(event) {
  if (!drag || drag.pointerId !== event.pointerId) return;
  const completed = drag;
  drag = null;
  viewport.classList.remove("dragging");
  dragReadout.textContent = "";
  if (Math.abs(completed.dx) + Math.abs(completed.dy) < 2) return;
  const scale = completed.action === "rotate" ? 0.35 : 1;
  cameraCommand(completed.action, completed.dx * scale, completed.dy * scale);
}

viewport.addEventListener("pointerup", finishDrag);
viewport.addEventListener("pointercancel", finishDrag);
viewport.addEventListener(
  "wheel",
  (event) => {
    event.preventDefault();
    clearTimeout(wheelTimer);
    const factor = Math.exp(Math.max(-500, Math.min(500, event.deltaY)) * 0.001);
    wheelTimer = setTimeout(() => cameraCommand("zoom", factor), 28);
  },
  { passive: false },
);

window.addEventListener("keydown", (event) => {
  const editing = ["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement?.tagName);
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "o") {
    event.preventDefault();
    openFiles();
  } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    saveScene();
  } else if (!editing && event.key.toLowerCase() === "f") {
    cameraCommand("fit");
  } else if (!editing && event.key.toLowerCase() === "p") {
    cameraCommand("projection", 0, 0, true);
  }
});

window.addEventListener("dragover", (event) => event.preventDefault());
window.addEventListener("drop", (event) => {
  event.preventDefault();
  const paths = [...event.dataTransfer.files].map((file) => file.path).filter(Boolean);
  if (paths.length) mutate("open_paths", { paths });
});

new ResizeObserver(() => scheduleRender(140)).observe(viewport);

async function start() {
  if (!invoke) {
    statusText.textContent = "Tauri IPC unavailable";
    showError("Run cargo run -p cuemol-desktop --bin cuemol-desktop-tauri --features tauri-gui");
    return;
  }
  try {
    snapshot = await invoke("get_snapshot");
    renderShell();
    await renderViewport();
  } catch (error) {
    showError(error);
  }
}

start();

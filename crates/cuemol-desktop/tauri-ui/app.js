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
const measureReadout = document.getElementById("measure-readout");
const molstructObject = document.getElementById("molstruct-object");
const molstructTree = document.getElementById("molstruct-tree");
const coloringObject = document.getElementById("coloring-object");
const coloringType = document.getElementById("coloring-type");
const coloringNote = document.getElementById("coloring-note");
const symmetryContent = document.getElementById("symmetry-content");
const showUnitCell = document.getElementById("show-unit-cell");
const showSymmetryMates = document.getElementById("show-symmetry-mates");
const densityContent = document.getElementById("density-content");
const showDensityContour = document.getElementById("show-density-contour");
const densityLevel = document.getElementById("density-level");
const densityLevelValue = document.getElementById("density-level-value");
const sequenceView = document.getElementById("sequence-view");
const referenceDialog = document.getElementById("reference-dialog");
const dialogTitle = document.getElementById("dialog-title");
const dialogContent = document.getElementById("dialog-content");
const centerMarkElement = document.querySelector(".center-mark");
const selectionRectangle = document.querySelector(".selection-rectangle");
const animationPlay = document.getElementById("animation-play");
const animationStop = document.getElementById("animation-stop");
const animationLoop = document.getElementById("animation-loop");
const animationSlider = document.getElementById("animation-slider");
const animationTime = document.getElementById("animation-time");
const animationDuration = document.getElementById("animation-duration");
const animationList = document.getElementById("animation-list");
const animationKind = document.getElementById("animation-kind");
const animationAdd = document.getElementById("animation-add");
const animationDelete = document.getElementById("animation-delete");
const animationChange = document.getElementById("animation-change");
const animationUp = document.getElementById("animation-up");
const animationDown = document.getElementById("animation-down");

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
let lastOpenedPaths = [];
let centerMark = "cross";
let performanceMeasure = false;
let interactionMode = "rotate";
let selectedAnimationIndex = null;
let animationTimer = null;
let animationTickBusy = false;

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
  statusTrail = statusTrail.slice(-80);
  statusHistory.textContent = statusTrail.join("\n") || "CueMol2 Rust core";
  statusHistory.scrollTop = statusHistory.scrollHeight;
}

function closeMenus(except = null) {
  document.querySelectorAll(".menu[open]").forEach((menu) => {
    if (menu !== except) menu.removeAttribute("open");
  });
}

function showBottomPanel(name) {
  document.querySelectorAll("[data-bottom-select]").forEach((tab) => {
    tab.classList.toggle("active", tab.dataset.bottomSelect === name);
  });
  document.querySelectorAll("[data-bottom-panel]").forEach((panel) => {
    panel.classList.toggle("active", panel.dataset.bottomPanel === name);
  });
}

function dialogParagraph(text) {
  const paragraph = document.createElement("p");
  paragraph.textContent = text;
  return paragraph;
}

function dialogField(label, value, type = "text") {
  const wrapper = document.createElement("label");
  wrapper.className = "dialog-field";
  const text = document.createElement("span");
  text.textContent = label;
  const input = document.createElement("input");
  input.type = type;
  input.value = value;
  wrapper.append(text, input);
  dialogContent.append(wrapper);
  return input;
}

function showReferenceDialog(title) {
  dialogTitle.textContent = title;
  dialogContent.replaceChildren();
  if (title === "About CueMol2") {
    const heading = document.createElement("h2");
    heading.textContent = "CueMol2";
    dialogContent.append(
      heading,
      dialogParagraph("Molecular visualization system — Rust desktop implementation."),
    );
  } else if (title === "Scene Properties") {
    const scene = snapshot?.scenes.find((item) => item.active);
    const properties = document.createElement("dl");
    properties.className = "dialog-properties";
    for (const [term, value] of [
      ["Name", scene?.name || "(none)"],
      ["UID", scene?.uid ?? "—"],
      ["Objects", scene?.objectCount ?? 0],
      ["Maps", scene?.mapCount ?? 0],
    ]) {
      const dt = document.createElement("dt");
      dt.textContent = term;
      const dd = document.createElement("dd");
      dd.textContent = String(value);
      properties.append(dt, dd);
    }
    dialogContent.append(properties);
  } else if (title === "View Property") {
    dialogContent.append(
      dialogParagraph(`Projection: ${snapshot?.perspective ? "Perspective" : "Orthographic"}`),
      dialogParagraph(`Background: ${snapshot?.background || "black"}`),
      dialogParagraph(`Center mark: ${centerMark}`),
    );
  } else if (title === "Delete mol atoms") {
    const object = selectedObject();
    if (!object) {
      dialogContent.append(dialogParagraph("Select a molecular object first."));
    } else {
      const selection = dialogField("Selection", snapshot.selection);
      const apply = button("Delete", "dialog-primary", async () => {
        await mutate("delete_atoms", {
          objectUid: object.uid,
          selection: selection.value,
        });
        referenceDialog.close();
      });
      dialogContent.append(apply);
    }
  } else if (title === "Change chain ID") {
    const object = selectedObject();
    if (!object) {
      dialogContent.append(dialogParagraph("Select a molecular object first."));
    } else {
      const selection = dialogField("Selection", snapshot.selection);
      const chainName = dialogField("New chain ID", "A");
      const apply = button("Apply", "dialog-primary", async () => {
        await mutate("change_chain", {
          objectUid: object.uid,
          selection: selection.value,
          chainName: chainName.value,
        });
        referenceDialog.close();
      });
      dialogContent.append(apply);
    }
  } else if (title === "Change residue number") {
    const object = selectedObject();
    if (!object) {
      dialogContent.append(dialogParagraph("Select a molecular object first."));
    } else {
      const selection = dialogField("Selection", snapshot.selection);
      const start = dialogField("Start residue number", "1", "number");
      const apply = button("Apply", "dialog-primary", async () => {
        await mutate("renumber_residues", {
          objectUid: object.uid,
          selection: selection.value,
          start: Number(start.value),
        });
        referenceDialog.close();
      });
      dialogContent.append(apply);
    }
  } else if (title === "Animation object properties") {
    const object = snapshot?.animationObjects.find(
      (item) => item.index === selectedAnimationIndex,
    );
    if (!object) {
      dialogContent.append(dialogParagraph("Select an animation object first."));
    } else {
      dialogContent.append(dialogParagraph(object.name));
      const duration = dialogField("Duration (seconds)", String(object.duration), "number");
      duration.min = "0.1";
      duration.step = "0.1";
      const apply = button("Apply", "dialog-primary", async () => {
        await mutate("set_animation_duration", {
          index: object.index,
          duration: Number(duration.value),
        });
        referenceDialog.close();
      });
      dialogContent.append(apply);
    }
  } else if (title === "Open PDB") {
    const pdbId = dialogField("PDB ID", "1CRN");
    const open = button("Open RCSB PDB", "dialog-primary", () => {
      const id = pdbId.value.trim().toUpperCase();
      if (id) window.open(`https://www.rcsb.org/structure/${encodeURIComponent(id)}`, "_blank");
    });
    dialogContent.append(open);
  } else {
    dialogContent.append(dialogParagraph(`${title} dialog`));
  }
  if (referenceDialog.showModal) referenceDialog.showModal();
}

function applyCenterMark(value) {
  centerMark = value;
  centerMarkElement.classList.toggle("axis", value === "axis");
  centerMarkElement.classList.toggle("none", value === "none");
}

function restoreDefaultPanels() {
  const openPanels = new Set(["Workspace", "MolStruct", "Selection", "Coloring", "Density map"]);
  document.querySelectorAll("[data-side-card]").forEach((card) => {
    const title = card.querySelector(".side-title span:nth-child(2)")?.textContent;
    const open = openPanels.has(title);
    card.classList.toggle("open", open);
    const button = card.querySelector("[data-panel-toggle]");
    button?.setAttribute("aria-expanded", String(open));
    const disclosure = card.querySelector(".disclosure");
    if (disclosure) disclosure.textContent = open ? "▾" : "▸";
  });
  document.querySelector(".side-panel")?.scrollTo({ top: 0 });
  scheduleRender(140);
}

async function handleLocalAction(action) {
  switch (action) {
    case "new-window": {
      const WebviewWindow = tauri?.webviewWindow?.WebviewWindow;
      if (!WebviewWindow) {
        showError("Tauri window API is unavailable.");
        break;
      }
      new WebviewWindow(`cuemol-${Date.now()}`, {
        url: "index.html",
        title: "CueMol2",
        width: 1024,
        height: 768,
        minWidth: 760,
        minHeight: 520,
      });
      break;
    }
    case "reload-scene":
      if (lastOpenedPaths.length) await mutate("open_paths", { paths: lastOpenedPaths });
      else showError("No loaded path to reload.");
      break;
    case "color-proof":
      if (snapshot) {
        await mutate("set_display_option", {
          option: "color-proof",
          value: !snapshot.colorProofing,
        });
      }
      break;
    case "performance-measure":
      performanceMeasure = !performanceMeasure;
      renderSize.hidden = !performanceMeasure;
      recordStatus(`Performance measure: ${performanceMeasure ? "on" : "off"}`);
      break;
    case "toggle-ribbon":
      document.querySelector(".ribbon")?.classList.toggle("collapsed");
      scheduleRender(140);
      break;
    case "clear-log":
      statusTrail = [];
      statusHistory.textContent = "";
      break;
    case "restore-panels":
      restoreDefaultPanels();
      break;
    case "show-console":
      showBottomPanel("output");
      break;
    case "check-updates":
      recordStatus("Update check is not available offline");
      break;
  }
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

function replaceOptions(select, objects) {
  select.replaceChildren();
  for (const object of objects) {
    const option = document.createElement("option");
    option.value = String(object.uid ?? object.objectUid);
    option.textContent = object.name ?? object.objectName;
    select.append(option);
  }
  select.disabled = objects.length === 0;
  if (selectedObjectUid !== null) select.value = String(selectedObjectUid);
}

function renderMolStruct() {
  replaceOptions(
    molstructObject,
    snapshot.structure.map((object) => ({ uid: object.objectUid, name: object.objectName })),
  );
  molstructTree.replaceChildren();
  const object = snapshot.structure.find((item) => item.objectUid === selectedObjectUid);
  if (!object) {
    const empty = document.createElement("p");
    empty.textContent = "No molecule selected.";
    molstructTree.append(empty);
    return;
  }
  for (const chain of object.chains) {
    const chainNode = document.createElement("details");
    chainNode.className = "structure-node chain-node";
    const chainLabel = chain.chainId || "_";
    const residueCount = chain.residues.length;
    const chainSummary = document.createElement("summary");
    chainSummary.textContent = `Chain ${chainLabel} (${residueCount} res)`;
    chainNode.append(chainSummary);
    for (const residue of chain.residues) {
      const residueNode = document.createElement("details");
      residueNode.className = "structure-node residue-node";
      const residueSummary = document.createElement("summary");
      residueSummary.textContent = `${residue.resName} ${residue.resId}${residue.insCode || ""}`;
      residueNode.append(residueSummary);
      const atomList = document.createElement("div");
      atomList.className = "atom-list";
      for (const atom of residue.atoms) {
        const atomButton = button(`${atom.name}  ${atom.element || "?"}  #${atom.serial}`, "atom-row", () => {
          mutate("set_selection", { selection: `aid ${atom.serial}` });
        });
        atomButton.title = `atom index ${atom.atomIndex}`;
        atomList.append(atomButton);
      }
      residueNode.append(atomList);
      chainNode.append(residueNode);
    }
    molstructTree.append(chainNode);
  }
}

function renderColoring() {
  const objects = activeObjects();
  replaceOptions(coloringObject, objects);
  const object = selectedObject();
  const value =
    object?.coloringType === "RainbowColoring"
      ? "rainbow"
      : object?.coloringType === "BfacColoring"
        ? "bfac"
        : "element";
  coloringType.value = value;
  coloringType.disabled = !object;
  coloringNote.textContent =
    value === "rainbow"
      ? "N-terminus blue → C-terminus red."
      : value === "bfac"
        ? "Blue (low) → red (high), auto-scaled."
        : "Per-element CPK colors.";
}

function renderSymmetry() {
  symmetryContent.replaceChildren();
  if (snapshot.crystals.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "No crystal cell (non-crystallographic).";
    symmetryContent.append(empty);
  }
  for (const crystal of snapshot.crystals) {
    const card = document.createElement("dl");
    card.className = "summary-grid";
    const values = [
      ["Molecule", crystal.objectName],
      ["Space group", crystal.spaceGroupName || "(none)"],
      ["a, b, c (Å)", `${crystal.a.toFixed(3)}, ${crystal.b.toFixed(3)}, ${crystal.c.toFixed(3)}`],
      ["α, β, γ (°)", `${crystal.alpha.toFixed(2)}, ${crystal.beta.toFixed(2)}, ${crystal.gamma.toFixed(2)}`],
    ];
    for (const [term, description] of values) {
      const dt = document.createElement("dt");
      dt.textContent = term;
      const dd = document.createElement("dd");
      dd.textContent = description;
      card.append(dt, dd);
    }
    symmetryContent.append(card);
  }
  showUnitCell.checked = snapshot.showUnitCell;
  showSymmetryMates.checked = snapshot.showSymmetryMates;
  showUnitCell.disabled = snapshot.crystals.length === 0;
  showSymmetryMates.disabled = snapshot.crystals.length === 0;
}

function renderDensity() {
  densityContent.replaceChildren();
  if (snapshot.densityMaps.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "No density map loaded.";
    densityContent.append(empty);
  }
  for (const map of snapshot.densityMaps) {
    const entry = document.createElement("div");
    entry.className = "density-summary";
    const title = document.createElement("strong");
    title.textContent = map.name;
    const dimensions = document.createElement("span");
    dimensions.textContent = `${map.format} · ${map.counts.join(" × ")}`;
    const range = document.createElement("span");
    range.textContent = `mean ${map.meanDensity.toFixed(3)} / rmsd ${map.rmsdDensity.toFixed(3)}`;
    entry.append(title, dimensions, range);
    densityContent.append(entry);
  }
  showDensityContour.checked = snapshot.showDensityContour;
  showDensityContour.disabled = snapshot.densityMaps.length === 0;
  densityLevel.value = String(snapshot.densityLevel);
  densityLevel.disabled = snapshot.densityMaps.length === 0;
  densityLevelValue.textContent = `${snapshot.densityLevel.toFixed(1)} σ`;
}

function renderSequence() {
  sequenceView.replaceChildren();
  if (snapshot.sequence.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "No molecule loaded.";
    sequenceView.append(empty);
    return;
  }
  for (const chain of snapshot.sequence) {
    const row = document.createElement("section");
    row.className = "sequence-chain";
    const title = document.createElement("strong");
    title.textContent = `${chain.objectName} : chain ${chain.chainId || "_"}`;
    const residues = document.createElement("div");
    residues.className = "sequence-residues";
    for (const residue of chain.residues) {
      const cell = button(residue.letter, `sequence-cell${residue.selected ? " selected" : ""}`, () => {
        mutate("set_selection", { selection: residue.selection });
      });
      cell.title = `${residue.resName} ${residue.resId}`;
      residues.append(cell);
    }
    row.append(title, residues);
    sequenceView.append(row);
  }
}

function formatAnimationTime(seconds) {
  const value = Math.max(0, Number(seconds) || 0);
  const minutes = Math.floor(value / 60);
  const remainder = Math.floor(value % 60);
  return `${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`;
}

function syncAnimationTimer() {
  if (snapshot?.animationPlaying && invoke) {
    if (animationTimer) return;
    animationTimer = setInterval(async () => {
      if (animationTickBusy) return;
      animationTickBusy = true;
      try {
        snapshot = await invoke("tick_animation", { dt: 0.1 });
        renderShell();
        await renderViewport();
      } catch (error) {
        showError(error);
      } finally {
        animationTickBusy = false;
      }
    }, 100);
  } else if (animationTimer) {
    clearInterval(animationTimer);
    animationTimer = null;
  }
}

function renderAnimation() {
  const objects = snapshot.animationObjects;
  if (!objects.some((object) => object.index === selectedAnimationIndex)) {
    selectedAnimationIndex = null;
  }
  animationList.replaceChildren();
  if (objects.length === 0) {
    const empty = document.createElement("p");
    empty.className = "animation-empty";
    empty.textContent = "No animation objects.";
    animationList.append(empty);
  }
  for (const object of objects) {
    const row = button("", `animation-row${object.index === selectedAnimationIndex ? " selected" : ""}`, () => {
      selectedAnimationIndex = object.index;
      renderAnimation();
    });
    const name = document.createElement("span");
    name.textContent = object.name;
    const start = document.createElement("span");
    start.textContent = object.start.toFixed(1);
    const end = document.createElement("span");
    end.textContent = object.end.toFixed(1);
    row.append(name, start, end);
    animationList.append(row);
  }
  const total = snapshot.animationDuration;
  animationDuration.textContent = formatAnimationTime(total);
  animationSlider.max = String(Math.max(total, 0));
  animationSlider.value = String(Math.min(snapshot.animationElapsed, total));
  animationSlider.disabled = total <= 0;
  animationTime.textContent = `${formatAnimationTime(snapshot.animationElapsed)} / ${formatAnimationTime(total)}`;
  animationLoop.checked = snapshot.animationLoop;
  animationPlay.disabled = objects.length === 0;
  animationStop.disabled = !snapshot.animationPlaying && snapshot.animationElapsed <= 0;
  const playIcon = animationPlay.querySelector(".tool-icon");
  const playLabel = animationPlay.querySelector("span:last-child");
  playIcon.textContent = snapshot.animationPlaying ? "Ⅱ" : "▶";
  playLabel.textContent = snapshot.animationPlaying ? "Pause" : "Play";
  const selected = selectedAnimationIndex !== null;
  animationDelete.disabled = !selected;
  animationChange.disabled = !selected;
  animationUp.disabled = !selected || selectedAnimationIndex === 0;
  animationDown.disabled = !selected || selectedAnimationIndex === objects.length - 1;
  syncAnimationTimer();
}

function renderShell() {
  if (!snapshot) return;
  normalizeSelection();
  selectionInput.value = snapshot.selection;
  projectionReadout.textContent = snapshot.perspective ? "PERSPECTIVE" : "ORTHO";
  document.querySelectorAll("input[name='projection']").forEach((input) => {
    input.checked = input.value === (snapshot.perspective ? "perspective" : "orthographic");
  });
  document.querySelectorAll("input[name='background']").forEach((input) => {
    input.checked = input.value === snapshot.background;
  });
  document.querySelectorAll("[data-projection]").forEach((item) => {
    item.classList.toggle(
      "checked",
      item.dataset.projection === (snapshot.perspective ? "perspective" : "orthographic"),
    );
  });
  document.querySelectorAll("[data-background]").forEach((item) => {
    item.classList.toggle("checked", item.dataset.background === snapshot.background);
  });
  document
    .querySelector("[data-local-action='color-proof']")
    ?.classList.toggle("checked", snapshot.colorProofing);
  document.querySelectorAll("[data-action='undo']").forEach((item) => {
    item.disabled = !snapshot.canUndo;
  });
  document.querySelectorAll("[data-action='redo']").forEach((item) => {
    item.disabled = !snapshot.canRedo;
  });
  measureReadout.textContent = snapshot.measureResult || "Select 2 / 3 / 4 atoms";
  statusText.textContent = snapshot.status;
  recordStatus(snapshot.status);
  metrics.textContent = `${snapshot.scenes.length} scenes / ${snapshot.objects.length} objects`;
  renderSceneTabs();
  renderSceneTree();
  renderInspector();
  renderMolStruct();
  renderColoring();
  renderSymmetry();
  renderDensity();
  renderSequence();
  renderAnimation();
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
  lastOpenedPaths = paths;
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
    case "undo":
      await mutate("undo");
      break;
    case "redo":
      await mutate("redo");
      break;
    case "clear-undo":
      await mutate("clear_undo");
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
    case "select-object":
      selectedObjectUid = Number(molstructObject.value) || null;
      renderShell();
      break;
  }
}

document.querySelectorAll("[data-action]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    handleAction(element.dataset.action);
  });
});

document.querySelectorAll("[data-dialog]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    showReferenceDialog(element.dataset.dialog);
  });
});

document.querySelectorAll("[data-local-action]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    handleLocalAction(element.dataset.localAction);
  });
});

document.querySelectorAll("[data-background]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    if (snapshot?.background !== element.dataset.background) {
      mutate("set_background", { background: element.dataset.background });
    }
  });
});

document.querySelectorAll("[data-projection]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    const wantsPerspective = element.dataset.projection === "perspective";
    if (snapshot && snapshot.perspective !== wantsPerspective) {
      cameraCommand("projection", 0, 0, true);
    }
  });
});

document.querySelectorAll("[data-center-mark]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    applyCenterMark(element.dataset.centerMark);
    document.querySelectorAll("[data-center-mark]").forEach((item) => {
      item.classList.toggle("checked", item === element);
    });
  });
});

document.querySelectorAll("[data-panel-menu]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    const card = [...document.querySelectorAll("[data-side-card]")].find(
      (item) =>
        item.querySelector(".side-title span:nth-child(2)")?.textContent ===
        element.dataset.panelMenu,
    );
    if (!card) return;
    card.classList.add("open");
    card.querySelector("[data-panel-toggle]")?.setAttribute("aria-expanded", "true");
    const disclosure = card.querySelector(".disclosure");
    if (disclosure) disclosure.textContent = "▾";
    card.scrollIntoView({ block: "nearest" });
    scheduleRender(140);
  });
});

document.querySelectorAll(".menu").forEach((menu) => {
  menu.addEventListener("toggle", () => {
    if (menu.open) closeMenus(menu);
  });
});

document.querySelectorAll("[data-ribbon-select]").forEach((element) => {
  element.addEventListener("click", () => {
    const target = element.dataset.ribbonSelect;
    document.querySelectorAll("[data-ribbon-select]").forEach((tab) => {
      const active = tab === element;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", String(active));
    });
    document.querySelectorAll("[data-ribbon-panel]").forEach((panel) => {
      panel.classList.toggle("active", panel.dataset.ribbonPanel === target);
    });
    scheduleRender(140);
  });
});

document.querySelector("[data-ribbon-collapse]")?.addEventListener("click", () => {
  document.querySelector(".ribbon")?.classList.toggle("collapsed");
  scheduleRender(140);
});

document.querySelectorAll("[data-panel-toggle]").forEach((element) => {
  element.addEventListener("click", (event) => {
    if (event.target.closest(".panel-close")) return;
    const card = element.closest("[data-side-card]");
    const open = card.classList.toggle("open");
    element.setAttribute("aria-expanded", String(open));
    element.querySelector(".disclosure").textContent = open ? "▾" : "▸";
    scheduleRender(140);
  });
});

document.querySelectorAll(".toolbar-group[role='radiogroup']").forEach((group) => {
  group.querySelectorAll(".tool-button").forEach((element) => {
    element.addEventListener("click", () => {
      group.querySelectorAll(".tool-button").forEach((button) => {
        button.classList.toggle("selected", button === element);
      });
      if (element.dataset.interaction) {
        interactionMode = element.dataset.interaction;
        viewport.classList.toggle("rect-select", interactionMode === "rect-select");
      }
    });
  });
});

document.querySelectorAll("[data-bottom-select]").forEach((element) => {
  element.addEventListener("click", () => {
    showBottomPanel(element.dataset.bottomSelect);
  });
});

document.querySelectorAll(".selection-tabs button").forEach((element) => {
  element.addEventListener("click", () => {
    document.querySelectorAll(".selection-tabs button").forEach((tab) => {
      tab.classList.toggle("active", tab === element);
    });
  });
});

molstructObject.addEventListener("change", () => {
  selectedObjectUid = Number(molstructObject.value) || null;
  renderShell();
});

coloringObject.addEventListener("change", () => {
  selectedObjectUid = Number(coloringObject.value) || null;
  renderShell();
});

coloringType.addEventListener("change", () => {
  const object = selectedObject();
  if (object) {
    mutate("set_object_coloring", {
      objectUid: object.uid,
      coloring: coloringType.value,
    });
  }
});

showUnitCell.addEventListener("change", () => {
  mutate("set_display_option", { option: "unit-cell", value: showUnitCell.checked });
});

showSymmetryMates.addEventListener("change", () => {
  mutate("set_display_option", { option: "symmetry-mates", value: showSymmetryMates.checked });
});

showDensityContour.addEventListener("change", () => {
  mutate("set_display_option", { option: "density-contour", value: showDensityContour.checked });
});

densityLevel.addEventListener("input", () => {
  densityLevelValue.textContent = `${Number(densityLevel.value).toFixed(1)} σ`;
});

densityLevel.addEventListener("change", () => {
  mutate("set_density_level", { level: Number(densityLevel.value) });
});

animationPlay.addEventListener("click", () => {
  mutate("animation_control", {
    action: snapshot?.animationPlaying ? "pause" : "play",
  });
});

animationStop.addEventListener("click", () => {
  mutate("animation_control", { action: "stop" });
});

animationLoop.addEventListener("change", () => {
  mutate("set_animation_loop", { value: animationLoop.checked });
});

animationSlider.addEventListener("input", () => {
  animationTime.textContent = `${formatAnimationTime(animationSlider.value)} / ${formatAnimationTime(snapshot?.animationDuration)}`;
});

animationSlider.addEventListener("change", () => {
  mutate("seek_animation", { elapsed: Number(animationSlider.value) });
});

animationAdd.addEventListener("click", () => {
  mutate("add_animation_object", { kind: animationKind.value });
});

animationDelete.addEventListener("click", () => {
  if (selectedAnimationIndex !== null) {
    mutate("delete_animation_object", { index: selectedAnimationIndex });
  }
});

animationChange.addEventListener("click", () => {
  showReferenceDialog("Animation object properties");
});

animationUp.addEventListener("click", () => {
  if (selectedAnimationIndex !== null) {
    mutate("move_animation_object", {
      index: selectedAnimationIndex,
      direction: -1,
    });
    selectedAnimationIndex -= 1;
  }
});

animationDown.addEventListener("click", () => {
  if (selectedAnimationIndex !== null) {
    mutate("move_animation_object", {
      index: selectedAnimationIndex,
      direction: 1,
    });
    selectedAnimationIndex += 1;
  }
});

document.querySelectorAll("input[name='projection']").forEach((input) => {
  input.addEventListener("change", () => {
    const wantsPerspective = input.value === "perspective";
    if (input.checked && snapshot && snapshot.perspective !== wantsPerspective) {
      cameraCommand("projection", 0, 0, true);
    }
  });
});

document.querySelectorAll("input[name='background']").forEach((input) => {
  input.addEventListener("change", () => {
    if (input.checked) mutate("set_background", { background: input.value });
  });
});

document.addEventListener("pointerdown", (event) => {
  document.querySelectorAll(".menu[open]").forEach((menu) => {
    if (!menu.contains(event.target)) menu.removeAttribute("open");
  });
});

selectionInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") handleAction("select");
});

viewport.addEventListener("contextmenu", (event) => event.preventDefault());
viewport.addEventListener("pointerdown", (event) => {
  if (event.button !== 0 && event.button !== 2) return;
  viewport.setPointerCapture(event.pointerId);
  const action =
    event.button === 2
      ? "pan"
      : event.shiftKey
        ? "zoom"
        : interactionMode === "rect-select"
          ? "rect-select"
          : "rotate";
  drag = {
    pointerId: event.pointerId,
    x: event.clientX,
    y: event.clientY,
    dx: 0,
    dy: 0,
    action,
  };
  viewport.classList.add("dragging");
  dragReadout.textContent = drag.action.toUpperCase();
});

viewport.addEventListener("pointermove", (event) => {
  if (!drag || drag.pointerId !== event.pointerId) return;
  drag.dx = event.clientX - drag.x;
  drag.dy = event.clientY - drag.y;
  dragReadout.textContent = `${drag.action.toUpperCase()}  ${Math.round(drag.dx)}, ${Math.round(drag.dy)}`;
  if (drag.action === "rect-select") {
    const bounds = viewport.getBoundingClientRect();
    selectionRectangle.style.left = `${Math.min(drag.x, event.clientX) - bounds.left}px`;
    selectionRectangle.style.top = `${Math.min(drag.y, event.clientY) - bounds.top}px`;
    selectionRectangle.style.width = `${Math.abs(drag.dx)}px`;
    selectionRectangle.style.height = `${Math.abs(drag.dy)}px`;
    selectionRectangle.classList.add("visible");
  }
});

function finishDrag(event) {
  if (!drag || drag.pointerId !== event.pointerId) return;
  const completed = drag;
  drag = null;
  viewport.classList.remove("dragging");
  selectionRectangle.classList.remove("visible");
  dragReadout.textContent = "";
  const bounds = viewport.getBoundingClientRect();
  const startX = completed.x - bounds.left;
  const startY = completed.y - bounds.top;
  if (Math.abs(completed.dx) + Math.abs(completed.dy) < 2) {
    if (completed.action === "rotate" || completed.action === "rect-select") {
      mutate("pick_atom", {
        x: startX,
        y: startY,
        ...viewportDimensions(),
      });
    }
    return;
  }
  if (completed.action === "rect-select") {
    mutate("select_screen_rect", {
      x1: startX,
      y1: startY,
      x2: startX + completed.dx,
      y2: startY + completed.dy,
      ...viewportDimensions(),
    });
  } else if (completed.action === "zoom") {
    cameraCommand("zoom", Math.exp(completed.dy * 0.01));
  } else {
    const scale = completed.action === "rotate" ? 0.35 : 1;
    cameraCommand(completed.action, completed.dx * scale, completed.dy * scale);
  }
}

viewport.addEventListener("pointerup", finishDrag);
viewport.addEventListener("pointercancel", () => {
  drag = null;
  viewport.classList.remove("dragging");
  selectionRectangle.classList.remove("visible");
  dragReadout.textContent = "";
});
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
  if (paths.length) {
    lastOpenedPaths = paths;
    mutate("open_paths", { paths });
  }
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

"use strict";

const appShell = document.querySelector(".app-shell");
const sidePanel = document.querySelector(".side-panel");
const viewport = document.getElementById("viewport");
const viewportImage = document.getElementById("viewport-image");
const viewportLines = document.getElementById("viewport-lines");
const viewportLabels = document.getElementById("viewport-labels");
const sceneTabs = document.getElementById("scene-tabs");
const sceneTree = document.getElementById("scene-tree");
const rendererList = document.getElementById("renderer-list");
const selectionInput = document.getElementById("selection-input");
const selectionObject = document.getElementById("selection-object");
const selectionCommandPanel = document.getElementById("selection-command-panel");
const selectionEditorPanel = document.getElementById("selection-editor-panel");
const selectionHistory = document.getElementById("selection-history");
const selectionEditorList = document.getElementById("selection-editor-list");
const selectionEditorApply = document.getElementById("selection-editor-apply");
const selectionEditorAddKind = document.getElementById("selection-editor-add-kind");
const selectionEditorDelete = document.getElementById("selection-editor-delete");
const selectionEditorClear = document.getElementById("selection-editor-clear");
const selectionChainCandidates = document.getElementById("selection-chain-candidates");
const selectionResnameCandidates = document.getElementById("selection-resname-candidates");
const selectionAtomCandidates = document.getElementById("selection-atom-candidates");
const selectionElementCandidates = document.getElementById("selection-element-candidates");
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
const molstructSelect = document.getElementById("molstruct-select");
const molstructCenter = document.getElementById("molstruct-center");
const molstructZoom = document.getElementById("molstruct-zoom");
const molstructProperties = document.getElementById("molstruct-properties");
const coloringTarget = document.getElementById("coloring-target");
const coloringType = document.getElementById("coloring-type");
const coloringDeck = document.getElementById("coloring-deck");
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
const dialogFooter = document.getElementById("dialog-footer");
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
const trajectoryObject = document.getElementById("trajectory-object");
const trajectoryFirst = document.getElementById("trajectory-first");
const trajectoryPrevious = document.getElementById("trajectory-previous");
const trajectoryPlay = document.getElementById("trajectory-play");
const trajectoryNext = document.getElementById("trajectory-next");
const trajectoryLast = document.getElementById("trajectory-last");
const trajectoryFrame = document.getElementById("trajectory-frame");
const trajectoryLastFrame = document.getElementById("trajectory-last-frame");
const trajectorySlider = document.getElementById("trajectory-slider");
const trajectoryEmpty = document.getElementById("trajectory-empty");
const trajectoryLoop = document.getElementById("trajectory-loop");
const trajectorySpeed = document.getElementById("trajectory-speed");
const trajectoryBlockStrip = document.getElementById("trajectory-block-strip");
const trajectoryBlockAdd = document.getElementById("trajectory-block-add");
const trajectoryBlockRemove = document.getElementById("trajectory-block-remove");
const trajectoryBlockLeft = document.getElementById("trajectory-block-left");
const trajectoryBlockRight = document.getElementById("trajectory-block-right");
const trajectoryBlockCount = document.getElementById("trajectory-block-count");
const recentMenu = document.getElementById("recent-menu");
const windowsMenu = document.getElementById("windows-menu");
const windowsMenuTrigger = document.getElementById("windows-menu-trigger");
const workspaceProperties = document.getElementById("workspace-properties");
const workspaceDelete = document.getElementById("workspace-delete");
const workspaceZoom = document.getElementById("workspace-zoom");
const rendererType = document.getElementById("renderer-type");

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const dialog = tauri?.dialog;

const RECENT_STORAGE_KEY = "cuemol2.recentFiles.v1";
const SELECTION_HISTORY_STORAGE_KEY = "cuemol2.selectionHistory.v1";
const MAX_RECENT_FILES = 10;
const MAX_SELECTION_HISTORY = 20;
const MOLECULE_RENDERER_TYPES = [
  "anisou",
  "atomintr",
  "ballstick",
  "cartoon",
  "cpk",
  "disorder",
  "dsurf2",
  "dsurface",
  "nucl",
  "ribbon",
  "simple",
  "spline",
  "trace",
  "tube",
];
const WORKSPACE_PAINT_MONOCHROME = [
  ["White", "#FFF"],
  ["75% Gray", "rgb(0.75,0.75,0.75)"],
  ["50% Gray", "rgb(0.5,0.5,0.5)"],
  ["25% Gray", "rgb(0.25,0.25,0.25)"],
  ["Black", "#000"],
];
const WORKSPACE_PAINT_HUES = [
  ["Red", 0],
  ["Orange", 30],
  ["Yellow", 60],
  ["Green", 120],
  ["Cyan", 180],
  ["Blue", 240],
  ["Purple", 300],
];

let snapshot = null;
let selectedObjectUid = null;
let selectedRendererUid = null;
const workspaceRendererStyleCache = new Map();
const spaceGroupOptionsCache = new Map();
let workspaceCameraClipboardAvailable = false;
let workspaceObjectClipboardAvailable = false;
let workspaceRendererClipboardAvailable = false;
let workspaceStyleClipboardName = null;
let selectedDensityMapUid = null;
let selectedDensityRendererUid = null;
let selectedSurfaceUid = null;
let selectedSurfaceRendererUid = null;
let selectedCameraName = null;
let selectedStyleIndex = null;
let workspaceSelectionKind = "object";
let molstructObjectUid = null;
const molstructSelectedKeys = new Set();
const molstructOpenKeys = new Set();
let molstructSelectionAnchor = null;
let selectionObjectUid = null;
let selectionActiveTab = "command";
let selectionEditorItems = [];
let selectionEditorSelectedIndex = -1;
let coloringTargetKey = null;
let coloringPaintSelectedIndex = -1;
let coloringGradientSelectedIndex = -1;
let selectionHistoryEntries = loadSelectionHistory();
let renderTimer = null;
let viewportRenderRequest = null;
let viewportRenderQueued = false;
let cameraRequest = null;
const cameraQueue = [];
let renderedViewportSize = "";
let wheelTimer = null;
let wheelPending = null;
let toastTimer = null;
let requestSequence = 0;
let drag = null;
let statusTrail = [];
let centerMark = "cross";
let performanceMeasure = false;
let interactionMode = "rotate";
let atomPickerTarget = null;
let selectedAnimationIndex = null;
let selectedMorphFrameIndex = null;
let lastModelLogCount = 0;
let animationTimer = null;
let animationTickBusy = false;
let selectedTrajectoryUid = null;
let trajectoryPlaying = false;
let trajectoryTimer = null;
let trajectoryTickBusy = false;
let selectedTrajectoryBlockUid = null;
let draggedTrajectoryBlockUid = null;
let draggedWorkspaceObject = null;
let draggedWorkspaceRenderer = null;
const trajectoryFrames = new Map();
let recentFiles = loadRecentFiles();
let applicationInfoCache = null;
let preferences = null;
let moleculeDetails = {
  structureKey: "",
  structureObjectUid: null,
  sequenceKey: "",
  structure: [],
  sequence: [],
};
const moleculeStructureCache = new Map();
const pendingMoleculeObjectUids = new Set();
let moleculeDetailsTimer = null;
let moleculeDetailsRequest = null;
let renderedMolstructKey = "";
let renderedSequenceKey = "";
let renderedSceneTabsKey = "";
let renderedSceneTreeKey = "";
let renderedInspectorKey = "";
let secondaryPanelRenderGeneration = 0;
let selectionCandidateCache = { key: "", values: null };

function loadRecentFiles() {
  try {
    const stored = JSON.parse(localStorage.getItem(RECENT_STORAGE_KEY) || "[]");
    if (!Array.isArray(stored)) return [];
    return stored
      .filter(
        (entry) =>
          entry &&
          typeof entry.path === "string" &&
          entry.path.length > 0 &&
          (entry.ftype === "obj" || entry.ftype === "scene"),
      )
      .slice(0, MAX_RECENT_FILES);
  } catch {
    return [];
  }
}

function saveRecentFiles() {
  try {
    localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(recentFiles));
  } catch (error) {
    console.warn("Cannot persist recent files:", error);
  }
}

function loadSelectionHistory() {
  try {
    const stored = JSON.parse(localStorage.getItem(SELECTION_HISTORY_STORAGE_KEY) || "[]");
    return Array.isArray(stored)
      ? stored.filter((entry) => typeof entry === "string" && entry).slice(0, MAX_SELECTION_HISTORY)
      : [];
  } catch {
    return [];
  }
}

function saveSelectionHistory() {
  try {
    localStorage.setItem(SELECTION_HISTORY_STORAGE_KEY, JSON.stringify(selectionHistoryEntries));
  } catch (error) {
    console.warn("Cannot persist selection history:", error);
  }
}

function rememberSelection(selection) {
  const value = selection.trim();
  if (!value) return;
  selectionHistoryEntries = selectionHistoryEntries.filter((entry) => entry !== value);
  selectionHistoryEntries.unshift(value);
  selectionHistoryEntries = selectionHistoryEntries.slice(0, MAX_SELECTION_HISTORY);
  saveSelectionHistory();
  renderSelectionHistory();
}

function renderSelectionHistory() {
  selectionHistory.replaceChildren();
  const placeholder = document.createElement("option");
  placeholder.value = "";
  placeholder.textContent = selectionHistoryEntries.length ? "History" : "(no history)";
  selectionHistory.append(placeholder);
  for (const entry of selectionHistoryEntries) {
    const option = document.createElement("option");
    option.value = entry;
    option.textContent = entry;
    selectionHistory.append(option);
  }
  selectionHistory.value = "";
  selectionHistory.disabled = selectionHistoryEntries.length === 0;
}

function recentFileType(path) {
  return /\.qsc$/i.test(path) ? "scene" : "obj";
}

function fileBasename(path) {
  const parts = path.split(/[\\/]/);
  return parts.at(-1) || path;
}

function addRecentFile(path, ftype = recentFileType(path)) {
  if (!path) return;
  addRecentFiles([{ path, ftype }]);
}

function addRecentFiles(entries) {
  for (const { path, ftype = recentFileType(path) } of entries) {
    if (!path) continue;
    recentFiles = recentFiles.filter((entry) => entry.path !== path);
    recentFiles.unshift({ path, ftype });
  }
  recentFiles.length = Math.min(recentFiles.length, MAX_RECENT_FILES);
  saveRecentFiles();
  renderRecentMenu();
}

function clearRecentFiles() {
  recentFiles = [];
  saveRecentFiles();
  renderRecentMenu();
}

function renderRecentMenu() {
  if (!recentMenu) return;
  recentMenu.replaceChildren();
  if (recentFiles.length === 0) {
    const empty = button("(none)", "", () => {});
    empty.disabled = true;
    recentMenu.append(empty);
  } else {
    for (const entry of recentFiles) {
      const open = button(fileBasename(entry.path), "", async () => {
        closeMenus();
        const opened = await openSelectedPaths([entry.path]);
        if (!opened) {
          recentFiles = recentFiles.filter((item) => item.path !== entry.path);
          saveRecentFiles();
          renderRecentMenu();
        }
      });
      open.title = entry.path;
      open.setAttribute("role", "menuitem");
      recentMenu.append(open);
    }
  }
  const separator = document.createElement("span");
  separator.className = "menu-separator";
  separator.setAttribute("role", "separator");
  const clear = button("Clear Menu", "", () => {
    closeMenus();
    clearRecentFiles();
  });
  clear.disabled = recentFiles.length === 0;
  clear.setAttribute("role", "menuitem");
  recentMenu.append(separator, clear);
}

function closeHistoryMenus(except = null) {
  document.querySelectorAll("[data-history-group]").forEach((group) => {
    const keepOpen = group.dataset.historyGroup === except;
    group.classList.toggle("open", keepOpen);
    group
      .querySelector("[data-history-toggle]")
      ?.setAttribute("aria-expanded", String(keepOpen));
  });
}

function renderHistoryMenu(kind, descriptions, canExecute) {
  const group = document.querySelector(`[data-history-group='${kind}']`);
  const popup = document.getElementById(`${kind}-history`);
  const caret = document.querySelector(`[data-history-toggle='${kind}']`);
  const main = group?.querySelector(`[data-action='${kind}']`);
  const entries = Array.isArray(descriptions) ? descriptions : [];
  if (!group || !popup || !caret || !main) return;

  popup.replaceChildren();
  for (const [depth, description] of entries.entries()) {
    const item = button(description, "", async (event) => {
      event.stopPropagation();
      closeHistoryMenus();
      await mutate(kind, { depth });
    });
    item.title = description;
    item.setAttribute("role", "menuitem");
    item.dataset.depth = String(depth);
    popup.append(item);
  }
  main.disabled = !canExecute;
  main.title = canExecute && entries.length ? `${kind === "undo" ? "Undo" : "Redo"}: ${entries[0]}` : "";
  caret.disabled = entries.length === 0;
  if (entries.length === 0) closeHistoryMenus();
}

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
  closeHistoryMenus();
}

function showBottomPanel(name) {
  document.querySelectorAll("[data-bottom-select]").forEach((tab) => {
    tab.classList.toggle("active", tab.dataset.bottomSelect === name);
  });
  document.querySelectorAll("[data-bottom-panel]").forEach((panel) => {
    panel.classList.toggle("active", panel.dataset.bottomPanel === name);
  });
  if (name === "sequence") {
    renderSequence();
    scheduleMoleculeDetailsRefresh(0);
  } else if (name === "trajectory") {
    renderTrajectory();
  }
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

function dialogTextArea(label, value) {
  const wrapper = document.createElement("label");
  wrapper.className = "dialog-field dialog-textarea";
  const text = document.createElement("span");
  text.textContent = label;
  const input = document.createElement("textarea");
  input.value = value;
  input.spellcheck = false;
  wrapper.append(text, input);
  dialogContent.append(wrapper);
  return input;
}

function dialogSelect(label, options, value, parent = dialogContent) {
  const wrapper = document.createElement("label");
  wrapper.className = "dialog-field";
  const text = document.createElement("span");
  text.textContent = label;
  const select = document.createElement("select");
  for (const [optionValue, optionLabel] of options) {
    const option = document.createElement("option");
    option.value = String(optionValue);
    option.textContent = optionLabel;
    select.append(option);
  }
  select.value = String(value);
  wrapper.append(text, select);
  parent.append(wrapper);
  return select;
}

function dialogCheckbox(label, checked, parent = dialogContent) {
  const wrapper = document.createElement("label");
  wrapper.className = "dialog-check";
  const input = document.createElement("input");
  input.type = "checkbox";
  input.checked = checked;
  const text = document.createElement("span");
  text.textContent = label;
  wrapper.append(input, text);
  parent.append(wrapper);
  return input;
}

function dialogGroup(title) {
  const group = document.createElement("fieldset");
  group.className = "property-common-group";
  const legend = document.createElement("legend");
  legend.textContent = title;
  group.append(legend);
  dialogContent.append(group);
  return group;
}

function dialogActions(...actions) {
  dialogFooter.hidden = true;
  referenceDialog.classList.add("has-actions");
  const row = document.createElement("div");
  row.className = "dialog-actions";
  row.append(...actions);
  dialogContent.append(row);
}

function replaceFileExtension(path, extension) {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  const dot = path.lastIndexOf(".");
  return `${dot > slash ? path.slice(0, dot) : path}${extension}`;
}

async function getApplicationInfo(refresh = false) {
  if (!invoke) return null;
  if (!refresh && applicationInfoCache) return applicationInfoCache;
  applicationInfoCache = await invoke("get_application_info");
  return applicationInfoCache;
}

function applicationDefinitionList(entries) {
  const properties = document.createElement("dl");
  properties.className = "dialog-properties application-properties";
  for (const [term, value] of entries) {
    const dt = document.createElement("dt");
    dt.textContent = term;
    const dd = document.createElement("dd");
    dd.textContent = String(value);
    properties.append(dt, dd);
  }
  return properties;
}

function pluginCatalog(info) {
  const toolbar = document.createElement("div");
  toolbar.className = "info-toolbar";
  const search = document.createElement("input");
  search.type = "search";
  search.placeholder = "Filter plugins";
  search.setAttribute("aria-label", "Filter plugins");
  const category = document.createElement("select");
  category.setAttribute("aria-label", "Plugin category");
  const categories = ["All", ...new Set(info.plugins.map((plugin) => plugin.category))];
  for (const name of categories) {
    const option = document.createElement("option");
    option.value = name;
    option.textContent = name;
    category.append(option);
  }
  const count = document.createElement("span");
  toolbar.append(search, category, count);

  const list = document.createElement("div");
  list.className = "plugin-catalog";
  const render = () => {
    const query = search.value.trim().toLowerCase();
    const matches = info.plugins.filter(
      (plugin) =>
        (category.value === "All" || plugin.category === category.value) &&
        (!query ||
          `${plugin.name} ${plugin.description} ${plugin.extensions}`
            .toLowerCase()
            .includes(query)),
    );
    list.replaceChildren();
    for (const plugin of matches) {
      const row = document.createElement("article");
      row.className = "plugin-row";
      const name = document.createElement("strong");
      name.textContent = plugin.name;
      const description = document.createElement("span");
      description.textContent = plugin.description || plugin.category;
      const extensions = document.createElement("code");
      extensions.textContent = plugin.extensions || "—";
      const type = document.createElement("small");
      type.textContent = plugin.category;
      row.append(name, description, extensions, type);
      list.append(row);
    }
    if (matches.length === 0) {
      const empty = document.createElement("p");
      empty.className = "info-empty";
      empty.textContent = "No matching plugins.";
      list.append(empty);
    }
    count.textContent = `${matches.length} / ${info.plugins.length}`;
  };
  search.addEventListener("input", render);
  category.addEventListener("change", render);
  render();
  return [toolbar, list];
}

function configBrowser(info) {
  const toolbar = document.createElement("div");
  toolbar.className = "info-toolbar";
  const search = document.createElement("input");
  search.type = "search";
  search.placeholder = "Filter preferences";
  search.setAttribute("aria-label", "Filter preferences");
  const count = document.createElement("span");
  toolbar.append(search, count);
  const list = document.createElement("div");
  list.className = "config-browser";
  const render = () => {
    const query = search.value.trim().toLowerCase();
    const matches = info.config.filter(
      (entry) => !query || `${entry.key} ${entry.value}`.toLowerCase().includes(query),
    );
    list.replaceChildren();
    for (const entry of matches) {
      const row = document.createElement("label");
      row.className = "config-row";
      const key = document.createElement("code");
      key.textContent = entry.key;
      key.title = entry.key;
      const value = document.createElement("input");
      value.value = entry.value;
      value.setAttribute("aria-label", `${entry.key} value`);
      const apply = button("Apply", "", async () => {
        apply.disabled = true;
        setBusy(true);
        try {
          const updated = await invoke("set_config_value", {
            key: entry.key,
            value: value.value,
          });
          applicationInfoCache = updated;
          entry.value = value.value;
          statusText.textContent = updated.status;
          recordStatus(updated.status);
        } catch (error) {
          showError(error);
        } finally {
          setBusy(false);
          apply.disabled = value.value === entry.value;
        }
      });
      apply.disabled = true;
      value.addEventListener("input", () => {
        apply.disabled = value.value === entry.value;
      });
      row.append(key, value, apply);
      list.append(row);
    }
    if (matches.length === 0) {
      const empty = document.createElement("p");
      empty.className = "info-empty";
      empty.textContent = "No matching preferences.";
      list.append(empty);
    }
    count.textContent = `${matches.length} / ${info.config.length}`;
  };
  search.addEventListener("input", render);
  render();
  return [toolbar, list];
}

function copyProperties(value) {
  return JSON.parse(JSON.stringify(value));
}

function scenePropertyDefinitions() {
  return [
    { name: "uid", type: "integer", path: ["uid"], readonly: true },
    { name: "modified", type: "boolean", path: ["modified"], readonly: true },
    { name: "name", type: "string", path: ["name"], readonly: true },
    { name: "src", type: "string", path: ["source"], readonly: true },
    { name: "srctype", type: "string", path: ["sourceType"], readonly: true },
    { name: "basePath", type: "string", path: ["basePath"], readonly: true },
    { name: "bgcolor", type: "color", path: ["background"], defaultValue: "black" },
    { name: "aoEnabled", type: "boolean", path: ["aoEnabled"], defaultValue: false },
    { name: "aoRadius", type: "real", path: ["aoRadius"], defaultValue: 4 },
    { name: "aoIntensity", type: "real", path: ["aoIntensity"], defaultValue: 2.2 },
    { name: "aoSlices", type: "integer", path: ["aoSlices"], defaultValue: 9 },
    { name: "aoSteps", type: "integer", path: ["aoSteps"], defaultValue: 3 },
    { name: "aoHalfRes", type: "boolean", path: ["aoHalfRes"], defaultValue: false },
    {
      name: "aa_method",
      type: "enum",
      path: ["aaMethod"],
      defaultValue: "fxaa",
      options: [
        ["none", "none"],
        ["fxaa", "fxaa"],
        ["smaa", "smaa"],
      ],
    },
    {
      name: "aaJitterLevel",
      type: "integer",
      path: ["aaJitterLevel"],
      defaultValue: 0,
    },
    { name: "onload", type: "string", path: ["onload"], defaultValue: "" },
    { name: "obj_uids", type: "string", path: ["objectUids"], readonly: true },
    { name: "activeObjID", type: "integer", path: ["activeObjectId"], readonly: true },
    { name: "activeRendID", type: "integer", path: ["activeRendererId"], readonly: true },
    { name: "view_uids", type: "string", path: ["viewUids"], readonly: true },
    { name: "activeViewID", type: "integer", path: ["activeViewId"], readonly: true },
    {
      name: "use_colproof",
      type: "boolean",
      path: ["useColorProof"],
      defaultValue: false,
    },
    { name: "icc_filename", type: "string", path: ["iccFilename"], defaultValue: "" },
    {
      name: "icc_intent",
      type: "enum",
      path: ["iccIntent"],
      defaultValue: "perceptual",
      options: [
        ["perceptual", "perceptual"],
        ["relative_colorimetric", "relative colorimetric"],
        ["saturation", "saturation"],
        ["absolute_colorimetric", "absolute colorimetric"],
      ],
    },
  ];
}

function genericPropertyEditor(info, kind) {
  const isScene = kind === "scene";
  const isCamera = kind === "camera";
  let originalCameraName = isCamera ? info.name : null;
  let draft = copyProperties(info);
  let dirty = false;
  let selected = null;
  const definitions = isScene ? scenePropertyDefinitions() : [
    { name: "name", type: "string", path: ["name"], defaultValue: "" },
    { name: "uid", type: "integer", path: ["uid"], readonly: true },
    { name: "active", type: "boolean", path: ["active"], readonly: true },
    { name: "zoom", type: "real", path: ["zoom"], defaultValue: 50 },
    { name: "distance", type: "real", path: ["distance"], defaultValue: 200 },
    { name: "slab", type: "real", path: ["slab"], defaultValue: 50 },
    { name: "center", type: "Vector", node: true, readonly: true },
    { name: "center.x", type: "real", path: ["center", 0], defaultValue: 0, level: 1 },
    { name: "center.y", type: "real", path: ["center", 1], defaultValue: 0, level: 1 },
    { name: "center.z", type: "real", path: ["center", 2], defaultValue: 0, level: 1 },
    { name: "rotation", type: "Quat", node: true, readonly: true },
    { name: "rotation.x", type: "real", path: ["rotation", 0], defaultValue: 0, level: 1 },
    { name: "rotation.y", type: "real", path: ["rotation", 1], defaultValue: 0, level: 1 },
    { name: "rotation.z", type: "real", path: ["rotation", 2], defaultValue: 0, level: 1 },
    { name: "rotation.w", type: "real", path: ["rotation", 3], defaultValue: 1, level: 1 },
    { name: "perspective", type: "boolean", path: ["perspective"], defaultValue: false },
    { name: "width", type: "integer", path: ["width"], readonly: true },
    { name: "height", type: "integer", path: ["height"], readonly: true },
    { name: "trans_mms", type: "boolean", path: ["transMms"], defaultValue: false },
    { name: "rot_mms", type: "boolean", path: ["rotMms"], defaultValue: false },
    {
      name: "stereoMode",
      type: "enum",
      path: ["stereoMode"],
      defaultValue: "none",
      options: [
        ["none", "none"],
        ["para", "para"],
        ["cross", "cross"],
        ["hardware", "hardware"],
      ],
    },
    { name: "hasHWStereo", type: "boolean", path: ["hasHwStereo"], readonly: true },
    { name: "stereoDist", type: "real", path: ["stereoDist"], defaultValue: 1 },
    {
      name: "swapStereoEyes",
      type: "boolean",
      path: ["swapStereoEyes"],
      defaultValue: false,
    },
    { name: "up_vec", type: "Vector", node: true, readonly: true },
    { name: "up_vec.x", type: "real", path: ["upVec", 0], level: 1, readonly: true },
    { name: "up_vec.y", type: "real", path: ["upVec", 1], level: 1, readonly: true },
    { name: "up_vec.z", type: "real", path: ["upVec", 2], level: 1, readonly: true },
    { name: "right_vec", type: "Vector", node: true, readonly: true },
    { name: "right_vec.x", type: "real", path: ["rightVec", 0], level: 1, readonly: true },
    { name: "right_vec.y", type: "real", path: ["rightVec", 1], level: 1, readonly: true },
    { name: "right_vec.z", type: "real", path: ["rightVec", 2], level: 1, readonly: true },
    { name: "forward_vec", type: "Vector", node: true, readonly: true },
    {
      name: "forward_vec.x",
      type: "real",
      path: ["forwardVec", 0],
      level: 1,
      readonly: true,
    },
    {
      name: "forward_vec.y",
      type: "real",
      path: ["forwardVec", 1],
      level: 1,
      readonly: true,
    },
    {
      name: "forward_vec.z",
      type: "real",
      path: ["forwardVec", 2],
      level: 1,
      readonly: true,
    },
    {
      name: "centerMark",
      type: "enum",
      path: ["centerMark"],
      defaultValue: "crosshair",
      options: [
        ["none", "none"],
        ["crosshair", "crosshair"],
        ["axis", "axis"],
      ],
    },
  ];

  const propertyValue = (definition) => {
    if (definition.node) return "<node>";
    return definition.path.reduce((value, key) => value?.[key], draft);
  };
  const setPropertyValue = (definition, value) => {
    const [last, ...parents] = [...definition.path].reverse();
    const target = parents.reverse().reduce((current, key) => current[key], draft);
    target[last] = value;
  };
  const formatValue = (definition) => {
    const value = propertyValue(definition);
    if (definition.node) return value;
    if (typeof value === "number") return Number.isInteger(value) ? String(value) : value.toFixed(6);
    return String(value);
  };
  const isDefault = (definition) =>
    Object.hasOwn(definition, "defaultValue") &&
    propertyValue(definition) === definition.defaultValue;

  const editor = document.createElement("div");
  editor.className = "generic-property-editor";
  const tableFrame = document.createElement("div");
  tableFrame.className = "property-table-frame";
  const table = document.createElement("table");
  table.className = "property-table";
  const head = document.createElement("thead");
  const header = document.createElement("tr");
  for (const label of ["Name", "R", "Type", "Value"]) {
    const cell = document.createElement("th");
    cell.textContent = label;
    header.append(cell);
  }
  head.append(header);
  const body = document.createElement("tbody");
  const rows = new Map();
  const valueCells = new Map();
  for (const definition of definitions) {
    const row = document.createElement("tr");
    row.tabIndex = 0;
    const name = document.createElement("td");
    name.textContent = definition.name;
    if (definition.level) name.className = "property-child";
    const readonly = document.createElement("td");
    readonly.textContent = definition.readonly ? "R" : "";
    readonly.className = "property-readonly";
    const type = document.createElement("td");
    type.textContent = definition.type;
    const value = document.createElement("td");
    value.textContent = formatValue(definition);
    row.append(name, readonly, type, value);
    rows.set(definition, row);
    valueCells.set(definition, value);
    body.append(row);
  }
  table.append(head, body);
  tableFrame.append(table);

  const detail = document.createElement("div");
  detail.className = "property-detail";
  const nameValue = document.createElement("input");
  nameValue.readOnly = true;
  const typeValue = document.createElement("input");
  typeValue.readOnly = true;
  const controlHost = document.createElement("div");
  controlHost.className = "property-control-host";
  const defaultLabel = document.createElement("label");
  defaultLabel.className = "property-default";
  const defaultCheck = document.createElement("input");
  defaultCheck.type = "checkbox";
  const defaultText = document.createElement("span");
  defaultText.textContent = "default";
  defaultLabel.append(defaultCheck, defaultText);
  for (const [label, control] of [
    ["Name:", nameValue],
    ["Type:", typeValue],
  ]) {
    const caption = document.createElement("label");
    caption.textContent = label;
    detail.append(caption, control);
  }
  const valueCaption = document.createElement("label");
  valueCaption.textContent = "Value:";
  const valueRow = document.createElement("div");
  valueRow.className = "property-value-row";
  valueRow.append(defaultLabel, controlHost);
  detail.append(valueCaption, valueRow);

  let applyButton;
  const markDirty = () => {
    dirty = true;
    applyButton.disabled = false;
  };
  const refreshValueCell = (definition) => {
    valueCells.get(definition).textContent = formatValue(definition);
  };
  const selectDefinition = (definition) => {
    selected = definition;
    for (const [item, row] of rows) row.classList.toggle("selected", item === definition);
    nameValue.value = definition.name;
    typeValue.value = definition.type;
    defaultCheck.disabled =
      definition.readonly || !Object.hasOwn(definition, "defaultValue");
    defaultCheck.checked = isDefault(definition);
    controlHost.replaceChildren();
    let control;
    const current = propertyValue(definition);
    if (definition.type === "color") {
      control = document.createElement("div");
      control.className = "property-color-editor";
      const text = document.createElement("input");
      text.value = current;
      text.spellcheck = false;
      const picker = document.createElement("input");
      picker.type = "color";
      picker.value = /^#[0-9a-f]{6}$/i.test(current) ? current : "#000000";
      const update = (value) => {
        setPropertyValue(definition, value);
        defaultCheck.checked = isDefault(definition);
        refreshValueCell(definition);
        markDirty();
      };
      text.addEventListener("input", () => {
        if (/^#[0-9a-f]{6}$/i.test(text.value)) picker.value = text.value;
        update(text.value);
      });
      picker.addEventListener("input", () => {
        text.value = picker.value;
        update(picker.value);
      });
      text.disabled = Boolean(definition.readonly);
      picker.disabled = Boolean(definition.readonly);
      control.append(text, picker);
    } else if (definition.type === "boolean") {
      control = document.createElement("input");
      control.type = "checkbox";
      control.checked = Boolean(current);
      control.addEventListener("change", () => {
        setPropertyValue(definition, control.checked);
        defaultCheck.checked = isDefault(definition);
        refreshValueCell(definition);
        markDirty();
      });
    } else if (definition.options) {
      control = document.createElement("select");
      for (const [value, label] of definition.options) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = label;
        control.append(option);
      }
      control.value = current;
      control.addEventListener("change", () => {
        setPropertyValue(definition, control.value);
        defaultCheck.checked = isDefault(definition);
        refreshValueCell(definition);
        markDirty();
      });
    } else {
      control = document.createElement("input");
      control.type = definition.type === "real" || definition.type === "integer" ? "number" : "text";
      if (control.type === "number") control.step = definition.type === "integer" ? "1" : "any";
      control.value = definition.node ? "<node>" : current;
      control.addEventListener("input", () => {
        const value = control.type === "number" ? Number(control.value) : control.value;
        if (control.type === "number" && !Number.isFinite(value)) return;
        setPropertyValue(definition, value);
        defaultCheck.checked = isDefault(definition);
        refreshValueCell(definition);
        markDirty();
      });
    }
    if (definition.type !== "color") control.disabled = Boolean(definition.readonly);
    controlHost.append(control);
  };
  for (const definition of definitions) {
    const choose = () => selectDefinition(definition);
    rows.get(definition).addEventListener("click", choose);
    rows.get(definition).addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        choose();
      }
    });
  }
  defaultCheck.addEventListener("change", () => {
    if (!selected || !defaultCheck.checked || defaultCheck.disabled) return;
    setPropertyValue(selected, selected.defaultValue);
    refreshValueCell(selected);
    selectDefinition(selected);
    markDirty();
  });

  const submit = async (closeAfter) => {
    if (!dirty) {
      if (closeAfter) referenceDialog.close();
      return;
    }
    setBusy(true);
    try {
      if (isScene) {
        snapshot = await invoke("set_scene_properties", {
          properties: {
            sceneUid: draft.uid,
            background: draft.background,
            aoEnabled: draft.aoEnabled,
            aoRadius: draft.aoRadius,
            aoIntensity: draft.aoIntensity,
            aoSlices: draft.aoSlices,
            aoSteps: draft.aoSteps,
            aoHalfRes: draft.aoHalfRes,
            aaMethod: draft.aaMethod,
            aaJitterLevel: draft.aaJitterLevel,
            onload: draft.onload,
            useColorProof: draft.useColorProof,
            iccFilename: draft.iccFilename,
            iccIntent: draft.iccIntent,
          },
        });
        draft = copyProperties(
          snapshot.scenes.find((scene) => scene.uid === draft.uid) || draft,
        );
        renderShell();
        await renderViewport();
      } else if (isCamera) {
        snapshot = await invoke("set_camera_properties", {
          oldName: originalCameraName,
          properties: {
            name: draft.name,
            zoom: draft.zoom,
            distance: draft.distance,
            slab: draft.slab,
            center: draft.center,
            rotation: draft.rotation,
            perspective: draft.perspective,
            transMms: draft.transMms,
            rotMms: draft.rotMms,
            stereoMode: draft.stereoMode,
            stereoDist: draft.stereoDist,
            swapStereoEyes: draft.swapStereoEyes,
            centerMark: draft.centerMark,
          },
        });
        originalCameraName = draft.name.trim();
        draft.name = originalCameraName;
        selectedCameraName = originalCameraName;
        workspaceSelectionKind = "camera";
        renderShell();
      } else {
        const size = viewportDimensions();
        const sequence = ++requestSequence;
        const result = await invoke("set_view_properties", {
          properties: {
            name: draft.name,
            zoom: draft.zoom,
            distance: draft.distance,
            slab: draft.slab,
            center: draft.center,
            rotation: draft.rotation,
            perspective: draft.perspective,
            transMms: draft.transMms,
            rotMms: draft.rotMms,
            stereoMode: draft.stereoMode,
            stereoDist: draft.stereoDist,
            swapStereoEyes: draft.swapStereoEyes,
            centerMark: draft.centerMark,
          },
          ...size,
        });
        snapshot = result.snapshot;
        draft = copyProperties(result.properties);
        centerMark = draft.centerMark === "crosshair" ? "cross" : draft.centerMark;
        applyCenterMark(centerMark);
        applyViewport(result.viewport, sequence);
        renderShell();
      }
      for (const definition of definitions) refreshValueCell(definition);
      dirty = false;
      applyButton.disabled = true;
      selectDefinition(selected);
      if (closeAfter) referenceDialog.close();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  };
  const cancel = button("Cancel", "", () => referenceDialog.close());
  applyButton = button("Apply", "", () => submit(false));
  applyButton.disabled = true;
  const ok = button("OK", "dialog-primary", () => submit(true));
  editor.append(tableFrame, detail);
  dialogContent.append(editor);
  dialogActions(cancel, applyButton, ok);
  selectDefinition(definitions[0]);
}

function viewPropertyEditor(info) {
  genericPropertyEditor(info, "view");
}

function cameraPropertyEditor(info) {
  genericPropertyEditor(info, "camera");
}

function scenePropertyEditor(info) {
  genericPropertyEditor(info, "scene");
}

function optionsEditor(source) {
  const draft = JSON.parse(JSON.stringify(source));
  const editor = document.createElement("div");
  editor.className = "options-editor";
  const tabs = document.createElement("nav");
  tabs.className = "options-tabs";
  tabs.setAttribute("aria-label", "Options panes");
  const paneHost = document.createElement("div");
  paneHost.className = "options-pane-host";
  const panes = new Map();

  const createPane = (name) => {
    const pane = document.createElement("section");
    pane.className = "options-pane";
    pane.dataset.optionsPane = name;
    pane.hidden = name !== "Misc";
    paneHost.append(pane);
    panes.set(name, pane);
    const tab = button(name, name === "Misc" ? "active" : "", () => {
      for (const [paneName, item] of panes) item.hidden = paneName !== name;
      tabs.querySelectorAll("button").forEach((item) => {
        item.classList.toggle("active", item.textContent === name);
      });
    });
    tab.dataset.optionsTab = name;
    tabs.append(tab);
    return pane;
  };
  const group = (title) => {
    const fieldset = document.createElement("fieldset");
    fieldset.className = "options-group";
    const legend = document.createElement("legend");
    legend.textContent = title;
    fieldset.append(legend);
    return fieldset;
  };
  const field = (host, label, control) => {
    const wrapper = document.createElement("label");
    wrapper.className = "options-field";
    const caption = document.createElement("span");
    caption.textContent = label;
    wrapper.append(caption, control);
    host.append(wrapper);
    return control;
  };
  const check = (host, label, checked, onChange) => {
    const wrapper = document.createElement("label");
    wrapper.className = "options-check";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = checked;
    input.addEventListener("change", () => onChange(input.checked));
    const caption = document.createElement("span");
    caption.textContent = label;
    wrapper.append(input, caption);
    host.append(wrapper);
    return input;
  };
  const select = (items, value) => {
    const control = document.createElement("select");
    for (const [optionValue, label] of items) {
      const option = document.createElement("option");
      option.value = optionValue;
      option.textContent = label;
      control.append(option);
    }
    control.value = value;
    return control;
  };

  const miscPane = createPane("Misc");
  const labelGroup = group("Atom label");
  const font = document.createElement("input");
  font.value = draft.atomLabelFont;
  font.addEventListener("input", () => {
    draft.atomLabelFont = font.value;
    updateSample();
  });
  field(labelGroup, "Font name", font);
  const size = document.createElement("input");
  size.type = "number";
  size.min = "4";
  size.max = "144";
  size.step = "1";
  size.value = draft.atomLabelSize;
  size.addEventListener("input", () => {
    draft.atomLabelSize = Number(size.value);
    updateSample();
  });
  field(labelGroup, "Font size", size);
  const color = document.createElement("input");
  color.type = "color";
  const namedColors = { yellow: "#ffff00", white: "#ffffff", black: "#000000" };
  color.value = namedColors[String(draft.atomLabelColor).toLowerCase()] ||
    (/^#[0-9a-f]{6}$/i.test(draft.atomLabelColor) ? draft.atomLabelColor : "#ffff00");
  color.addEventListener("input", () => {
    draft.atomLabelColor = color.value;
    updateSample();
  });
  field(labelGroup, "Color", color);
  const styleRow = document.createElement("div");
  styleRow.className = "options-check-row";
  check(styleRow, "Italic", draft.atomLabelItalic, (value) => {
    draft.atomLabelItalic = value;
    updateSample();
  });
  check(styleRow, "Bold", draft.atomLabelBold, (value) => {
    draft.atomLabelBold = value;
    updateSample();
  });
  labelGroup.append(styleRow);
  const sample = document.createElement("div");
  sample.className = "atom-label-sample";
  sample.textContent = "Atom label sample: CA 123";
  labelGroup.append(sample);
  function updateSample() {
    sample.style.fontFamily = draft.atomLabelFont;
    sample.style.fontSize = `${Math.max(4, Number(draft.atomLabelSize) || 12)}px`;
    sample.style.color = draft.atomLabelColor;
    sample.style.fontStyle = draft.atomLabelItalic ? "italic" : "normal";
    sample.style.fontWeight = draft.atomLabelBold ? "bold" : "normal";
  }
  updateSample();

  const miscGroup = group("Misc settings");
  const language = select(
    [
      ["en-US", "English (en-US)"],
      ["ja", "Japanese (ja)"],
    ],
    draft.uiLanguage,
  );
  language.addEventListener("change", () => {
    draft.uiLanguage = language.value;
  });
  field(miscGroup, "UI language", language);
  check(miscGroup, "Use HiDPI rendering", draft.useHidpi, (value) => {
    draft.useHidpi = value;
  });
  miscPane.append(labelGroup, miscGroup);

  const keyPane = createPane("Key");
  const keyGroup = group("Key bindings");
  const keyTable = document.createElement("table");
  keyTable.className = "shortcut-table";
  keyTable.innerHTML = "<thead><tr><th>Name</th><th>Key</th><th>Ctrl</th><th>Alt</th><th>Shift</th></tr></thead>";
  const keyBody = document.createElement("tbody");
  keyTable.append(keyBody);
  const shortcutNames = {
    "view-trans-plusx": "Translate view +X",
    "view-trans-minusx": "Translate view −X",
  };
  let selectedShortcut = 0;
  const shortcutDetail = document.createElement("div");
  shortcutDetail.className = "shortcut-detail";
  const keySelect = select(
    ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].map((key) => [key, key]),
    draft.shortcuts[0]?.key,
  );
  const ctrl = document.createElement("input");
  ctrl.type = "checkbox";
  const alt = document.createElement("input");
  alt.type = "checkbox";
  const shift = document.createElement("input");
  shift.type = "checkbox";
  const refreshShortcutDetail = () => {
    const shortcut = draft.shortcuts[selectedShortcut];
    keySelect.value = shortcut.key;
    ctrl.checked = shortcut.ctrl;
    alt.checked = shortcut.alt;
    shift.checked = shortcut.shift;
  };
  const refreshShortcutRows = () => {
    keyBody.replaceChildren();
    draft.shortcuts.forEach((shortcut, index) => {
      const row = document.createElement("tr");
      row.classList.toggle("selected", index === selectedShortcut);
      row.tabIndex = 0;
      for (const value of [
        shortcutNames[shortcut.id] || shortcut.id,
        shortcut.key,
        shortcut.ctrl ? "✓" : "",
        shortcut.alt ? "✓" : "",
        shortcut.shift ? "✓" : "",
      ]) {
        const cell = document.createElement("td");
        cell.textContent = value;
        row.append(cell);
      }
      const choose = () => {
        selectedShortcut = index;
        refreshShortcutRows();
        refreshShortcutDetail();
      };
      row.addEventListener("click", choose);
      row.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") choose();
      });
      keyBody.append(row);
    });
  };
  const updateShortcut = () => {
    const shortcut = draft.shortcuts[selectedShortcut];
    shortcut.key = keySelect.value;
    shortcut.ctrl = ctrl.checked;
    shortcut.alt = alt.checked;
    shortcut.shift = shift.checked;
    refreshShortcutRows();
  };
  keySelect.addEventListener("change", updateShortcut);
  ctrl.addEventListener("change", updateShortcut);
  alt.addEventListener("change", updateShortcut);
  shift.addEventListener("change", updateShortcut);
  field(shortcutDetail, "Key", keySelect);
  for (const [label, control] of [["Ctrl", ctrl], ["Alt", alt], ["Shift", shift]]) {
    const wrapper = document.createElement("label");
    wrapper.className = "options-check";
    const caption = document.createElement("span");
    caption.textContent = label;
    wrapper.append(control, caption);
    shortcutDetail.append(wrapper);
  }
  refreshShortcutRows();
  refreshShortcutDetail();
  keyGroup.append(keyTable, shortcutDetail);
  keyPane.append(keyGroup);

  const mousePane = createPane("Mouse");
  const presetGroup = group("Mouse configuration");
  const preset = select(
    [
      ["DefaultViewInConf", "Default"],
      ["CootViewInConf", "Coot"],
      ["PymolViewInConf", "Pymol"],
      ["IphoneViewInConf", "iPhone"],
      ["TrackpadViewInConf", "Trackpad"],
    ],
    draft.mousePreset,
  );
  preset.addEventListener("change", () => {
    draft.mousePreset = preset.value;
  });
  field(presetGroup, "Style", preset);
  const trackball = document.createElement("input");
  trackball.type = "number";
  trackball.min = "0.1";
  trackball.max = "10";
  trackball.step = "0.1";
  trackball.value = draft.trackballRadius;
  trackball.addEventListener("input", () => {
    draft.trackballRadius = Number(trackball.value);
  });
  field(presetGroup, "Trackball radius", trackball);
  const hitPrecision = document.createElement("input");
  hitPrecision.type = "number";
  hitPrecision.min = "0";
  hitPrecision.max = "100";
  hitPrecision.step = "1";
  hitPrecision.value = draft.hitPrecision;
  hitPrecision.addEventListener("input", () => {
    draft.hitPrecision = Number(hitPrecision.value);
  });
  field(presetGroup, "Hit precision", hitPrecision);
  check(presetGroup, "Momentum scroll", draft.momentumScroll, (value) => {
    draft.momentumScroll = value;
  });
  check(presetGroup, "Multitouch trackpad", draft.multitouchPad, (value) => {
    draft.multitouchPad = value;
  });
  check(presetGroup, "Emulate right mouse button", draft.emulateRightButton, (value) => {
    draft.emulateRightButton = value;
  });
  mousePane.append(presetGroup);

  const cancel = button("Cancel", "", () => referenceDialog.close());
  const ok = button("OK", "dialog-primary", async () => {
    setBusy(true);
    try {
      const result = await invoke("set_preferences", { preferences: draft });
      preferences = result.preferences;
      recordStatus(result.status);
      referenceDialog.close();
      scheduleRender(0);
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  });
  editor.append(tabs, paneHost);
  dialogContent.append(editor);
  dialogActions(cancel, ok);
}

function workspacePropertyPage(tabNames = ["Common"]) {
  const editor = document.createElement("div");
  editor.className = "workspace-property-editor";
  const tabs = document.createElement("div");
  tabs.className = "property-dialog-tabs";
  tabs.setAttribute("role", "tablist");
  const pageHost = document.createElement("div");
  pageHost.className = "property-dialog-pages";
  const panes = [];
  const tabButtons = [];
  const activate = (index) => {
    tabButtons.forEach((tab, tabIndex) => {
      const active = tabIndex === index;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", String(active));
      tab.tabIndex = active ? 0 : -1;
      panes[tabIndex].hidden = !active;
    });
  };
  tabNames.forEach((name, index) => {
    const tab = button(name, index === 0 ? "active" : "", () => activate(index));
    tab.type = "button";
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-selected", String(index === 0));
    const pane = document.createElement("section");
    pane.className = "property-common-page";
    pane.setAttribute("role", "tabpanel");
    pane.hidden = index !== 0;
    tabButtons.push(tab);
    panes.push(pane);
    tabs.append(tab);
    pageHost.append(pane);
  });
  editor.append(tabs, pageHost);
  return { editor, page: panes[0], panes, activate };
}

function propertyField(host, label, control) {
  const wrapper = document.createElement("label");
  wrapper.className = "property-common-field";
  const caption = document.createElement("span");
  caption.textContent = label;
  wrapper.append(caption, control);
  host.append(wrapper);
  return control;
}

function propertyCheckbox(host, label, checked) {
  const wrapper = document.createElement("label");
  wrapper.className = "property-common-check";
  const input = document.createElement("input");
  input.type = "checkbox";
  input.checked = checked;
  const caption = document.createElement("span");
  caption.textContent = label;
  wrapper.append(input, caption);
  host.append(wrapper);
  return input;
}

function objectPropertiesEditor(object) {
  const densityMap = Array.isArray(object.counts);
  const surfaceObject = object.className === "MolSurfObj";
  const { editor, page } = workspacePropertyPage();
  const name = document.createElement("input");
  name.value = object.name;
  propertyField(page, "Name:", name);
  const selection = document.createElement("input");
  selection.value = "(not available)";
  selection.disabled = true;
  propertyField(page, "Selection:", selection);
  const checks = document.createElement("div");
  checks.className = "property-common-checks";
  const visible = propertyCheckbox(checks, "Visible", object.visible);
  const locked = propertyCheckbox(checks, "Locked", object.locked);
  page.append(checks);
  const linked = document.createElement("input");
  linked.disabled = true;
  linked.value =
    !object.source || object.source.startsWith("datachunk:")
      ? "(embeded)"
      : `${object.source}${object.sourceType || object.format ? `  (${object.sourceType || object.format})` : ""}`;
  propertyField(page, "Linked:", linked);

  const submit = async () => {
    const changed = await mutate(
      densityMap
        ? "set_density_map_properties"
        : surfaceObject
          ? "set_surface_properties"
          : "set_object_properties",
      {
        properties: {
          objectUid: object.uid,
          name: name.value,
          visible: visible.checked,
          locked: locked.checked,
        },
      },
    );
    if (changed) referenceDialog.close();
  };
  const cancel = button("Cancel", "", () => referenceDialog.close());
  const ok = button("OK", "dialog-primary", submit);
  dialogContent.append(editor);
  dialogActions(cancel, ok);
}

function rendererPropertiesEditor(object, renderer, styleOptions = []) {
  const densityMap = Array.isArray(object.counts);
  const surfaceObject = object.className === "MolSurfObj";
  const rendererGroup = renderer.rendererType === "*group";
  const directSurfaceLike = ["dsurface", "dsurf2"].includes(renderer.rendererType);
  const surfaceLike = surfaceObject || directSurfaceLike;
  const simpleLike = ["simple", "trace"].includes(renderer.rendererType);
  const splineLike = renderer.rendererType === "spline";
  const nameLabelLike = ["*namelabel", "name_label"].includes(renderer.rendererType);
  const selectionLike = ["*selection", "selection"].includes(renderer.rendererType);
  const atomIntrLike = renderer.rendererType === "atomintr";
  const anisouLike = renderer.rendererType === "anisou";
  const ballStickLike = ["ballstick", "anisou"].includes(renderer.rendererType);
  const cpkLike = renderer.rendererType === "cpk";
  const disorderLike = renderer.rendererType === "disorder";
  const tubeLike = renderer.rendererType === "tube";
  const nuclLike = renderer.rendererType === "nucl";
  const ribbonLike = renderer.rendererType === "ribbon";
  const cartoonLike = renderer.rendererType === "cartoon";
  const contourLike = densityMap && ["contour", "gpu_mapmesh"].includes(renderer.rendererType);
  const isosurfLike = densityMap && renderer.rendererType === "isosurf";
  const volumeLike = densityMap && renderer.rendererType === "gpu_mapvol";
  if (rendererGroup) {
    dialogTitle.textContent = "Renderer group properties";
  } else if (renderer.rendererType === "simple") {
    dialogTitle.textContent = "Simple Rend Properties";
  } else if (renderer.rendererType === "trace") {
    dialogTitle.textContent = "Trace renderer properties";
  } else if (splineLike) {
    dialogTitle.textContent = "Spline renderer properties";
  } else if (nameLabelLike) {
    dialogTitle.textContent = "Name label renderer properties";
  } else if (selectionLike) {
    dialogTitle.textContent = "Selection renderer properties";
  } else if (atomIntrLike) {
    dialogTitle.textContent = "Interaction renderer properties";
  } else if (anisouLike) {
    dialogTitle.textContent = "Anisotropic U renderer properties";
  } else if (ballStickLike) {
    dialogTitle.textContent = "Ballstick renderer properties";
  } else if (cpkLike) {
    dialogTitle.textContent = "CPK Rend Properties";
  } else if (disorderLike) {
    dialogTitle.textContent = "Disorder renderer properties";
  } else if (tubeLike) {
    dialogTitle.textContent = "Tube Rend Properties";
  } else if (nuclLike) {
    dialogTitle.textContent = "Nucl renderer properties";
  } else if (ribbonLike) {
    dialogTitle.textContent = "Ribbon representation properties";
  } else if (cartoonLike) {
    dialogTitle.textContent = "Cartoon representation properties";
  } else if (contourLike) {
    dialogTitle.textContent = "Contour renderer properties";
  } else if (isosurfLike) {
    dialogTitle.textContent = "Isosurf renderer properties";
  } else if (volumeLike) {
    dialogTitle.textContent = "Volume renderer properties";
  } else if (surfaceLike) {
    dialogTitle.textContent = "MolSurf Rend Properties";
  }
  const tabNames = simpleLike
    ? [renderer.rendererType === "trace" ? "Trace" : "Simple"]
    : splineLike
      ? ["Common", "Spline"]
    : nameLabelLike
      ? ["Label"]
    : selectionLike
      ? ["Selection"]
    : atomIntrLike
      ? ["Common", "Interaction"]
      : anisouLike
      ? ["Common", "Ball & Stick", "Anisotropic U"]
      : ballStickLike
      ? ["Common", "Ball & Stick"]
      : cpkLike
        ? ["Common", "Atom radii"]
        : disorderLike
          ? ["Common", "Disorder"]
          : tubeLike
            ? ["Common", "Tube"]
            : nuclLike
              ? ["Common", "Nucleic acid", "Tube"]
              : ribbonLike
                ? ["Common", "Helix", "Sheet", "Coil"]
                : cartoonLike
                  ? ["Common", "Cartoon", "Helix", "Sheet", "Coil"]
                  : contourLike || isosurfLike || volumeLike
                    ? ["Common", "Map"]
                  : surfaceLike
                    ? ["Common", "MolSurf"]
                  : ["Common"];
  const { editor, page, panes } = workspacePropertyPage(tabNames);
  const specificControls = [];
  const specificInputs = new Map();
  const specificDefaults = new Map();
  const specificReaders = new Map();
  const specificResetters = [];
  const addSpecificNumber = (host, key, label, minimum, maximum, step, fallback) => {
    const input = document.createElement("input");
    input.type = "number";
    input.min = String(minimum);
    input.max = String(maximum);
    input.step = String(step);
    input.value = renderer.specific?.[key] ?? fallback;
    propertyField(host, label, input);
    specificInputs.set(key, input);
    specificDefaults.set(key, String(fallback));
    specificControls.push(input);
    return input;
  };
  const addSpecificSelect = (host, key, label, choices, fallback) => {
    const select = document.createElement("select");
    for (const [value, caption] of choices) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = caption;
      select.append(option);
    }
    select.value = renderer.specific?.[key] ?? fallback;
    propertyField(host, label, select);
    specificInputs.set(key, select);
    specificDefaults.set(key, fallback);
    specificControls.push(select);
    return select;
  };
  const addSpecificCheckbox = (host, key, label, fallback) => {
    const checked = (renderer.specific?.[key] ?? String(fallback)) === "true";
    const input = propertyCheckbox(host, label, checked);
    specificInputs.set(key, input);
    specificDefaults.set(key, String(fallback));
    specificControls.push(input);
    return input;
  };
  const addSpecificText = (host, key, label, fallback) => {
    const input = document.createElement("input");
    input.value = renderer.specific?.[key] ?? fallback;
    input.spellcheck = false;
    propertyField(host, label, input);
    specificInputs.set(key, input);
    specificDefaults.set(key, fallback);
    specificControls.push(input);
    return input;
  };
  const addSpecificColor = (host, key, label, fallback, pickerFallback = fallback) => {
    const row = document.createElement("div");
    row.className = "edge-color-editor";
    const input = document.createElement("input");
    input.value = renderer.specific?.[key] ?? fallback;
    input.spellcheck = false;
    const picker = document.createElement("input");
    picker.type = "color";
    picker.value = /^#[0-9a-f]{6}$/i.test(input.value) ? input.value : pickerFallback;
    input.addEventListener("input", () => {
      if (/^#[0-9a-f]{6}$/i.test(input.value)) picker.value = input.value;
    });
    picker.addEventListener("input", () => {
      input.value = picker.value;
    });
    row.append(input, picker);
    propertyField(host, label, row);
    specificInputs.set(key, input);
    specificDefaults.set(key, fallback);
    specificControls.push(input, picker);
    return { input, picker };
  };
  let lineWidth = null;
  if (simpleLike || splineLike) {
    const line = document.createElement("fieldset");
    line.className = "property-common-group renderer-specific-group";
    const legend = document.createElement("legend");
    legend.textContent = splineLike
      ? "Spline"
      : renderer.rendererType === "trace"
        ? "Trace"
        : "Simple";
    line.append(legend);
    lineWidth = document.createElement("input");
    lineWidth.type = "number";
    lineWidth.min = "0.2";
    lineWidth.max = "10";
    lineWidth.step = "0.2";
    lineWidth.value = renderer.lineWidth;
    propertyField(line, "Line width:", lineWidth);
    (splineLike ? panes[1] : page).append(line);
    specificControls.push(lineWidth);
    if (splineLike) {
      addSpecificNumber(line, "axialdetail", "Axial detail:", 1, 100, 1, "6");
      addSpecificNumber(line, "smooth", "Smoothing:", -1000, 1000, 0.05, "0");
      addSpecificCheckbox(line, "smoothcolor", "Smooth colors", true);
      addSpecificText(line, "pivotatom", "Pivot atom:", "");
    }
  }
  if (selectionLike) {
    const selectionGroup = document.createElement("fieldset");
    selectionGroup.className = "property-common-group renderer-specific-group";
    const selectionLegend = document.createElement("legend");
    selectionLegend.textContent = "Selection overlay";
    selectionGroup.append(selectionLegend);
    addSpecificSelect(
      selectionGroup,
      "mode",
      "Mode:",
      [
        ["stick", "Stick"],
        ["point", "Point"],
      ],
      "stick",
    );
    addSpecificNumber(selectionGroup, "linew", "Line width:", 0, 20, 0.2, "3");
    addSpecificNumber(selectionGroup, "dispx", "X displacement:", -100, 100, 0.5, "3");
    addSpecificNumber(selectionGroup, "dispy", "Y displacement:", -100, 100, 0.5, "3");
    addSpecificColor(
      selectionGroup,
      "color",
      "Color:",
      "rgba(0,255,255,0.5)",
      "#00ffff",
    );
    page.append(selectionGroup);
  }
  if (nameLabelLike) {
    const labelGroup = document.createElement("fieldset");
    labelGroup.className = "property-common-group renderer-specific-group";
    const labelLegend = document.createElement("legend");
    labelLegend.textContent = "Atom labels";
    labelGroup.append(labelLegend);
    addSpecificNumber(labelGroup, "maxlabel", "Maximum labels:", 0, 1000000, 1, "50");
    addSpecificNumber(labelGroup, "dispx", "X displacement:", -100, 100, 0.5, "0");
    addSpecificNumber(labelGroup, "dispy", "Y displacement:", -100, 100, 0.5, "0");
    addSpecificColor(labelGroup, "color", "Color:", "#ffff00");
    addSpecificNumber(labelGroup, "font_size", "Font size:", 1, 512, 0.5, "12");
    addSpecificText(labelGroup, "font_name", "Font family:", "sans-serif");
    addSpecificSelect(
      labelGroup,
      "font_style",
      "Font style:",
      [
        ["normal", "Normal"],
        ["italic", "Italic"],
        ["oblique", "Oblique"],
      ],
      "normal",
    );
    addSpecificSelect(
      labelGroup,
      "font_weight",
      "Font weight:",
      [
        ["normal", "Normal"],
        ["bold", "Bold"],
      ],
      "normal",
    );
    page.append(labelGroup);
  }
  const basic = document.createElement("fieldset");
  basic.className = "property-common-group";
  const basicLegend = document.createElement("legend");
  basicLegend.textContent = "Basic settings";
  basic.append(basicLegend);
  const name = document.createElement("input");
  name.value = renderer.name;
  propertyField(basic, "Name:", name);
  const selection = document.createElement("input");
  selection.value = densityMap || rendererGroup || nameLabelLike ? "(not available)" : renderer.selection || "*";
  selection.disabled = densityMap || rendererGroup || selectionLike || nameLabelLike;
  selection.spellcheck = false;
  propertyField(basic, "Selection:", selection);
  const checks = document.createElement("div");
  checks.className = "property-common-checks";
  const visible = propertyCheckbox(checks, "Visible", renderer.visible);
  const locked = propertyCheckbox(checks, "Locked", renderer.locked);
  basic.append(checks);
  const styleEditor = document.createElement("div");
  styleEditor.className = "renderer-style-editor";
  const style = document.createElement("input");
  style.value = renderer.style || "";
  style.spellcheck = false;
  const stylePreset = document.createElement("select");
  const presetPlaceholder = document.createElement("option");
  presetPlaceholder.value = "";
  presetPlaceholder.textContent = "Add preset";
  stylePreset.append(presetPlaceholder);
  for (const option of styleOptions) {
    const element = document.createElement("option");
    element.value = option.name;
    element.textContent = option.description || option.name;
    element.title = option.name;
    stylePreset.append(element);
  }
  stylePreset.addEventListener("change", () => {
    const next = stylePreset.value;
    if (!next) return;
    const names = style.value
      .split(",")
      .map((name) => name.trim())
      .filter(Boolean);
    if (!names.some((name) => name.toLowerCase() === next.toLowerCase())) {
      names.push(next);
      style.value = names.join(",");
      style.dispatchEvent(new Event("input"));
    }
    stylePreset.value = "";
  });
  styleEditor.append(style, stylePreset);
  propertyField(basic, "Style:", styleEditor);
  const material = document.createElement("select");
  const materialNames = ["", ...(snapshot.materialNames || [])];
  if (renderer.material && !materialNames.includes(renderer.material)) {
    materialNames.push(renderer.material);
  }
  for (const value of materialNames) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = value || "(none)";
    material.append(option);
  }
  material.value = renderer.material;
  propertyField(basic, "Material:", material);
  const alpha = document.createElement("input");
  alpha.type = "number";
  alpha.min = "0.1";
  alpha.max = "1";
  alpha.step = "0.1";
  alpha.value = renderer.alpha;
  propertyField(basic, "Opacity:", alpha);
  page.append(basic);

  const edge = document.createElement("fieldset");
  edge.className = "property-common-group edge-properties";
  const edgeLegend = document.createElement("legend");
  edgeLegend.textContent = "Edge lines:";
  const edgeType = document.createElement("select");
  for (const value of ["none", "edges", "silhouette"]) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = value;
    edgeType.append(option);
  }
  edgeType.value = renderer.edgeType;
  edgeLegend.append(edgeType);
  edge.append(edgeLegend);
  const edgeWidth = document.createElement("input");
  edgeWidth.type = "number";
  edgeWidth.min = "0";
  edgeWidth.max = "1";
  edgeWidth.step = "0.01";
  edgeWidth.value = renderer.edgeWidth;
  propertyField(edge, "Width:", edgeWidth);
  const edgeColorRow = document.createElement("div");
  edgeColorRow.className = "edge-color-editor";
  const edgeColor = document.createElement("input");
  edgeColor.value = renderer.edgeColor;
  edgeColor.spellcheck = false;
  const edgeColorPicker = document.createElement("input");
  edgeColorPicker.type = "color";
  edgeColorPicker.value = /^#[0-9a-f]{6}$/i.test(renderer.edgeColor)
    ? renderer.edgeColor
    : "#000000";
  edgeColor.addEventListener("input", () => {
    if (/^#[0-9a-f]{6}$/i.test(edgeColor.value)) edgeColorPicker.value = edgeColor.value;
  });
  edgeColorPicker.addEventListener("input", () => {
    edgeColor.value = edgeColorPicker.value;
  });
  edgeColorRow.append(edgeColor, edgeColorPicker);
  propertyField(edge, "Color:", edgeColorRow);
  page.append(edge);

  const syncEdgeState = () => {
    const disabled = edgeType.value === "none";
    edgeWidth.disabled = disabled;
    edgeColor.disabled = disabled;
    edgeColorPicker.disabled = disabled;
  };
  edgeType.addEventListener("change", syncEdgeState);
  syncEdgeState();

  let detail = null;
  let bondWidth = null;
  let sphereRadius = null;
  let showRing = null;
  let ringThickness = null;
  let ringColor = null;
  let ringColorPicker = null;
  if (ballStickLike) {
    const ballStickPage = panes[1];
    const geometry = document.createElement("fieldset");
    geometry.className = "property-common-group renderer-specific-group";
    const geometryLegend = document.createElement("legend");
    geometryLegend.textContent = "Ball & Stick settings";
    geometry.append(geometryLegend);
    detail = document.createElement("input");
    detail.type = "number";
    detail.min = "2";
    detail.max = "20";
    detail.step = "1";
    detail.value = renderer.detail;
    propertyField(geometry, "Detail:", detail);
    bondWidth = document.createElement("input");
    bondWidth.type = "number";
    bondWidth.min = "0";
    bondWidth.max = "3";
    bondWidth.step = "0.01";
    bondWidth.value = renderer.bondWidth;
    propertyField(geometry, "Bond width:", bondWidth);
    sphereRadius = document.createElement("input");
    sphereRadius.type = "number";
    sphereRadius.min = "0";
    sphereRadius.max = "3";
    sphereRadius.step = "0.01";
    sphereRadius.value = renderer.sphereRadius;
    propertyField(geometry, "Atom radius:", sphereRadius);
    ballStickPage.append(geometry);

    const ring = document.createElement("fieldset");
    ring.className = "property-common-group renderer-specific-group";
    const ringLegend = document.createElement("legend");
    ringLegend.textContent = "Ring settings";
    ring.append(ringLegend);
    showRing = propertyCheckbox(ring, "Show ring", renderer.showRing);
    ringThickness = document.createElement("input");
    ringThickness.type = "number";
    ringThickness.min = "0";
    ringThickness.max = "3";
    ringThickness.step = "0.01";
    ringThickness.value = renderer.ringThickness;
    propertyField(ring, "Thickness:", ringThickness);
    const ringColorRow = document.createElement("div");
    ringColorRow.className = "edge-color-editor";
    ringColor = document.createElement("input");
    ringColor.value = renderer.ringColor;
    ringColor.spellcheck = false;
    ringColorPicker = document.createElement("input");
    ringColorPicker.type = "color";
    ringColorPicker.value = /^#[0-9a-f]{6}$/i.test(renderer.ringColor)
      ? renderer.ringColor
      : "#ffffbf";
    ringColor.addEventListener("input", () => {
      if (/^#[0-9a-f]{6}$/i.test(ringColor.value)) ringColorPicker.value = ringColor.value;
    });
    ringColorPicker.addEventListener("input", () => {
      ringColor.value = ringColorPicker.value;
    });
    ringColorRow.append(ringColor, ringColorPicker);
    propertyField(ring, "Color:", ringColorRow);
    const syncRingState = () => {
      ringThickness.disabled = !showRing.checked;
      ringColor.disabled = !showRing.checked;
      ringColorPicker.disabled = !showRing.checked;
    };
    showRing.addEventListener("change", syncRingState);
    syncRingState();
    ballStickPage.append(ring);
    specificControls.push(
      detail,
      bondWidth,
      sphereRadius,
      showRing,
      ringThickness,
      ringColor,
      ringColorPicker,
    );
  }

  let disorderColorPicker = null;
  let syncAtomIntrState = () => {};
  if (atomIntrLike) {
    const interactionPage = panes[1];
    const basicSettings = document.createElement("fieldset");
    basicSettings.className = "property-common-group renderer-specific-group";
    const basicLegend = document.createElement("legend");
    basicLegend.textContent = "Interaction";
    basicSettings.append(basicLegend);
    const mode = addSpecificSelect(
      basicSettings,
      "mode",
      "Mode:",
      [
        ["simple", "Simple line"],
        ["fancy", "3D tube"],
      ],
      "fancy",
    );
    addSpecificCheckbox(basicSettings, "showlabel", "Show label", false);
    const width = addSpecificNumber(basicSettings, "width", "Width (Å):", 0, 5, 0.05, "0.1");
    const widthCaption = width.closest("label").querySelector("span");
    addSpecificColor(basicSettings, "color", "Color:", "#ffff00");
    interactionPage.append(basicSettings);

    const dashedGroup = document.createElement("fieldset");
    dashedGroup.className = "property-common-group renderer-specific-group";
    const stippleValues = Array.from({ length: 6 }, (_, index) =>
      Number(renderer.specific?.[`stipple${index}`] ?? (index < 2 ? 1 : -1)),
    );
    const dashed = propertyCheckbox(
      dashedGroup,
      "Dashed line",
      stippleValues.some((value) => value >= 0),
    );
    const stippleGrid = document.createElement("div");
    stippleGrid.className = "interaction-stipple-grid";
    const stippleInputs = [];
    const stippleDefaults = ["1", "1", "", "", "", ""];
    for (let index = 0; index < 6; index += 1) {
      const cell = document.createElement("label");
      const input = document.createElement("input");
      input.type = "number";
      input.min = "0";
      input.max = "100";
      input.step = "0.05";
      const current = stippleValues[index];
      input.value = current >= 0 ? String(current) : "";
      const caption = document.createElement("span");
      caption.textContent = index % 2 === 0 ? "dash" : "gap";
      cell.append(input, caption);
      stippleGrid.append(cell);
      specificInputs.set(`stipple${index}`, input);
      specificDefaults.set(`stipple${index}`, stippleDefaults[index]);
      specificReaders.set(`stipple${index}`, () =>
        dashed.checked && input.value.trim() !== "" ? input.value : "-1",
      );
      specificControls.push(input);
      stippleInputs.push(input);
    }
    dashedGroup.append(stippleGrid);
    specificControls.push(dashed);
    interactionPage.append(dashedGroup);

    const tubeOptions = document.createElement("fieldset");
    tubeOptions.className = "property-common-group renderer-specific-group";
    const tubeLegend = document.createElement("legend");
    tubeLegend.textContent = "3D tube options";
    tubeOptions.append(tubeLegend);
    const intrDetail = addSpecificNumber(tubeOptions, "detail", "Detail:", 0, 20, 1, "5");
    const startCap = addSpecificSelect(
      tubeOptions,
      "captype_start",
      "Start cap:",
      [
        ["flat", "Flat"],
        ["sphere", "Round"],
        ["arrow", "Arrow"],
      ],
      "sphere",
    );
    const endCap = addSpecificSelect(
      tubeOptions,
      "captype_end",
      "End cap:",
      [
        ["flat", "Flat"],
        ["sphere", "Round"],
        ["arrow", "Arrow"],
      ],
      "sphere",
    );
    interactionPage.append(tubeOptions);

    syncAtomIntrState = () => {
      const simple = mode.value === "simple";
      widthCaption.textContent = simple ? "Width (px):" : "Width (Å):";
      intrDetail.disabled = simple;
      startCap.disabled = simple;
      endCap.disabled = simple;
      for (const input of stippleInputs) input.disabled = !dashed.checked;
    };
    mode.addEventListener("change", syncAtomIntrState);
    dashed.addEventListener("change", () => {
      if (dashed.checked && (!stippleInputs[0].value || Number(stippleInputs[0].value) < 0)) {
        stippleInputs[0].value = "1";
      }
      syncAtomIntrState();
    });
    specificResetters.push(() => {
      dashed.checked = true;
    });
    syncAtomIntrState();
  }
  let syncAnisouState = () => {};
  if (anisouLike) {
    const anisouPage = panes[2];
    const display = document.createElement("fieldset");
    display.className = "property-common-group renderer-specific-group";
    const legend = document.createElement("legend");
    legend.textContent = "Thermal ellipsoid";
    display.append(legend);
    const drawDiscs = addSpecificCheckbox(display, "drawdisc", "Show principal discs", true);
    const discScale = addSpecificNumber(display, "discscale", "Disc scale:", 0, 10, 0.05, "1.1");
    const discThickness = addSpecificNumber(
      display,
      "discthick",
      "Disc thickness:",
      0,
      10,
      0.01,
      "0.1",
    );
    addSpecificNumber(
      display,
      "maxverts",
      "Maximum vertices:",
      1,
      100000000,
      1000,
      "1000000",
    );
    syncAnisouState = () => {
      discScale.disabled = !drawDiscs.checked;
      discThickness.disabled = !drawDiscs.checked;
    };
    drawDiscs.addEventListener("change", syncAnisouState);
    syncAnisouState();
    anisouPage.append(display);
  }
  if (cpkLike) {
    const radiiPage = panes[1];
    const radii = document.createElement("fieldset");
    radii.className = "property-common-group renderer-specific-group";
    const legend = document.createElement("legend");
    legend.textContent = "Atom radii";
    radii.append(legend);
    for (const [key, label, fallback] of [
      ["vdwr_C", "Carbon:", "1.7"],
      ["vdwr_N", "Nitrogen:", "1.55"],
      ["vdwr_O", "Oxygen:", "1.52"],
      ["vdwr_S", "Sulfur:", "1.8"],
      ["vdwr_P", "Phosphorus:", "1.8"],
      ["vdwr_H", "Hydrogen:", "1.2"],
      ["vdwr_X", "Others:", "1.7"],
    ]) {
      addSpecificNumber(radii, key, label, 0, 3, 0.01, fallback);
    }
    radiiPage.append(radii);
    const detailGroup = document.createElement("fieldset");
    detailGroup.className = "property-common-group renderer-specific-group compact-group";
    const detailLegend = document.createElement("legend");
    detailLegend.textContent = "Tessellation";
    detailGroup.append(detailLegend);
    addSpecificNumber(detailGroup, "detail", "Detail:", 2, 20, 1, "10");
    addSpecificSelect(
      detailGroup,
      "glrender_mode",
      "OpenGL rendering:",
      [
        ["default", "Default"],
        ["shader", "Shader"],
        ["vbo", "VBO"],
        ["glu", "GLU"],
      ],
      "default",
    );
    radiiPage.append(detailGroup);
  }

  if (disorderLike) {
    const disorderPage = panes[1];
    const settings = document.createElement("fieldset");
    settings.className = "property-common-group renderer-specific-group";
    const legend = document.createElement("legend");
    legend.textContent = "Disorder settings";
    settings.append(legend);
    const target = document.createElement("select");
    const targetNames = [
      "",
      ...object.renderers
        .filter((candidate) => ["tube", "ribbon", "cartoon", "nucl"].includes(candidate.rendererType))
        .map((candidate) => candidate.name),
    ];
    const currentTarget = renderer.specific?.target ?? "";
    if (currentTarget && !targetNames.includes(currentTarget)) targetNames.push(currentTarget);
    for (const value of targetNames) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value || "(none)";
      target.append(option);
    }
    target.value = currentTarget;
    propertyField(settings, "Target:", target);
    specificInputs.set("target", target);
    specificDefaults.set("target", "");
    specificControls.push(target);
    addSpecificNumber(settings, "detail", "Detail:", 2, 20, 1, "5");
    addSpecificNumber(settings, "width", "Dot size:", 0, 3, 0.1, "0.3");
    addSpecificNumber(settings, "dotsep", "Dot separation:", 0, 3, 0.1, "1");
    addSpecificNumber(settings, "loopsize", "Loop size:", 0, 10, 0.1, "2");
    addSpecificNumber(settings, "loopsize2", "Loop size 2:", -1, 10, 0.1, "-1");
    const colorRow = document.createElement("div");
    colorRow.className = "edge-color-editor";
    const color = document.createElement("input");
    color.value = renderer.specific?.defaultcolor ?? "#ffffff";
    color.spellcheck = false;
    disorderColorPicker = document.createElement("input");
    disorderColorPicker.type = "color";
    disorderColorPicker.value = /^#[0-9a-f]{6}$/i.test(color.value) ? color.value : "#ffffff";
    color.addEventListener("input", () => {
      if (/^#[0-9a-f]{6}$/i.test(color.value)) disorderColorPicker.value = color.value;
    });
    disorderColorPicker.addEventListener("input", () => {
      color.value = disorderColorPicker.value;
    });
    colorRow.append(color, disorderColorPicker);
    propertyField(settings, "Color:", colorRow);
    specificInputs.set("defaultcolor", color);
    specificDefaults.set("defaultcolor", "#ffffff");
    specificControls.push(color, disorderColorPicker);
    disorderPage.append(settings);
  }

  let syncDensityMapState = () => {};
  if (contourLike || isosurfLike || volumeLike) {
    const mapPage = panes[1];
    const settings = document.createElement("fieldset");
    settings.className = "property-common-group renderer-specific-group";
    const settingsLegend = document.createElement("legend");
    settingsLegend.textContent = "Map";
    settings.append(settingsLegend);

    const updateMode = document.createElement("select");
    for (const [value, label] of [
      ["none", "None"],
      ["auto", "Automatic"],
      ["drag", "Automatic (drag)"],
    ]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      updateMode.append(option);
    }
    const autoUpdate = renderer.specific?.autoupdate !== "false";
    const dragUpdate = renderer.specific?.dragupdate === "true";
    updateMode.value = autoUpdate ? (dragUpdate ? "drag" : "auto") : "none";
    propertyField(settings, "Center update:", updateMode);
    specificInputs.set("autoupdate", updateMode);
    specificDefaults.set("autoupdate", "auto");
    specificReaders.set("autoupdate", () => String(updateMode.value !== "none"));
    specificInputs.set("dragupdate", updateMode);
    specificDefaults.set("dragupdate", "auto");
    specificReaders.set("dragupdate", () => String(updateMode.value === "drag"));
    specificControls.push(updateMode);

    let mapWidth = null;
    let drawMode = null;
    if (isosurfLike) {
      drawMode = addSpecificSelect(
        settings,
        "drawmode",
        "Drawing Mode:",
        [
          ["fill", "fill"],
          ["line", "line"],
          ["point", "point"],
        ],
        "fill",
      );
      mapWidth = addSpecificNumber(
        settings,
        "width",
        "Line/Point size:",
        0.1,
        10,
        0.1,
        "1.2",
      );
      addSpecificNumber(settings, "max_grids", "Max grid size:", 10, 2000, 10, "100");
      addSpecificCheckbox(settings, "cullface", "Back-face culling", false);
    } else if (contourLike) {
      mapWidth = addSpecificNumber(settings, "width", "Line width:", 0.1, 10, 0.1, "1");
      addSpecificNumber(settings, "bufsize", "Buffer size:", 50, 200, 10, "100");
    } else {
      addSpecificSelect(
        settings,
        "xferfn_type",
        "Transfer function:",
        [
          ["rainbow1", "Rainbow"],
          ["auto2", "Automatic"],
          ["multigrad", "Multi-gradient"],
        ],
        "rainbow1",
      );
    }
    addSpecificCheckbox(settings, "use_pbc", "Use periodic boundary", true);
    mapPage.append(settings);

    const boundary = document.createElement("fieldset");
    boundary.className = "property-common-group renderer-specific-group";
    const boundaryLegend = document.createElement("legend");
    boundaryLegend.textContent = "Display boundary";
    boundary.append(boundaryLegend);
    const targetNames = ["", ...activeObjects().map((candidate) => candidate.name)];
    const currentTarget = renderer.specific?.bndry_molname ?? "";
    if (currentTarget && !targetNames.includes(currentTarget)) targetNames.push(currentTarget);
    const target = document.createElement("select");
    for (const value of targetNames) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value || "(none)";
      target.append(option);
    }
    target.value = currentTarget;
    propertyField(boundary, "Target:", target);
    const boundarySelection = document.createElement("input");
    boundarySelection.value = renderer.specific?.bndry_sel ?? "all";
    boundarySelection.spellcheck = false;
    propertyField(boundary, "Selection:", boundarySelection);
    const boundaryRange = addSpecificNumber(
      boundary,
      "bndry_rng",
      "Distance:",
      0.1,
      100,
      0.1,
      "5",
    );
    const limitDisplay = propertyCheckbox(
      boundary,
      "Limit display by",
      currentTarget.trim() !== "",
    );
    specificInputs.set("bndry_molname", target);
    specificDefaults.set("bndry_molname", "");
    specificReaders.set("bndry_molname", () =>
      limitDisplay.checked ? target.value : "",
    );
    specificInputs.set("bndry_sel", boundarySelection);
    specificDefaults.set("bndry_sel", "all");
    specificReaders.set("bndry_sel", () =>
      limitDisplay.checked ? boundarySelection.value.trim() || "all" : "",
    );
    specificControls.push(target, boundarySelection, limitDisplay);
    specificResetters.push(() => {
      limitDisplay.checked = false;
    });
    mapPage.append(boundary);

    syncDensityMapState = () => {
      const boundaryDisabled = !limitDisplay.checked;
      target.disabled = boundaryDisabled;
      boundarySelection.disabled = boundaryDisabled;
      boundaryRange.disabled = boundaryDisabled;
      if (drawMode && mapWidth) mapWidth.disabled = drawMode.value === "fill";
    };
    limitDisplay.addEventListener("change", syncDensityMapState);
    drawMode?.addEventListener("change", syncDensityMapState);
    syncDensityMapState();
  }

  let syncSurfaceState = () => {};
  if (surfaceLike) {
    const surfacePage = panes[1];
    const draw = document.createElement("fieldset");
    draw.className = "property-common-group renderer-specific-group";
    const drawLegend = document.createElement("legend");
    drawLegend.textContent = "Draw";
    draw.append(drawLegend);
    const drawMode = addSpecificSelect(
      draw,
      "drawmode",
      "Drawing Mode:",
      [
        ["fill", "fill"],
        ["line", "line"],
        ["point", "point"],
      ],
      "fill",
    );
    const surfaceWidth = addSpecificNumber(
      draw,
      "width",
      "Line/Point size:",
      0.1,
      10,
      0.1,
      "1.2",
    );
    let surfaceType;
    let surfaceDetail;
    if (directSurfaceLike) {
      surfaceType = addSpecificSelect(
        draw,
        "surftype",
        "Surface type:",
        [
          ["vdw", "van der Waals"],
          ["sas", "Solvent accessible"],
          ["ses", "Solvent excluded"],
        ],
        "ses",
      );
      surfaceDetail = addSpecificNumber(draw, "detail", "Detail:", 1, 21, 1, "6");
    } else {
      surfaceType = document.createElement("select");
      for (const [value, label] of [
        ["vdw", "van der Waals"],
        ["sas", "Solvent accessible"],
        ["ses", "Solvent excluded"],
      ]) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = label;
        surfaceType.append(option);
      }
      surfaceType.value = "vdw";
      surfaceType.disabled = true;
      propertyField(draw, "Surface type:", surfaceType);
      surfaceDetail = document.createElement("input");
      surfaceDetail.type = "number";
      surfaceDetail.min = "1";
      surfaceDetail.max = "21";
      surfaceDetail.step = "1";
      surfaceDetail.value = "1";
      surfaceDetail.disabled = true;
      propertyField(draw, "Detail:", surfaceDetail);
      specificControls.push(surfaceType, surfaceDetail);
    }
    surfacePage.append(draw);

    const selected = document.createElement("fieldset");
    selected.className = "property-common-group renderer-specific-group";
    const selectedLegend = document.createElement("legend");
    selectedLegend.textContent = "Show selected:";
    selected.append(selectedLegend);
    const target = document.createElement("select");
    const targetNames = ["", ...activeObjects().map((candidate) => candidate.name)];
    const currentTarget = renderer.specific?.target ?? "";
    if (currentTarget && !targetNames.includes(currentTarget)) targetNames.push(currentTarget);
    for (const value of targetNames) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value || "(none)";
      target.append(option);
    }
    target.value = currentTarget;
    target.disabled = directSurfaceLike;
    propertyField(selected, "Selection mol:", target);
    specificInputs.set("target", target);
    specificDefaults.set("target", "");
    specificControls.push(target);
    const showSelection = document.createElement("input");
    showSelection.value = renderer.specific?.showsel ?? "*";
    showSelection.spellcheck = false;
    propertyField(selected, "Selection:", showSelection);
    specificInputs.set("showsel", showSelection);
    specificDefaults.set("showsel", "*");
    specificControls.push(showSelection);
    surfacePage.append(selected);

    addSpecificSelect(
      surfacePage,
      "colormode",
      "Coloring mode:",
      directSurfaceLike
        ? [
            ["molecule", "By molecule"],
            ["potential", "By potential"],
          ]
        : [
            ["solid", "Solid color"],
            ["molecule", "By molecule"],
            ["potential", "By potential"],
          ],
      directSurfaceLike ? "molecule" : "solid",
    );
    syncSurfaceState = () => {
      surfaceWidth.disabled = drawMode.value === "fill";
    };
    drawMode.addEventListener("change", syncSurfaceState);
    syncSurfaceState();
  }

  let syncRibbonState = () => {};
  if (ribbonLike || cartoonLike) {
    const sectionChoices = [
      ["elliptical", "Elliptical"],
      ["roundsquare", "Rounded square"],
      ["rectangle", "Rectangle"],
      ["fancy1", "Fancy"],
    ];
    const junctionChoices = [
      ["smooth", "Smooth"],
      ["flat", "Flat"],
      ["arrow", "Arrow"],
    ];
    const stateSyncers = [];
    const addSpecificText = (host, key, label, fallback) => {
      const input = document.createElement("input");
      input.value = renderer.specific?.[key] ?? fallback;
      input.spellcheck = false;
      propertyField(host, label, input);
      specificInputs.set(key, input);
      specificDefaults.set(key, fallback);
      specificControls.push(input);
      return input;
    };
    const addPercentNumber = (host, key, label, minimum, maximum, fallback) => {
      const input = document.createElement("input");
      input.type = "number";
      input.min = String(minimum);
      input.max = String(maximum);
      input.step = "5";
      input.value = String(Number(renderer.specific?.[key] ?? fallback) * 100);
      propertyField(host, label, input);
      specificInputs.set(key, input);
      specificDefaults.set(key, String(Number(fallback) * 100));
      specificReaders.set(key, () => String(Number(input.value) / 100));
      specificControls.push(input);
      return input;
    };
    const addSectionEditor = (host, key, title) => {
      const group = document.createElement("fieldset");
      group.className = "property-common-group renderer-specific-group tube-section-group";
      const legend = document.createElement("legend");
      legend.textContent = title;
      group.append(legend);
      const type = addSpecificSelect(group, `${key}.type`, "Type:", sectionChoices, "elliptical");
      addSpecificNumber(group, `${key}.detail`, "Detail:", 2, 20, 1, "16");
      const width1 = addSpecificNumber(group, `${key}.width`, "Width 1:", 0.01, 10, 0.01, "0.35");
      const width2 = document.createElement("input");
      width2.type = "number";
      width2.min = "0.01";
      width2.max = "10";
      width2.step = "0.01";
      width2.value = String(
        Number(renderer.specific?.[`${key}.width`] ?? 0.35) *
          Number(renderer.specific?.[`${key}.tuber`] ?? 1),
      );
      propertyField(group, "Width 2:", width2);
      specificInputs.set(`${key}.tuber`, width2);
      specificDefaults.set(`${key}.tuber`, "0.35");
      specificReaders.set(`${key}.tuber`, () =>
        String(Number(width2.value) / Math.max(Number(width1.value), Number.EPSILON)),
      );
      specificControls.push(width2);
      const sharp = addSpecificNumber(
        group,
        `${key}.sharp`,
        "Sharpness:",
        0,
        1,
        0.05,
        "0.4",
      );
      sharp.classList.add("section-sharpness");
      const sync = () => {
        sharp.disabled = !["roundsquare", "fancy1"].includes(type.value);
      };
      type.addEventListener("change", sync);
      stateSyncers.push(sync);
      host.append(group);
      return group;
    };
    const addJunctionEditor = (host, key, title) => {
      const group = document.createElement("fieldset");
      group.className = "property-common-group renderer-specific-group compact-group";
      const legend = document.createElement("legend");
      legend.textContent = title;
      group.append(legend);
      const type = addSpecificSelect(group, `${key}.type`, "Type:", junctionChoices, "smooth");
      const gamma = addSpecificNumber(group, `${key}.gamma`, "Gamma:", 0.1, 10, 0.1, "2.2");
      const baseWidth = addPercentNumber(group, `${key}.basw`, "Base width (%):", 0, 100, "0");
      const arrowWidth = addPercentNumber(
        group,
        `${key}.arrow`,
        "Arrow width (%):",
        100,
        300,
        "1.8",
      );
      for (const control of [gamma, baseWidth, arrowWidth]) {
        control.classList.add("junction-parameter");
      }
      const sync = () => {
        const disabled = type.value !== "arrow";
        gamma.disabled = disabled;
        baseWidth.disabled = disabled;
        arrowWidth.disabled = disabled;
      };
      type.addEventListener("change", sync);
      stateSyncers.push(sync);
      host.append(group);
      return group;
    };
    const addBackboneEditor = (host, axialFallback, smoothColorFallback, capFallback) => {
      const spline = document.createElement("fieldset");
      spline.className = "property-common-group renderer-specific-group";
      const splineLegend = document.createElement("legend");
      splineLegend.textContent = "Backbone spline";
      spline.append(splineLegend);
      addSpecificNumber(spline, "axialdetail", "Axial detail:", 2, 20, 1, axialFallback);
      addSpecificCheckbox(spline, "smoothcolor", "Smooth color", smoothColorFallback);
      addSpecificCheckbox(spline, "segend_fade", "Fade at segment ends", false);
      addSpecificSelect(
        spline,
        "start_captype",
        "Start cap:",
        [
          ["flat", "Flat"],
          ["sphere", "Sphere"],
          ["none", "None"],
        ],
        capFallback,
      );
      addSpecificSelect(
        spline,
        "end_captype",
        "End cap:",
        [
          ["flat", "Flat"],
          ["sphere", "Sphere"],
          ["none", "None"],
        ],
        capFallback,
      );
      host.append(spline);

      const pivot = document.createElement("fieldset");
      pivot.className = "property-common-group renderer-specific-group pivot-atom-group";
      const pivotLegend = document.createElement("legend");
      pivotLegend.textContent = "Pivot atom";
      pivot.append(pivotLegend);
      const pivotName = addSpecificText(pivot, "pivotatom", "Atom name:", "");
      const pivotEnabled = propertyCheckbox(
        pivot,
        "Use a custom pivot atom",
        pivotName.value.trim() !== "",
      );
      specificReaders.set("pivotatom", () => (pivotEnabled.checked ? pivotName.value.trim() : ""));
      specificResetters.push(() => {
        pivotEnabled.checked = false;
      });
      specificControls.push(pivotEnabled);
      const sync = () => {
        pivotName.disabled = !pivotEnabled.checked;
      };
      pivotEnabled.addEventListener("change", sync);
      stateSyncers.push(sync);
      host.append(pivot);
    };

    if (ribbonLike) {
      addBackboneEditor(page, "6", true, "sphere");

      const helixPage = panes[1];
      addSectionEditor(helixPage, "helix", "Helix section");
      const helixSpline = document.createElement("fieldset");
      helixSpline.className = "property-common-group renderer-specific-group compact-group";
      const helixSplineLegend = document.createElement("legend");
      helixSplineLegend.textContent = "Helix spline";
      helixSpline.append(helixSplineLegend);
      addSpecificNumber(helixSpline, "helix_smooth", "Smoothing:", 0, 5, 0.1, "0");
      const useBackColor = addSpecificCheckbox(
        helixSpline,
        "helix_usebackcol",
        "Use back-face color",
        false,
      );
      const backColor = addSpecificColor(helixSpline, "helix_backcol", "Back-face color:", "#ffffff");
      const syncBackColor = () => {
        backColor.input.disabled = !useBackColor.checked;
        backColor.picker.disabled = !useBackColor.checked;
      };
      useBackColor.addEventListener("change", syncBackColor);
      stateSyncers.push(syncBackColor);
      helixPage.append(helixSpline);
      addJunctionEditor(helixPage, "helixhead", "Helix head");
      addJunctionEditor(helixPage, "helixtail", "Helix tail");

      const sheetPage = panes[2];
      addSectionEditor(sheetPage, "sheet", "Sheet section");
      const sheetSpline = document.createElement("fieldset");
      sheetSpline.className = "property-common-group renderer-specific-group compact-group";
      const sheetSplineLegend = document.createElement("legend");
      sheetSplineLegend.textContent = "Sheet spline";
      sheetSpline.append(sheetSplineLegend);
      addSpecificNumber(sheetSpline, "sheet_smooth", "Smoothing:", 0, 5, 0.1, "0.5");
      const useSideColor = addSpecificCheckbox(
        sheetSpline,
        "sheet_usesidecol",
        "Use side-face color",
        false,
      );
      const sideColor = addSpecificColor(sheetSpline, "sheet_sidecol", "Side-face color:", "#ffffff");
      const syncSideColor = () => {
        sideColor.input.disabled = !useSideColor.checked;
        sideColor.picker.disabled = !useSideColor.checked;
      };
      useSideColor.addEventListener("change", syncSideColor);
      stateSyncers.push(syncSideColor);
      sheetPage.append(sheetSpline);
      addJunctionEditor(sheetPage, "sheethead", "Sheet head");
      addJunctionEditor(sheetPage, "sheettail", "Sheet tail");

      const coilPage = panes[3];
      addSectionEditor(coilPage, "coil", "Coil section");
      const coilSpline = document.createElement("fieldset");
      coilSpline.className = "property-common-group renderer-specific-group compact-group";
      const coilSplineLegend = document.createElement("legend");
      coilSplineLegend.textContent = "Coil spline";
      coilSpline.append(coilSplineLegend);
      addSpecificNumber(coilSpline, "coil_smooth", "Smoothing:", 0, 5, 0.1, "0");
      coilPage.append(coilSpline);
    } else {
      const cartoonPage = panes[1];
      addBackboneEditor(cartoonPage, "8", false, "flat");
      const anchor = document.createElement("fieldset");
      anchor.className = "property-common-group renderer-specific-group";
      const anchorLegend = document.createElement("legend");
      anchorLegend.textContent = "Anchor selection";
      anchor.append(anchorLegend);
      addSpecificText(anchor, "anchor_sel", "Selection:", "");
      addSpecificNumber(anchor, "anchor_weight", "Weight:", 0, 20, 0.5, "10");
      cartoonPage.append(anchor);

      const helixPage = panes[2];
      const helixMode = document.createElement("fieldset");
      helixMode.className = "property-common-group renderer-specific-group";
      const helixModeLegend = document.createElement("legend");
      helixModeLegend.textContent = "Helix";
      helixMode.append(helixModeLegend);
      const ribbonHelix = addSpecificCheckbox(
        helixMode,
        "helix_ribbon",
        "Draw helix as ribbon",
        false,
      );
      addSpecificNumber(helixMode, "helix_smooth", "Smoothing:", -5, 5, 0.1, "3");
      helixPage.append(helixMode);

      const cylinder = document.createElement("fieldset");
      cylinder.className = "property-common-group renderer-specific-group";
      const cylinderLegend = document.createElement("legend");
      cylinderLegend.textContent = "Cylindrical helix";
      cylinder.append(cylinderLegend);
      const cylinderControls = [
        addSpecificSelect(
          cylinder,
          "helix_width_mode",
          "Width mode:",
          [
            ["const", "Constant"],
            ["average", "Average"],
            ["wavy", "Wavy"],
          ],
          "average",
        ),
        addSpecificNumber(cylinder, "helix_width", "Width:", 0.01, 10, 0.1, "2.3"),
        addSpecificNumber(cylinder, "helix_extend", "Extension:", 0, 3, 0.1, "0.5"),
        addSpecificNumber(cylinder, "helix_wsmooth", "Width smoothing:", -5, 5, 0.1, "5"),
        addSpecificNumber(cylinder, "helix_wplus", "Width adjustment:", -2, 3, 0.1, "0.2"),
      ];
      helixPage.append(cylinder);
      const ribbonSection = addSectionEditor(helixPage, "ribhelix", "Ribbon helix section");
      const ribbonHead = addJunctionEditor(helixPage, "ribhelix_head", "Ribbon helix head");
      const ribbonTail = addJunctionEditor(helixPage, "ribhelix_tail", "Ribbon helix tail");
      const syncHelixMode = () => {
        for (const control of cylinderControls) control.disabled = ribbonHelix.checked;
        const ribbonGroups = [ribbonSection, ribbonHead, ribbonTail];
        for (const group of ribbonGroups) {
          for (const control of group.querySelectorAll("input, select")) {
            control.disabled = !ribbonHelix.checked;
          }
        }
        if (ribbonHelix.checked) {
          const sectionSharp = ribbonSection.querySelector(".section-sharpness");
          const sectionType = ribbonSection.querySelector("select");
          sectionSharp.disabled = !["roundsquare", "fancy1"].includes(sectionType.value);
          for (const group of [ribbonHead, ribbonTail]) {
            const junctionDisabled = group.querySelector("select").value !== "arrow";
            for (const control of group.querySelectorAll(".junction-parameter")) {
              control.disabled = junctionDisabled;
            }
          }
        }
      };
      ribbonHelix.addEventListener("change", syncHelixMode);
      stateSyncers.push(syncHelixMode);

      const sheetPage = panes[3];
      addSectionEditor(sheetPage, "sheet", "Sheet section");
      const sheetSpline = document.createElement("fieldset");
      sheetSpline.className = "property-common-group renderer-specific-group compact-group";
      const sheetSplineLegend = document.createElement("legend");
      sheetSplineLegend.textContent = "Sheet spline";
      sheetSpline.append(sheetSplineLegend);
      addSpecificNumber(sheetSpline, "sheet_smooth", "Smoothing:", -5, 5, 0.1, "2");
      addSpecificNumber(sheetSpline, "sheet_wsmooth", "Width smoothing:", -5, 5, 0.1, "5");
      sheetPage.append(sheetSpline);
      addJunctionEditor(sheetPage, "sheethead", "Sheet head");

      const coilPage = panes[4];
      addSectionEditor(coilPage, "coil", "Coil section");
      const coilSpline = document.createElement("fieldset");
      coilSpline.className = "property-common-group renderer-specific-group compact-group";
      const coilSplineLegend = document.createElement("legend");
      coilSplineLegend.textContent = "Coil spline";
      coilSpline.append(coilSplineLegend);
      addSpecificNumber(coilSpline, "coil_smooth", "Smoothing:", -5, 5, 0.1, "-1");
      coilPage.append(coilSpline);
    }
    syncRibbonState = () => {
      for (const sync of stateSyncers) sync();
    };
    syncRibbonState();
  }

  let syncTubeState = () => {};
  let syncNuclState = () => {};
  if (tubeLike || nuclLike) {
    const tubePage = panes[nuclLike ? 2 : 1];
    const tubeControls = [];
    const rememberTubeControl = (control) => {
      tubeControls.push(control);
      return control;
    };

    const sectionGroup = document.createElement("fieldset");
    sectionGroup.className = "property-common-group renderer-specific-group tube-section-group";
    const sectionLegend = document.createElement("legend");
    sectionLegend.textContent = "Section";
    sectionGroup.append(sectionLegend);
    const sectionType = rememberTubeControl(
      addSpecificSelect(
        sectionGroup,
        "section.type",
        "Type:",
        [
          ["elliptical", "Elliptical"],
          ["roundsquare", "Rounded square"],
          ["rectangle", "Rectangle"],
          ["fancy1", "Fancy"],
        ],
        "elliptical",
      ),
    );
    rememberTubeControl(
      addSpecificNumber(sectionGroup, "section.detail", "Detail:", 2, 20, 1, "16"),
    );
    const sectionWidth = rememberTubeControl(
      addSpecificNumber(sectionGroup, "section.width", "Width 1:", 0.01, 10, 0.01, "0.35"),
    );
    const width2 = document.createElement("input");
    width2.type = "number";
    width2.min = "0.01";
    width2.max = "10";
    width2.step = "0.01";
    width2.value = String(
      Number(renderer.specific?.["section.width"] ?? 0.35) *
        Number(renderer.specific?.["section.tuber"] ?? 1),
    );
    propertyField(sectionGroup, "Width 2:", width2);
    specificInputs.set("section.tuber", width2);
    specificDefaults.set("section.tuber", "0.35");
    specificReaders.set("section.tuber", () =>
      String(Number(width2.value) / Math.max(Number(sectionWidth.value), Number.EPSILON)),
    );
    specificControls.push(width2);
    rememberTubeControl(width2);
    const sectionSharp = rememberTubeControl(
      addSpecificNumber(sectionGroup, "section.sharp", "Sharpness:", 0, 1, 0.05, "0.4"),
    );
    tubePage.append(sectionGroup);

    const splineGroup = document.createElement("fieldset");
    splineGroup.className = "property-common-group renderer-specific-group";
    const splineLegend = document.createElement("legend");
    splineLegend.textContent = "Spline";
    splineGroup.append(splineLegend);
    rememberTubeControl(
      addSpecificNumber(splineGroup, "axialdetail", "Axial detail:", 2, 20, 1, "6"),
    );
    rememberTubeControl(
      addSpecificNumber(splineGroup, "smooth", "Smoothing:", 0, 0.5, 0.05, "0"),
    );
    rememberTubeControl(addSpecificCheckbox(splineGroup, "smoothcolor", "Smooth color", true));
    rememberTubeControl(
      addSpecificCheckbox(splineGroup, "segend_fade", "Fade at segment ends", false),
    );
    const capType = rememberTubeControl(
      addSpecificSelect(
        splineGroup,
        "start_captype",
        "Cap type:",
        [
          ["flat", "Flat"],
          ["sphere", "Sphere"],
          ["none", "None"],
        ],
        "sphere",
      ),
    );
    specificInputs.set("end_captype", capType);
    specificDefaults.set("end_captype", "sphere");
    tubePage.append(splineGroup);

    const pivotGroup = document.createElement("fieldset");
    pivotGroup.className = "property-common-group renderer-specific-group pivot-atom-group";
    const pivotLegend = document.createElement("legend");
    pivotLegend.textContent = "Pivot atom";
    pivotGroup.append(pivotLegend);
    const pivotName = document.createElement("input");
    pivotName.value = renderer.specific?.pivotatom ?? "";
    pivotName.spellcheck = false;
    propertyField(pivotGroup, "Atom name:", pivotName);
    const pivotEnabled = propertyCheckbox(
      pivotGroup,
      "Use a custom pivot atom",
      pivotName.value.trim() !== "",
    );
    specificInputs.set("pivotatom", pivotName);
    specificDefaults.set("pivotatom", "");
    specificReaders.set("pivotatom", () => (pivotEnabled.checked ? pivotName.value.trim() : ""));
    specificResetters.push(() => {
      pivotEnabled.checked = false;
    });
    specificControls.push(pivotEnabled, pivotName);
    rememberTubeControl(pivotEnabled);
    rememberTubeControl(pivotName);
    tubePage.append(pivotGroup);

    const puttyGroup = document.createElement("fieldset");
    puttyGroup.className = "property-common-group renderer-specific-group";
    const puttyLegend = document.createElement("legend");
    puttyLegend.textContent = "Putty";
    puttyGroup.append(puttyLegend);
    const puttyMode = rememberTubeControl(
      addSpecificSelect(
        puttyGroup,
        "putty_mode",
        "Mode:",
        [
          ["none", "None"],
          ["linear1", "Linear"],
          ["scale1", "Scaled"],
        ],
        "none",
      ),
    );
    const puttyTarget = rememberTubeControl(
      addSpecificSelect(
        puttyGroup,
        "putty_tgt",
        "Target:",
        [
          ["bfac", "B-factor"],
          ["occ", "Occupancy"],
        ],
        "bfac",
      ),
    );
    const puttyLow = rememberTubeControl(
      addSpecificNumber(puttyGroup, "putty_loscl", "Low scale:", 0.01, 10, 0.1, "3"),
    );
    const puttyHigh = rememberTubeControl(
      addSpecificNumber(puttyGroup, "putty_hiscl", "High scale:", 0.01, 10, 0.1, "3"),
    );
    tubePage.append(puttyGroup);

    let showTube = null;
    if (nuclLike) {
      const nuclPage = panes[1];
      const displayGroup = document.createElement("fieldset");
      displayGroup.className = "property-common-group renderer-specific-group";
      const displayLegend = document.createElement("legend");
      displayLegend.textContent = "Nucleic acid";
      displayGroup.append(displayLegend);
      showTube = addSpecificCheckbox(displayGroup, "show_tube", "Show tube", true);
      const showBasePair = addSpecificCheckbox(
        displayGroup,
        "show_basepair",
        "Connect base pair",
        true,
      );
      nuclPage.append(displayGroup);

      const baseGroup = document.createElement("fieldset");
      baseGroup.className = "property-common-group renderer-specific-group";
      const baseLegend = document.createElement("legend");
      baseLegend.textContent = "Base";
      baseGroup.append(baseLegend);
      const baseType = addSpecificSelect(
        baseGroup,
        "base_type",
        "Type:",
        [
          ["basepair", "Base pair"],
          ["simple1", "Simple"],
          ["detail1", "Detailed 1"],
          ["detail2", "Detailed 2"],
        ],
        "basepair",
      );
      const baseDetail = addSpecificNumber(baseGroup, "base_detail", "Detail:", 2, 20, 1, "3");
      const baseSize = addSpecificNumber(baseGroup, "base_size", "Size:", 0, 10, 0.05, "0.5");
      const baseThickness = document.createElement("input");
      baseThickness.type = "number";
      baseThickness.min = "0";
      baseThickness.max = "100";
      baseThickness.step = "5";
      const currentBaseSize = Math.max(Number(renderer.specific?.base_size ?? 0.5), Number.EPSILON);
      baseThickness.value = String(
        (Number(renderer.specific?.base_thick ?? 0.5) * 100) / currentBaseSize,
      );
      propertyField(baseGroup, "Thickness (%):", baseThickness);
      specificInputs.set("base_thick", baseThickness);
      specificDefaults.set("base_thick", "100");
      specificReaders.set("base_thick", () =>
        String((Number(baseThickness.value) * Number(baseSize.value)) / 100),
      );
      specificControls.push(baseThickness);
      nuclPage.append(baseGroup);
      const baseControls = [baseType, baseDetail, baseSize, baseThickness];
      syncNuclState = () => {
        for (const control of baseControls) control.disabled = !showBasePair.checked;
      };
      showBasePair.addEventListener("change", syncNuclState);
    }

    syncTubeState = () => {
      const pageDisabled = showTube ? !showTube.checked : false;
      for (const control of tubeControls) control.disabled = pageDisabled;
      if (pageDisabled) return;
      sectionSharp.disabled = !["roundsquare", "fancy1"].includes(sectionType.value);
      pivotName.disabled = !pivotEnabled.checked;
      const puttyDisabled = puttyMode.value === "none";
      puttyTarget.disabled = puttyDisabled;
      puttyLow.disabled = puttyDisabled;
      puttyHigh.disabled = puttyDisabled;
    };
    for (const control of [sectionType, pivotEnabled, puttyMode, showTube].filter(Boolean)) {
      control.addEventListener("change", syncTubeState);
    }
    syncTubeState();
    syncNuclState();
  }

  let dirty = false;
  let apply;
  const markDirty = () => {
    dirty = true;
    apply.disabled = false;
  };
  for (const control of [
    name,
    selection,
    visible,
    locked,
    style,
    stylePreset,
    material,
    alpha,
    edgeType,
    edgeWidth,
    edgeColor,
    edgeColorPicker,
    ...specificControls,
  ]) {
    control.addEventListener("input", markDirty);
    control.addEventListener("change", markDirty);
  }
  const reset = button("Reset all to default", "dialog-reset", () => {
    selection.value = densityMap || rendererGroup ? "(not available)" : "*";
    visible.checked = true;
    locked.checked = false;
    style.value = "";
    stylePreset.value = "";
    material.value = "";
    alpha.value = "1";
    edgeType.value = "none";
    edgeWidth.value = "0.01";
    edgeColor.value = "#000000";
    edgeColorPicker.value = "#000000";
    if (lineWidth) lineWidth.value = "1.2";
    if (detail) detail.value = "3";
    if (bondWidth) bondWidth.value = "0.2";
    if (sphereRadius) sphereRadius.value = "0.3";
    if (showRing) showRing.checked = false;
    if (ringThickness) ringThickness.value = "0.2";
    if (ringColor) ringColor.value = "#ffffbf";
    if (ringColorPicker) ringColorPicker.value = "#ffffbf";
    for (const [key, control] of specificInputs) {
      const fallback = specificDefaults.get(key);
      if (control.type === "checkbox") control.checked = fallback === "true";
      else control.value = fallback;
    }
    for (const resetSpecific of specificResetters) resetSpecific();
    if (disorderColorPicker) disorderColorPicker.value = "#ffffff";
    syncEdgeState();
    if (showRing) showRing.dispatchEvent(new Event("change"));
    syncDensityMapState();
    syncSurfaceState();
    syncAtomIntrState();
    syncAnisouState();
    syncRibbonState();
    syncTubeState();
    syncNuclState();
    markDirty();
  });
  const submit = async (closeAfter) => {
    if (!dirty) {
      if (closeAfter) referenceDialog.close();
      return;
    }
    const changed = await mutate(
      densityMap
        ? "set_density_renderer_properties"
        : surfaceObject
          ? "set_surface_renderer_properties"
          : "set_renderer_properties",
      {
        properties: {
          objectUid: object.uid,
          rendererUid: renderer.uid,
          name: name.value,
          selection:
            densityMap || rendererGroup || selectionLike || nameLabelLike
              ? renderer.selection
              : selection.value,
          visible: visible.checked,
          locked: locked.checked,
          material: material.value,
          alpha: Number(alpha.value),
          edgeType: edgeType.value,
          edgeWidth: Number(edgeWidth.value),
          edgeColor: edgeColor.value,
          lineWidth: lineWidth ? Number(lineWidth.value) : renderer.lineWidth,
          detail: detail ? Number(detail.value) : renderer.detail,
          bondWidth: bondWidth ? Number(bondWidth.value) : renderer.bondWidth,
          sphereRadius: sphereRadius ? Number(sphereRadius.value) : renderer.sphereRadius,
          showRing: showRing ? showRing.checked : renderer.showRing,
          ringThickness: ringThickness ? Number(ringThickness.value) : renderer.ringThickness,
          ringColor: ringColor ? ringColor.value : renderer.ringColor,
          specific: {
            ...Object.fromEntries(
              [...specificInputs].map(([key, control]) => [
                key,
                specificReaders.has(key)
                  ? specificReaders.get(key)()
                  : control.type === "checkbox"
                    ? String(control.checked)
                    : control.value,
              ]),
            ),
            style: style.value,
          },
        },
      },
    );
    if (!changed) return;
    dirty = false;
    apply.disabled = true;
    if (closeAfter) referenceDialog.close();
  };
  const cancel = button("Cancel", "", () => referenceDialog.close());
  apply = button("Apply", "", () => submit(false));
  apply.disabled = true;
  const ok = button("OK", "dialog-primary", () => submit(true));
  dialogContent.append(editor);
  dialogActions(reset, cancel, apply, ok);
}

async function chooseOutputPath(defaultPath, name, extensions) {
  if (!dialog?.save) {
    showError("The Tauri dialog plugin is unavailable.");
    return null;
  }
  return dialog.save({
    defaultPath,
    filters: [{ name, extensions }],
  });
}

async function rendererStyleCreateEditor(object, renderer) {
  const styleSets = (snapshot?.styles || []).filter(
    (style) =>
      style.ownerSceneUid === object.sceneUid &&
      style.localIndex !== null &&
      !style.readOnly,
  );
  dialogTitle.textContent = "Create Renderer Style";
  dialogContent.replaceChildren();
  dialogFooter.hidden = false;
  referenceDialog.className = "reference-dialog has-actions";
  dialogContent.append(dialogParagraph(`Original rend: ${renderer.name} (${renderer.rendererType})`));
  const styleSet = dialogSelect(
    "Target Style set:",
    styleSets.map((style) => [style.localIndex, style.name]),
    styleSets.at(-1)?.localIndex ?? "",
  );
  const nameRow = document.createElement("label");
  nameRow.className = "dialog-field";
  const nameLabel = document.createElement("span");
  nameLabel.textContent = "Style name:";
  const nameEditor = document.createElement("span");
  nameEditor.className = "renderer-style-name";
  const namePrefix = document.createElement("input");
  const suffix = document.createElement("span");
  suffix.textContent = renderer.rendererType;
  nameEditor.append(namePrefix, suffix);
  nameRow.append(nameLabel, nameEditor);
  dialogContent.append(nameRow);
  const overwriteLabel = document.createElement("label");
  overwriteLabel.className = "dialog-checkbox";
  const overwrite = document.createElement("input");
  overwrite.type = "checkbox";
  const overwriteText = document.createElement("span");
  overwriteLabel.append(overwrite, overwriteText);
  overwriteLabel.hidden = true;
  dialogContent.append(overwriteLabel);
  const note = dialogParagraph("");
  note.className = "dialog-note";
  if (!styleSets.length) note.textContent = "Create a writable scene Style set first.";
  dialogContent.append(note);

  let existingNames = new Set();
  let requestSequence = 0;
  let create;
  const updateState = () => {
    const styleName = `${namePrefix.value.trim()}${renderer.rendererType}`;
    const exists = existingNames.has(styleName.toLowerCase());
    overwriteLabel.hidden = !exists;
    overwriteText.textContent = `Overwrite existing style ${styleName}`;
    if (!exists) overwrite.checked = false;
    create.disabled =
      !styleSets.length || !namePrefix.value.trim() || (exists && !overwrite.checked);
  };
  const loadStyleNames = async () => {
    const sequence = ++requestSequence;
    existingNames = new Set();
    if (!styleSets.length || styleSet.value === "") {
      updateState();
      return;
    }
    const localIndex = Number(styleSet.value);
    if (!Number.isInteger(localIndex)) {
      updateState();
      return;
    }
    try {
      const properties = await invoke("get_style_properties", {
        sceneUid: object.sceneUid,
        localIndex,
      });
      if (sequence !== requestSequence) return;
      const documentNode = new DOMParser().parseFromString(properties.xml, "application/xml");
      existingNames = new Set(
        [...documentNode.documentElement.children]
          .filter((node) => node.tagName === "style")
          .map((node) => (node.getAttribute("id") || "").toLowerCase()),
      );
    } catch (error) {
      if (sequence === requestSequence) showError(error);
    }
    updateState();
  };
  namePrefix.addEventListener("input", updateState);
  overwrite.addEventListener("change", updateState);
  styleSet.addEventListener("change", () => {
    overwrite.checked = false;
    loadStyleNames();
  });
  const cancel = button("Cancel", "", () => referenceDialog.close());
  create = button("Create", "dialog-primary", async () => {
    const localIndex = Number(styleSet.value);
    if (!Number.isInteger(localIndex)) return;
    const changed = await mutateWithoutViewport("create_renderer_style", {
      input: {
        sceneUid: object.sceneUid,
        localIndex,
        objectUid: object.uid,
        rendererUid: renderer.uid,
        namePrefix: namePrefix.value,
        overwrite: overwrite.checked,
      },
    });
    if (changed) referenceDialog.close();
  });
  create.disabled = true;
  dialogActions(cancel, create);
  await loadStyleNames();
  if (referenceDialog.showModal && !referenceDialog.open) referenceDialog.showModal();
  namePrefix.focus();
}

async function showStyleProperties(style) {
  if (!style || style.localIndex === null) {
    showError("Global styles are read-only.");
    return;
  }
  let properties;
  try {
    properties = await invoke("get_style_properties", {
      sceneUid: style.sceneUid,
      localIndex: style.localIndex,
    });
  } catch (error) {
    showError(error);
    return;
  }
  dialogTitle.textContent = "Style editor";
  dialogContent.replaceChildren();
  dialogFooter.hidden = false;
  referenceDialog.className = "reference-dialog wide has-actions";
  const name = dialogField("Name", properties.name);
  const readOnlyLabel = document.createElement("label");
  readOnlyLabel.className = "dialog-checkbox";
  const readOnly = document.createElement("input");
  readOnly.type = "checkbox";
  readOnly.checked = properties.readOnly;
  readOnlyLabel.append(readOnly, document.createTextNode("Read only"));
  const xmlLabel = document.createElement("label");
  xmlLabel.className = "dialog-field";
  const xmlTitle = document.createElement("span");
  xmlTitle.textContent = "Style XML";
  const xml = document.createElement("textarea");
  xml.rows = 18;
  xml.spellcheck = false;
  xml.value = properties.xml;
  xmlLabel.append(xmlTitle, xml);
  dialogContent.append(name.closest("label") || name, readOnlyLabel, xmlLabel);
  const submit = async (closeAfter) => {
    setBusy(true);
    try {
      snapshot = await invoke("set_style_properties", {
        properties: {
          sceneUid: properties.sceneUid,
          localIndex: properties.localIndex,
          name: name.value,
          readOnly: readOnly.checked,
          xml: xml.value,
        },
      });
      const styles = activeStyles();
      selectedStyleIndex = styles.findIndex(
        (candidate) =>
          candidate.sceneUid === properties.sceneUid &&
          candidate.localIndex === properties.localIndex,
      );
      workspaceSelectionKind = "style";
      renderShell();
      if (closeAfter) referenceDialog.close();
    } catch (error) {
      showError(error);
    } finally {
      setBusy(false);
    }
  };
  const cancel = button("Cancel", "", () => referenceDialog.close());
  const apply = button("Apply", "", () => submit(false));
  const ok = button("OK", "dialog-primary", () => submit(true));
  dialogActions(cancel, apply, ok);
  if (referenceDialog.showModal && !referenceDialog.open) referenceDialog.showModal();
}

async function showReferenceDialog(title) {
  if (referenceDialog.open) referenceDialog.close();
  atomPickerTarget = null;
  viewport.classList.remove("atom-picking");
  dialogTitle.textContent = title;
  dialogContent.replaceChildren();
  dialogFooter.hidden = false;
  referenceDialog.classList.remove("has-actions");
  const usesApplicationInfo = [
    "About plugins",
    "About config",
    "Addon manager",
    "About CueMol2",
  ].includes(title);
  const usesViewProperties = title === "View Property";
  const usesCameraProperties = title === "Camera Properties";
  const usesGenericProperties =
    usesViewProperties || usesCameraProperties || title === "Scene Properties";
  const usesPreferences = title === "Options";
  const usesWorkspaceProperties = ["Object Properties", "Renderer Properties"].includes(title);
  const usesBondPicker = title === "Mol bond editor";
  referenceDialog.classList.toggle("property-editor", usesGenericProperties);
  referenceDialog.classList.toggle("options-dialog", usesPreferences);
  referenceDialog.classList.toggle("workspace-property-dialog", usesWorkspaceProperties);
  referenceDialog.classList.toggle("bond-picker-dialog", usesBondPicker);
  referenceDialog.classList.toggle(
    "wide",
    usesGenericProperties ||
      usesPreferences ||
      (usesApplicationInfo && title !== "About CueMol2"),
  );
  let applicationInfo = null;
  let viewProperties = null;
  let optionPreferences = null;
  if (usesApplicationInfo) {
    try {
      applicationInfo = await getApplicationInfo();
    } catch (error) {
      showError(error);
    }
  }
  if (usesViewProperties) {
    try {
      viewProperties = await invoke("get_view_properties");
    } catch (error) {
      showError(error);
    }
  }
  if (usesCameraProperties) {
    try {
      viewProperties = await invoke("get_camera_properties", { name: selectedCameraName });
    } catch (error) {
      showError(error);
    }
  }
  if (usesPreferences) {
    try {
      optionPreferences = await invoke("get_preferences");
      preferences = optionPreferences;
    } catch (error) {
      showError(error);
    }
  }
  if (title === "About plugins") {
    const heading = document.createElement("h2");
    heading.textContent = "Registered I/O plugins";
    if (!applicationInfo) {
      dialogContent.append(heading, dialogParagraph("Plugin information is unavailable."));
    } else {
      const catalog = pluginCatalog(applicationInfo);
      dialogContent.append(
        heading,
        dialogParagraph(
          `${applicationInfo.plugins.length} readers, writers, and exporters are registered.`,
        ),
        ...catalog,
      );
    }
  } else if (title === "About config") {
    const heading = document.createElement("h2");
    heading.textContent = "Application preferences";
    if (!applicationInfo) {
      dialogContent.append(heading, dialogParagraph("Configuration is unavailable."));
    } else {
      const browser = configBrowser(applicationInfo);
      dialogContent.append(
        heading,
        dialogParagraph("Edit the active SysConfig values. Changes apply immediately."),
        ...browser,
      );
    }
  } else if (title === "Addon manager") {
    const heading = document.createElement("h2");
    heading.textContent = "Built-in components";
    dialogContent.append(heading);
    if (!applicationInfo) {
      dialogContent.append(dialogParagraph("Component information is unavailable."));
    } else {
      dialogContent.append(
        applicationDefinitionList([
          ["Runtime", applicationInfo.target],
          ["Components", applicationInfo.features.length],
          ["I/O plugins", applicationInfo.plugins.length],
          ["Installation", "Built-in; no external addons installed"],
        ]),
      );
      const features = document.createElement("div");
      features.className = "addon-components";
      for (const feature of applicationInfo.features) {
        const row = document.createElement("article");
        const state = document.createElement("span");
        state.className = "addon-state";
        state.textContent = "Enabled";
        const name = document.createElement("strong");
        name.textContent = feature;
        const source = document.createElement("small");
        source.textContent = "Bundled with CueMol2";
        row.append(name, source, state);
        features.append(row);
      }
      dialogContent.append(features);
    }
  } else if (title === "About CueMol2") {
    const heading = document.createElement("h2");
    heading.textContent = "CueMol2";
    dialogContent.append(
      heading,
      dialogParagraph("Molecular visualization system — Rust desktop implementation."),
    );
    if (applicationInfo) {
      dialogContent.append(
        applicationDefinitionList([
          ["Version", applicationInfo.version],
          ["Target", applicationInfo.target],
          ["Frontend", "Tauri"],
          ["Renderer", "wgpu"],
        ]),
      );
    }
  } else if (title === "Options") {
    if (optionPreferences) {
      optionsEditor(optionPreferences);
    } else {
      dialogContent.append(dialogParagraph("Options are unavailable."));
    }
  } else if (title === "Object Properties") {
    dialogTitle.textContent = "Object properties";
    const object =
      workspaceSelectionKind === "densityMap"
        ? selectedDensityMap()
        : workspaceSelectionKind === "surface"
          ? selectedSurface()
          : selectedObject();
    if (object) {
      objectPropertiesEditor(object);
    } else {
      dialogContent.append(dialogParagraph("Select an object first."));
    }
  } else if (title === "Renderer Properties") {
    dialogTitle.textContent = "Renderer properties";
    const densityRenderer = workspaceSelectionKind === "densityRenderer";
    const surfaceRenderer = workspaceSelectionKind === "surfaceRenderer";
    const object = densityRenderer
      ? selectedDensityMap()
      : surfaceRenderer
        ? selectedSurface()
        : selectedObject();
    const renderer = densityRenderer
      ? selectedDensityRenderer()
      : surfaceRenderer
        ? selectedSurfaceRenderer()
        : selectedRenderer();
    if (object && renderer) {
      let styleOptions = [];
      try {
        styleOptions = await invoke("get_renderer_style_options", {
          sceneUid: object.sceneUid,
          rendererType: renderer.rendererType,
        });
      } catch (error) {
        showError(error);
      }
      rendererPropertiesEditor(object, renderer, styleOptions);
    } else {
      dialogContent.append(dialogParagraph("Select a renderer first."));
    }
  } else if (title === "Scene Properties") {
    const scene = snapshot?.scenes.find((item) => item.active);
    dialogTitle.textContent = "Object properties";
    if (scene) {
      scenePropertyEditor(scene);
    } else {
      dialogContent.append(dialogParagraph("No active scene is available."));
    }
  } else if (title === "View Property") {
    dialogTitle.textContent = "Object properties";
    if (viewProperties) {
      viewPropertyEditor(viewProperties);
    } else {
      dialogContent.append(dialogParagraph("View properties are unavailable."));
    }
  } else if (title === "Camera Properties") {
    dialogTitle.textContent = "Object properties";
    if (viewProperties) {
      cameraPropertyEditor(viewProperties);
    } else {
      dialogContent.append(dialogParagraph("Camera properties are unavailable."));
    }
  } else if (title === "Molecular superposition") {
    const objects = activeObjects();
    if (objects.length < 2) {
      dialogContent.append(dialogParagraph("At least two molecular objects are required."));
    } else {
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const algorithm = dialogSelect(
        "Algorithm",
        [
          ["lsq", "Least Square Fitting"],
          ["ssm", "Secondary Structure Matching"],
        ],
        "lsq",
      );
      const referenceObject = dialogSelect("Reference molecule", objectOptions, objects[0].uid);
      const referenceSelection = dialogField("Reference selection", "name CA");
      const movingObject = dialogSelect("Moving molecule", objectOptions, objects[1].uid);
      const movingSelection = dialogField("Moving selection", "name CA");
      const autoRecenter = dialogCheckbox("Auto recenter", true);
      const useProperty = dialogCheckbox("Use xformMat property", false);
      const apply = button("OK", "dialog-primary", async () => {
        await mutate("superpose_objects", {
          algorithm: algorithm.value,
          referenceObjectUid: Number(referenceObject.value),
          referenceSelection: referenceSelection.value,
          movingObjectUid: Number(movingObject.value),
          movingSelection: movingSelection.value,
          autoRecenter: autoRecenter.checked,
          useProperty: useProperty.checked,
        });
        referenceDialog.close();
      });
      dialogActions(apply);
    }
  } else if (title === "Mol bond editor") {
    dialogTitle.textContent = "Edit non-standard bonds";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const object = dialogSelect(
        "Mol name",
        objectOptions,
        selectedObjectUid ?? objects[0].uid,
      );
      const selectedBonds = new Set();
      let bonds = [];
      const bondList = document.createElement("div");
      bondList.className = "bond-list";
      const remove = button("Delete selected", "", async () => {
        const pairs = bonds
          .filter((bond) => selectedBonds.has(`${bond[0].aid}:${bond[1].aid}`))
          .map((bond) => [bond[0].aid, bond[1].aid]);
        if (pairs.length === 0) return;
        const changed = await mutate("remove_nonstandard_bonds", {
          objectUid: Number(object.value),
          pairs,
        });
        if (changed) await refreshBonds();
      });
      remove.disabled = true;
      const formatAtom = (atom) =>
        `${atom.chain || "_"} ${atom.resn} ${atom.resid} ${atom.aname} (#${atom.aid})`;
      const renderBonds = () => {
        bondList.replaceChildren();
        if (bonds.length === 0) {
          bondList.append(dialogParagraph("No non-standard bonds."));
        }
        for (const bond of bonds) {
          const key = `${bond[0].aid}:${bond[1].aid}`;
          const row = document.createElement("label");
          row.className = "bond-list-row";
          const selected = document.createElement("input");
          selected.type = "checkbox";
          selected.checked = selectedBonds.has(key);
          selected.addEventListener("change", () => {
            if (selected.checked) selectedBonds.add(key);
            else selectedBonds.delete(key);
            remove.disabled = selectedBonds.size === 0;
          });
          const atom1 = document.createElement("span");
          atom1.textContent = formatAtom(bond[0]);
          const atom2 = document.createElement("span");
          atom2.textContent = formatAtom(bond[1]);
          row.append(selected, atom1, atom2);
          bondList.append(row);
        }
      };
      const refreshBonds = async () => {
        try {
          const json = await invoke("get_nonstandard_bonds", {
            objectUid: Number(object.value),
          });
          bonds = JSON.parse(json);
          selectedBonds.clear();
          remove.disabled = true;
          renderBonds();
        } catch (error) {
          showError(error);
        }
      };
      object.addEventListener("change", () => {
        atomPickerTarget = null;
        viewport.classList.remove("atom-picking");
        void refreshBonds();
      });
      dialogContent.append(bondList);
      const aid1 = dialogField("Atom1", "", "number");
      const aid2 = dialogField("Atom2", "", "number");
      const activateAtomPicker = (input) => {
        atomPickerTarget = { input, object };
        viewport.classList.add("atom-picking");
      };
      aid1.addEventListener("focus", () => activateAtomPicker(aid1));
      aid2.addEventListener("focus", () => activateAtomPicker(aid2));
      const make = button("Add", "dialog-primary", async () => {
        const changed = await mutate("edit_bond", {
          objectUid: Number(object.value),
          aid1: Number(aid1.value),
          aid2: Number(aid2.value),
          make: true,
        });
        if (changed) {
          aid1.value = "";
          aid2.value = "";
          atomPickerTarget = null;
          viewport.classList.remove("atom-picking");
          await refreshBonds();
        }
      });
      dialogActions(remove, make);
      void refreshBonds();
    }
  } else if (title === "Interaction") {
    dialogTitle.textContent = "Interaction analysis";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const object = dialogSelect(
        "Molecule",
        objectOptions,
        selectedObjectUid ?? objects[0].uid,
      );
      const selection1 = dialogField("Selection 1", snapshot.selection || "");
      const useObject2 = dialogCheckbox("Mol 2:", false);
      const object2 = dialogSelect(
        "Molecule 2",
        objectOptions,
        objects.find((candidate) => candidate.uid !== Number(object.value))?.uid ?? objects[0].uid,
      );
      object2.disabled = true;
      useObject2.addEventListener("change", () => {
        object2.disabled = !useObject2.checked;
      });
      const useSelection2 = dialogCheckbox("Sel 2:", false);
      const selection2 = dialogField("Selection 2", "");
      selection2.disabled = true;
      useSelection2.addEventListener("change", () => {
        selection2.disabled = !useSelection2.checked;
      });
      const rMin = dialogField("Min distance (Å)", "1.5", "number");
      rMin.step = "0.1";
      rMin.min = "0";
      const rMax = dialogField("Max distance (Å)", "3.5", "number");
      rMax.step = "0.1";
      rMax.min = "0";
      const maxCount = dialogField("Max labels", "100", "number");
      maxCount.min = "1";
      const hbond = dialogCheckbox("Hydrogen bond (N, O) only", true);
      const renderer = dialogField("Renderer", "measure");
      const rendererNames = [
        ...new Set(
          objects.flatMap((candidate) =>
            candidate.renderers
              .filter((item) => item.rendererType === "atomintr")
              .map((item) => item.name),
          ),
        ),
      ];
      if (rendererNames.length) {
        const rendererList = document.createElement("datalist");
        rendererList.id = "interaction-renderer-names";
        for (const name of rendererNames) {
          const option = document.createElement("option");
          option.value = name;
          rendererList.append(option);
        }
        renderer.setAttribute("list", rendererList.id);
        dialogContent.append(rendererList);
      }
      const calculate = button("OK", "dialog-primary", async () => {
        const changed = await mutate("calculate_contacts", {
          objectUid: Number(object.value),
          selection1: selection1.value,
          object2Uid: useObject2.checked ? Number(object2.value) : null,
          selection2: useSelection2.checked ? selection2.value : null,
          rMin: Number(rMin.value),
          rMax: Number(rMax.value),
          hbond: hbond.checked,
          maxCount: Number(maxCount.value),
          rendererName: renderer.value,
        });
        if (changed && snapshot.status !== "No interaction was found") {
          referenceDialog.close();
        }
      });
      dialogActions(calculate);
    }
  } else if (title === "Change symmetry") {
    dialogTitle.textContent = "Change symmetry";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      const object =
        objects.find((candidate) => candidate.uid === selectedObjectUid) || objects[0];
      const crystal = snapshot.crystals.find((item) => item.objectUid === object.uid);
      dialogContent.append(dialogParagraph(`Molecule: ${object.name}`));
      const lattice = dialogSelect(
        "Crystal system",
        [
          ["TRICLINIC", "Triclinic"],
          ["MONOCLINIC", "Monoclinic"],
          ["ORTHORHOMBIC", "Orthorhombic"],
          ["TRIGONAL", "Trigonal"],
          ["TETRAGONAL", "Tetragonal"],
          ["HEXAGONAL", "Hexagonal"],
          ["CUBIC", "Cubic"],
        ],
        crystal?.lattice || "TRICLINIC",
      );
      const spaceGroup = dialogSelect("Space Group", [], "");
      const spaceGroupNumber = dialogField(
        "Space Group Number",
        String(crystal?.spaceGroupNumber || 1),
        "number",
      );
      spaceGroupNumber.disabled = true;
      const restrict = dialogCheckbox("Restrict by symmetry", false);
      const cellA = dialogField("a (Å)", String(crystal?.a || 100), "number");
      const cellB = dialogField("b (Å)", String(crystal?.b || 100), "number");
      const cellC = dialogField("c (Å)", String(crystal?.c || 100), "number");
      const alpha = dialogField("α (°)", String(crystal?.alpha || 90), "number");
      const beta = dialogField("β (°)", String(crystal?.beta || 90), "number");
      const gamma = dialogField("γ (°)", String(crystal?.gamma || 90), "number");
      for (const input of [cellA, cellB, cellC, alpha, beta, gamma]) {
        input.step = "0.1";
      }

      const applyRestrictions = () => {
        for (const input of [cellA, cellB, cellC, alpha, beta, gamma]) {
          input.disabled = false;
        }
        if (!restrict.checked) return;
        const setRightAngles = () => {
          alpha.value = "90";
          beta.value = "90";
          gamma.value = "90";
          alpha.disabled = true;
          beta.disabled = true;
          gamma.disabled = true;
        };
        switch (lattice.value) {
          case "MONOCLINIC":
            alpha.value = "90";
            gamma.value = "90";
            alpha.disabled = true;
            gamma.disabled = true;
            break;
          case "ORTHORHOMBIC":
            setRightAngles();
            break;
          case "TETRAGONAL":
            setRightAngles();
            cellB.value = cellA.value;
            cellB.disabled = true;
            break;
          case "CUBIC":
            setRightAngles();
            cellB.value = cellA.value;
            cellC.value = cellA.value;
            cellB.disabled = true;
            cellC.disabled = true;
            break;
          case "TRIGONAL":
            if (spaceGroup.selectedOptions[0]?.textContent.startsWith("R")) {
              cellB.value = cellA.value;
              cellC.value = cellA.value;
              cellB.disabled = true;
              cellC.disabled = true;
              break;
            }
            setRightAngles();
            gamma.value = "120";
            cellB.value = cellA.value;
            cellB.disabled = true;
            break;
          case "HEXAGONAL":
            setRightAngles();
            gamma.value = "120";
            cellB.value = cellA.value;
            cellB.disabled = true;
            break;
        }
      };
      const loadSpaceGroups = async (preferredId = null) => {
        try {
          let groups = spaceGroupOptionsCache.get(lattice.value);
          if (!groups) {
            groups = JSON.parse(
              await invoke("get_space_groups", { lattice: lattice.value }),
            );
            spaceGroupOptionsCache.set(lattice.value, groups);
          }
          spaceGroup.replaceChildren();
          for (const group of groups) {
            const option = document.createElement("option");
            option.value = String(group.id);
            option.textContent = group.cname;
            spaceGroup.append(option);
          }
          const preferred = String(preferredId ?? groups[0]?.id ?? 1);
          spaceGroup.value = groups.some((group) => String(group.id) === preferred)
            ? preferred
            : String(groups[0]?.id ?? 1);
          spaceGroupNumber.value = spaceGroup.value;
          applyRestrictions();
        } catch (error) {
          showError(error);
        }
      };
      lattice.addEventListener("change", () => void loadSpaceGroups());
      spaceGroup.addEventListener("change", () => {
        spaceGroupNumber.value = spaceGroup.value;
        applyRestrictions();
      });
      restrict.addEventListener("change", applyRestrictions);
      cellA.addEventListener("input", applyRestrictions);
      await loadSpaceGroups(crystal?.spaceGroupNumber || 1);

      const apply = button("OK", "dialog-primary", async () => {
        applyRestrictions();
        const changed = await mutate("change_crystal_info", {
          objectUid: object.uid,
          a: Number(cellA.value),
          b: Number(cellB.value),
          c: Number(cellC.value),
          alpha: Number(alpha.value),
          beta: Number(beta.value),
          gamma: Number(gamma.value),
          spaceGroupNumber: Number(spaceGroup.value),
        });
        if (changed) referenceDialog.close();
      });
      dialogActions(apply);
    }
  } else if (title === "Reassign secondary str") {
    dialogTitle.textContent = "Protein secondary str tool";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const object = dialogSelect(
        "Molecule",
        objectOptions,
        selectedObjectUid ?? objects[0].uid,
      );
      const ignoreBulge = dialogCheckbox("Ignore β bulge", false);
      const gapFill = dialogCheckbox("Helix gap-fill angle (°)", false);
      const angle = dialogField("Gap-fill angle", "120", "number");
      angle.min = "0";
      angle.max = "180";
      angle.step = "5";
      angle.disabled = true;
      gapFill.addEventListener("change", () => {
        angle.disabled = !gapFill.checked;
      });
      const assign = button("OK", "dialog-primary", async () => {
        await mutate("assign_secondary_structure", {
          objectUid: Number(object.value),
          ignoreBulge: ignoreBulge.checked,
          angle: gapFill.checked ? Number(angle.value) : 0,
        });
        referenceDialog.close();
      });
      dialogActions(assign);
    }
  } else if (title === "Mol surface generation") {
    dialogTitle.textContent = "Mol surface tool";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      dialogContent.append(dialogParagraph("Create solvent-excluded surface:"));
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const object = dialogSelect(
        "Target molecule",
        objectOptions,
        selectedObjectUid ?? objects[0].uid,
      );
      const useSelection = dialogCheckbox("Selection", Boolean(snapshot.selection));
      const selection = dialogField("Selection code", snapshot.selection);
      selection.disabled = !useSelection.checked;
      useSelection.addEventListener("change", () => {
        selection.disabled = !useSelection.checked;
      });
      const outputName = dialogField("Surface name", "surface");
      const density = dialogField("Density (/Å)", "1.0", "number");
      density.min = "0.1";
      density.step = "0.1";
      const probeRadius = dialogField("Probe radius (Å)", "1.4", "number");
      probeRadius.min = "0.1";
      probeRadius.step = "0.1";
      const generate = button("OK", "dialog-primary", async () => {
        const name = outputName.value.trim() || "surface";
        const path = await chooseOutputPath(`${name}.obj`, "Wavefront OBJ", ["obj"]);
        if (!path) return;
        await mutate("write_surface", {
          objectUid: Number(object.value),
          selection: useSelection.checked ? selection.value : "",
          density: Number(density.value),
          probeRadius: Number(probeRadius.value),
          path,
        });
        referenceDialog.close();
      });
      dialogActions(generate);
    }
  } else if (title === "Mol surface cutter") {
    dialogTitle.textContent = "MolSurf cutting tool";
    const surfaces = snapshot.surfaces;
    if (surfaces.length === 0) {
      dialogContent.append(dialogParagraph("No molecular surface is loaded."));
    } else {
      dialogContent.append(
        dialogParagraph("Cut molecular surface by the current clipping plane:"),
      );
      const surface = dialogSelect(
        "Target surface",
        surfaces.map((item) => [item.uid, item.name]),
        selectedSurfaceUid ?? surfaces[0].uid,
      );
      const mode = dialogSelect(
        "Cross section type",
        [
          ["full", "Complete"],
          ["separate", "Separately"],
          ["sect", "Section only"],
          ["body", "No section"],
        ],
        "full",
      );
      const density = dialogField("Section mesh density (/Å)", "1.0", "number");
      density.min = "0.1";
      density.step = "0.1";
      const cut = button("OK", "dialog-primary", async () => {
        const changed = await mutate("cut_surface", {
          surfaceUid: Number(surface.value),
          density: Number(density.value),
          mode: mode.value,
        });
        if (changed) referenceDialog.close();
      });
      dialogActions(cut);
    }
  } else if (title === "APBS elepot calculation") {
    dialogTitle.textContent = "APBS tool";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      dialogContent.append(
        dialogParagraph("Calculate electrostatic potential by APBS:"),
      );
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const object = dialogSelect(
        "Target molecule",
        objectOptions,
        selectedObjectUid ?? objects[0].uid,
      );
      const useSelection = dialogCheckbox("Selection", Boolean(snapshot.selection));
      const selection = dialogField("Selection code", snapshot.selection);
      selection.disabled = !useSelection.checked;
      useSelection.addEventListener("change", () => {
        selection.disabled = !useSelection.checked;
      });
      const name = dialogField("Elepot name", "apbs");
      dialogSelect(
        "Charge calculation",
        [
          ["pdb2pqr", "Use PDB2PQR"],
          ["internal", "Use internal method"],
        ],
        "internal",
      );
      dialogCheckbox("Use hydrogen atoms", true);
      const write = button("Write input files", "dialog-primary", async () => {
        const baseName = name.value.trim() || "apbs";
        const inputPath = await chooseOutputPath(`${baseName}.in`, "APBS input", ["in"]);
        if (!inputPath) return;
        await mutate("write_apbs_files", {
          objectUid: Number(object.value),
          selection: useSelection.checked ? selection.value : "",
          pqrPath: replaceFileExtension(inputPath, ".pqr"),
          inputPath,
          dxPath: replaceFileExtension(inputPath, ".dx"),
        });
        referenceDialog.close();
      });
      dialogActions(write);
    }
  } else if (title === "Mol morphing animation") {
    dialogTitle.textContent = "Morph animation tool";
    const objects = activeObjects();
    if (objects.length === 0) {
      dialogContent.append(dialogParagraph("No molecular object is loaded."));
    } else {
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const target = dialogSelect(
        "Target",
        objectOptions,
        selectedObjectUid ?? objects[0].uid,
      );
      const source = dialogSelect("Source frame", objectOptions, objects[0].uid);
      const frameName = dialogField("Frame name", "frame");
      const morph = snapshot.morphs?.find(
        (item) => item.objectUid === Number(target.value),
      );
      const frameList = document.createElement("div");
      frameList.className = "morph-frame-list";
      for (const frame of morph?.frames || []) {
        const row = button(
          `${frame.name}  ${frame.source || frame.sourceType}`,
          frame.index === selectedMorphFrameIndex ? "active" : "",
          () => {
            selectedMorphFrameIndex = frame.index;
            frameList.querySelectorAll("button").forEach((item, index) => {
              item.classList.toggle("active", index === frame.index);
            });
          },
        );
        frameList.append(row);
      }
      if (!morph?.frames?.length) {
        frameList.append(dialogParagraph("No morph frames."));
      }
      dialogContent.append(frameList);
      const frame = dialogField("Frame", String(morph?.frame ?? 0), "number");
      frame.min = "0";
      frame.max = String(Math.max(0, (morph?.frames?.length || 1) - 1));
      frame.step = "0.05";
      const add = button("Add", "dialog-primary", async () => {
        await mutate("append_morph_frame", {
          targetObjectUid: Number(target.value),
          sourceObjectUid: Number(source.value),
          frameName: frameName.value,
        });
        referenceDialog.close();
      });
      const remove = button("Delete", "", async () => {
        if (selectedMorphFrameIndex === null) {
          showError("Select a morph frame first.");
          return;
        }
        await mutate("remove_morph_frame", {
          objectUid: Number(target.value),
          index: selectedMorphFrameIndex,
        });
        selectedMorphFrameIndex = null;
        referenceDialog.close();
      });
      const apply = button("Apply frame", "", async () => {
        await mutate("apply_morph_frame", {
          objectUid: Number(target.value),
          frame: Number(frame.value),
        });
      });
      dialogActions(remove, apply, add);
    }
  } else if (title === "POV-Ray rendering") {
    const scene = snapshot?.scenes.find((item) => item.active);
    dialogSelect(
      "Target",
      scene ? [[scene.uid, scene.name]] : [],
      scene?.uid ?? "",
    );
    const width = dialogField("Image width (px)", "1000", "number");
    width.min = "1";
    width.max = "10000";
    const height = dialogField("Image height (px)", "1000", "number");
    height.min = "1";
    height.max = "10000";
    dialogSelect(
      "Projection",
      [
        ["perspective", "Perspective"],
        ["orthographic", "Orthographic"],
      ],
      snapshot.perspective ? "perspective" : "orthographic",
    );
    dialogCheckbox("Clipping plane", true);
    dialogCheckbox("Post-render alpha blending", true);
    dialogCheckbox("Edge lines", true);
    dialogCheckbox("Pix labels", false);
    const savePov = button("Save POV scene…", "", async () => {
      const path = await chooseOutputPath("cuemol.pov", "POV-Ray scene", ["pov"]);
      if (!path) return;
      await mutateWithoutViewport("export_pov", { path }, renderStatusAndLog);
      referenceDialog.close();
    });
    const saveImage = button("Save image…", "dialog-primary", async () => {
      const path = await chooseOutputPath("cuemol.png", "PNG image", ["png"]);
      if (!path) return;
      await mutateWithoutViewport(
        "export_png",
        {
          path,
          width: Number(width.value),
          height: Number(height.value),
        },
        renderStatusAndLog,
      );
      referenceDialog.close();
    });
    dialogActions(savePov, saveImage);
  } else if (title === "Animation rendering") {
    const scene = snapshot?.scenes.find((item) => item.active);
    dialogSelect(
      "Target scene",
      scene ? [[scene.uid, scene.name]] : [],
      scene?.uid ?? "",
    );
    const outputDir = dialogField("Output dir", "");
    const baseName = dialogField("Output base name", "frame");
    const width = dialogField("Width (px)", "640", "number");
    width.min = "100";
    const height = dialogField("Height (px)", "480", "number");
    height.min = "100";
    const frameRate = dialogSelect(
      "Frame rate (fps)",
      [
        [24, "24"],
        [30, "30"],
        [60, "60"],
      ],
      30,
    );
    dialogCheckbox("Loop", false);
    const chooseDir = button("Change output dir…", "", async () => {
      if (!dialog?.open) {
        showError("The Tauri dialog plugin is unavailable.");
        return;
      }
      const path = await dialog.open({ directory: true, multiple: false });
      if (path) outputDir.value = path;
    });
    const start = button("Start", "dialog-primary", async () => {
      await mutateWithoutViewport(
        "render_animation",
        {
          outputDir: outputDir.value,
          baseName: baseName.value,
          width: Number(width.value),
          height: Number(height.value),
          frameRate: Number(frameRate.value),
        },
        renderStatusAndLog,
      );
      showBottomPanel("output");
      referenceDialog.close();
    });
    dialogActions(chooseDir, start);
  } else if (title === "Execute script") {
    const script = dialogTextArea(
      "JavaScript",
      'print("CueMol2");\n// cuemol.select("name CA");',
    );
    const run = button("Execute", "dialog-primary", async () => {
      await mutate("execute_script", { script: script.value });
      showBottomPanel("output");
      referenceDialog.close();
    });
    dialogActions(run);
  } else if (title === "Save File As") {
    const object = selectedObject();
    if (!object) {
      dialogContent.append(dialogParagraph("Select a molecular object first."));
    } else {
      dialogContent.append(dialogParagraph(`Object: ${object.name}`));
      const save = button("Save…", "dialog-primary", async () => {
        if (!dialog?.save) {
          showError("The Tauri dialog plugin is unavailable.");
          return;
        }
        const path = await dialog.save({
          defaultPath: `${object.name || "molecule"}.pdb`,
          filters: [
            { name: "Molecular structure", extensions: ["pdb", "sdf", "qdf"] },
          ],
        });
        if (!path) return;
        await mutateWithoutViewport(
          "save_object",
          { objectUid: object.uid, path },
          renderStatusAndLog,
        );
        referenceDialog.close();
      });
      dialogActions(save);
    }
  } else if (title === "Save Current View") {
    const save = button("Save…", "dialog-primary", async () => {
      const path = await chooseOutputPath("current.view", "CueMol view", ["view", "cam"]);
      if (!path) return;
      await mutateWithoutViewport("save_view", { path }, renderStatusAndLog);
      referenceDialog.close();
    });
    dialogActions(save);
  } else if (title === "Merge molecule") {
    const objects = activeObjects();
    if (objects.length < 2) {
      dialogContent.append(dialogParagraph("At least two molecular objects are required."));
    } else {
      const objectOptions = objects.map((object) => [object.uid, object.name]);
      const source = dialogSelect("From molecule", objectOptions, objects[0].uid);
      const selection = dialogField("Selection", snapshot.selection);
      const target = dialogSelect("To molecule", objectOptions, objects[1].uid);
      const copy = dialogCheckbox("Copy", false);
      const merge = button("OK", "dialog-primary", async () => {
        await mutate("merge_molecules", {
          sourceObjectUid: Number(source.value),
          selection: selection.value,
          targetObjectUid: Number(target.value),
          copy: copy.checked,
        });
        referenceDialog.close();
      });
      dialogActions(merge);
    }
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
      dialogActions(apply);
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
      dialogActions(apply);
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
      dialogActions(apply);
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
      dialogActions(apply);
    }
  } else if (title === "Open MD Trajectory") {
    const topologyPath = dialogField("Topology file (.gro)", "");
    topologyPath.placeholder = "/path/to/system.gro";
    const trajectoryPaths = [];
    const topologyActions = document.createElement("div");
    topologyActions.className = "trajectory-path-actions";
    const browseTopology = button("Browse topology…", "", async () => {
      if (!dialog?.open) {
        showError("The Tauri dialog plugin is unavailable.");
        return;
      }
      const path = await dialog.open({
        multiple: false,
        directory: false,
        filters: [{ name: "GROMACS topology", extensions: ["gro"] }],
      });
      if (path) topologyPath.value = path;
      updateOpenState();
    });
    topologyActions.append(browseTopology);
    dialogContent.append(topologyActions);

    const listWrapper = document.createElement("label");
    listWrapper.className = "dialog-field";
    const listLabel = document.createElement("span");
    listLabel.textContent = "Trajectory files";
    const trajectoryList = document.createElement("select");
    trajectoryList.className = "trajectory-file-list";
    trajectoryList.size = 5;
    listWrapper.append(listLabel, trajectoryList);
    dialogContent.append(listWrapper);

    const pathActions = document.createElement("div");
    pathActions.className = "trajectory-path-actions";
    const renderPaths = () => {
      const selected = trajectoryList.selectedIndex;
      trajectoryList.replaceChildren();
      for (const path of trajectoryPaths) {
        const option = document.createElement("option");
        const name = path.split(/[\\/]/).pop() || path;
        const extension = name.includes(".") ? name.split(".").pop().toUpperCase() : "TRAJ";
        option.value = path;
        option.textContent = `${name}  [${extension}]`;
        option.title = path;
        trajectoryList.append(option);
      }
      trajectoryList.selectedIndex = Math.min(selected, trajectoryPaths.length - 1);
      updateOpenState();
    };
    const addPaths = button("Add…", "", async () => {
      if (!dialog?.open) {
        showError("The Tauri dialog plugin is unavailable.");
        return;
      }
      const paths = await dialog.open({
        multiple: true,
        directory: false,
        filters: [
          { name: "MD trajectory", extensions: ["dcd", "xtc", "trr", "nc"] },
        ],
      });
      if (paths) trajectoryPaths.push(...(Array.isArray(paths) ? paths : [paths]));
      renderPaths();
    });
    const removePath = button("Remove", "", () => {
      if (trajectoryList.selectedIndex >= 0) {
        trajectoryPaths.splice(trajectoryList.selectedIndex, 1);
        renderPaths();
      }
    });
    const movePath = (direction) => {
      const index = trajectoryList.selectedIndex;
      const target = index + direction;
      if (index < 0 || target < 0 || target >= trajectoryPaths.length) return;
      [trajectoryPaths[index], trajectoryPaths[target]] = [
        trajectoryPaths[target],
        trajectoryPaths[index],
      ];
      renderPaths();
      trajectoryList.selectedIndex = target;
    };
    pathActions.append(
      addPaths,
      removePath,
      button("Up", "", () => movePath(-1)),
      button("Down", "", () => movePath(1)),
    );
    dialogContent.append(pathActions);

    const nevery = dialogField("Load every N-th frame", "1", "number");
    nevery.min = "1";
    nevery.max = "9999";
    nevery.step = "1";
    const rendererType = dialogSelect(
      "Initial renderer",
      [
        ["simple", "Simple"],
        ["ballstick", "Ball & stick"],
        ["cpk", "CPK"],
        ["ribbon", "Ribbon"],
        ["cartoon", "Cartoon"],
        ["trace", "Trace"],
      ],
      "simple",
    );
    const open = button("Open", "dialog-primary", async () => {
      await mutate("load_trajectory", {
        topologyPath: topologyPath.value.trim(),
        trajectoryPaths,
        nevery: Math.max(1, Number(nevery.value) || 1),
        rendererType: rendererType.value,
      });
      const loaded = snapshot?.trajectories?.at(-1);
      if (loaded) {
        selectedTrajectoryUid = loaded.objectUid;
        trajectoryFrames.set(loaded.objectUid, 0);
      }
      showBottomPanel("trajectory");
      referenceDialog.close();
    });
    function updateOpenState() {
      open.disabled = topologyPath.value.trim().length === 0 || trajectoryPaths.length === 0;
    }
    topologyPath.addEventListener("input", updateOpenState);
    updateOpenState();
    dialogActions(open);
  } else if (title === "Open PDB") {
    dialogTitle.textContent = "Download PDB";
    dialogContent.append(dialogParagraph("Enter PDB ID:"));
    let pdbHistory = [];
    if (invoke) {
      try {
        const storedHistory = await invoke("get_pdb_history");
        if (Array.isArray(storedHistory)) {
          pdbHistory = storedHistory.filter(
            (entry) => typeof entry === "string" && /^[0-9][0-9A-Za-z]{3}$/.test(entry),
          );
        }
      } catch {
        pdbHistory = [];
      }
    }
    const pdbId = dialogField("PDB ID", pdbHistory[0] || "");
    pdbId.maxLength = 4;
    pdbId.pattern = "[0-9][0-9A-Za-z]{3}";
    const pdbHistoryList = document.createElement("datalist");
    pdbHistoryList.id = "open-pdb-history";
    for (const entry of pdbHistory) {
      const option = document.createElement("option");
      option.value = entry;
      pdbHistoryList.append(option);
    }
    pdbId.setAttribute("list", pdbHistoryList.id);
    dialogContent.append(pdbHistoryList);
    const coordinatesGroup = dialogGroup("Coordinates");
    const fetchCoordinates = dialogCheckbox("Fetch pdb file", true, coordinatesGroup);
    const coordinateSource = dialogSelect(
      "Coordinate source",
      [
        ["RCSB_CIF", "RCSB (files.rcsb.org)/cif.gz"],
        ["RCSB_PDB", "RCSB (files.rcsb.org)/pdb.gz"],
      ],
      "RCSB_CIF",
      coordinatesGroup,
    );
    const mapsGroup = dialogGroup("Density maps");
    const fetchTwoFofc = dialogCheckbox(
      "Fetch map file (2Fo-Fc)",
      false,
      mapsGroup,
    );
    const fetchFofc = dialogCheckbox("Fetch map file (Fo-Fc)", false, mapsGroup);
    const mapSource = dialogSelect(
      "Map source",
      [
        ["RCSB_CIF", "RCSB (files.rcsb.org)"],
        ["EBI_MTZ", "EBI (www.ebi.ac.uk)"],
      ],
      "RCSB_CIF",
      mapsGroup,
    );
    const open = button("Download and Open", "dialog-primary", async () => {
      const id = pdbId.value.trim().toUpperCase();
      if (!/^[0-9][0-9A-Z]{3}$/.test(id)) return;
      const changed = await mutateWithViewport("open_pdb", {
        pdbId: id,
        fetchCoordinates: fetchCoordinates.checked,
        coordinateSource: coordinateSource.value,
        fetchTwoFofc: fetchTwoFofc.checked,
        fetchFofc: fetchFofc.checked,
        mapSource: mapSource.value,
      });
      if (changed) referenceDialog.close();
    });
    const updateOpenState = () => {
      const mapsSelected = fetchTwoFofc.checked || fetchFofc.checked;
      coordinateSource.disabled = !fetchCoordinates.checked;
      mapSource.disabled = !mapsSelected;
      open.disabled =
        !/^[0-9][0-9A-Z]{3}$/.test(pdbId.value.trim().toUpperCase()) ||
        (!fetchCoordinates.checked && !mapsSelected);
    };
    pdbId.addEventListener("input", updateOpenState);
    fetchCoordinates.addEventListener("change", updateOpenState);
    fetchTwoFofc.addEventListener("change", updateOpenState);
    fetchFofc.addEventListener("change", updateOpenState);
    updateOpenState();
    dialogActions(open);
  } else {
    dialogContent.append(dialogParagraph(`${title} dialog`));
  }
  if (!referenceDialog.open) {
    if (usesBondPicker && referenceDialog.show) referenceDialog.show();
    else if (referenceDialog.showModal) referenceDialog.showModal();
  }
}

function applyCenterMark(value) {
  centerMark = value;
  centerMarkElement.classList.toggle("axis", value === "axis");
  centerMarkElement.classList.toggle("none", value === "none");
  document.querySelectorAll("[data-center-mark]").forEach((item) => {
    item.classList.toggle("checked", item.dataset.centerMark === value);
  });
}

function restoreDefaultPanels() {
  const openPanels = new Set(["Workspace", "MolStruct", "Selection", "Coloring", "Density map"]);
  document.querySelectorAll("[data-side-card]").forEach((card) => {
    card.hidden = false;
    const title = card.querySelector(".side-title span:nth-child(2)")?.textContent;
    const open = openPanels.has(title);
    card.classList.toggle("open", open);
    const button = card.querySelector("[data-panel-toggle]");
    button?.setAttribute("aria-expanded", String(open));
    const disclosure = card.querySelector(".disclosure");
    if (disclosure) disclosure.textContent = open ? "▾" : "▸";
  });
  syncSidePanelVisibility();
  for (const name of openPanels) renderSidePanel(name);
  document.querySelector(".side-panel")?.scrollTo({ top: 0 });
  scheduleRender(140);
}

function sidePanelCard(name) {
  return [...document.querySelectorAll("[data-side-card]")].find(
    (item) => item.querySelector(".side-title span:nth-child(2)")?.textContent === name,
  );
}

function sidePanelIsExpanded(name) {
  const card = sidePanelCard(name);
  return Boolean(card && !card.hidden && card.classList.contains("open"));
}

function renderSidePanel(name) {
  switch (name) {
    case "MolStruct":
      renderMolStruct();
      scheduleMoleculeDetailsRefresh(0);
      break;
    case "Selection":
      renderSelectionPanel();
      scheduleMoleculeDetailsRefresh(0, selectionObjectUid);
      break;
    case "Coloring":
      renderColoring();
      break;
    case "Symmetry":
      renderSymmetry();
      break;
    case "Density map":
      renderDensity();
      break;
    case "Animation":
      renderAnimation();
      break;
  }
}

function syncSidePanelVisibility() {
  const cards = [...document.querySelectorAll("[data-side-card]")];
  sidePanel.hidden = !cards.some((card) => !card.hidden);
  document.querySelectorAll("[data-panel-menu]").forEach((element) => {
    element.classList.toggle("checked", !sidePanelCard(element.dataset.panelMenu)?.hidden);
  });
}

async function renderWindowsMenu() {
  const getAll = tauri?.webviewWindow?.getAllWebviewWindows;
  const getCurrent = tauri?.webviewWindow?.getCurrentWebviewWindow;
  if (!windowsMenu || !getAll || !getCurrent) return;
  try {
    const [windows, current] = await Promise.all([getAll(), getCurrent()]);
    windowsMenu.replaceChildren();
    windows.forEach((appWindow, index) => {
      const suffix = windows.length > 1 ? ` (${index + 1})` : "";
      const item = button(`CueMol2${suffix}`, appWindow.label === current.label ? "checked" : "", async () => {
        closeMenus();
        await appWindow.setFocus();
      });
      windowsMenu.append(item);
    });
  } catch (error) {
    console.warn("Cannot enumerate CueMol2 windows:", error);
  }
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
    case "reload-scene": {
      const scene = snapshot?.scenes.find((item) => item.active);
      if (
        scene?.modified &&
        !window.confirm(`Do you want to reload the scene: ${scene.name || ""}?`)
      ) {
        break;
      }
      await mutate("reload_scene");
      break;
    }
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
      await mutateWithoutViewport("clear_log", {}, renderStatusAndLog);
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

function workspaceStateControls(target, visible, locked, lockEnabled = true) {
  const controls = document.createElement("span");
  controls.className = "workspace-state-controls";

  const visibility = document.createElement("span");
  visibility.className = "workspace-state-toggle";
  visibility.textContent = visible ? "◉" : "○";
  visibility.title = visible ? "Hide" : "Show";
  visibility.setAttribute("role", "checkbox");
  visibility.setAttribute("aria-checked", String(visible));
  visibility.addEventListener("click", async (event) => {
    event.stopPropagation();
    await mutate("set_workspace_visible", { target, visible: !visible });
  });

  const lock = document.createElement("span");
  lock.className = `workspace-state-toggle lock${lockEnabled ? "" : " disabled"}`;
  lock.textContent = locked ? "🔒" : "🔓";
  lock.title = lockEnabled ? (locked ? "Unlock" : "Lock") : "Group locking is unavailable";
  lock.setAttribute("aria-disabled", String(!lockEnabled));
  if (lockEnabled) {
    lock.setAttribute("role", "checkbox");
    lock.setAttribute("aria-checked", String(locked));
    lock.addEventListener("click", async (event) => {
      event.stopPropagation();
      await mutateWithoutViewport("set_workspace_locked", { target, locked: !locked });
    });
  }

  controls.append(visibility, lock);
  return controls;
}

function activeObjects() {
  if (!snapshot) return [];
  return snapshot.objects.filter((object) => object.sceneUid === snapshot.activeSceneUid);
}

function activeSurfaces() {
  if (!snapshot) return [];
  return (snapshot.surfaces || []).filter(
    (surface) => surface.sceneUid === snapshot.activeSceneUid,
  );
}

function activeCameras() {
  if (!snapshot) return [];
  return (snapshot.cameras || []).filter(
    (camera) => camera.sceneUid === snapshot.activeSceneUid,
  );
}

function selectedCamera() {
  return activeCameras().find((camera) => camera.name === selectedCameraName) || null;
}

function activeStyles() {
  if (!snapshot) return [];
  return (snapshot.styles || []).filter(
    (style) => style.ownerSceneUid === snapshot.activeSceneUid,
  );
}

function selectedStyle() {
  return selectedStyleIndex === null ? null : activeStyles()[selectedStyleIndex] || null;
}

function selectedObject() {
  return activeObjects().find((object) => object.uid === selectedObjectUid) || null;
}

function selectedRenderer() {
  return selectedObject()?.renderers.find((renderer) => renderer.uid === selectedRendererUid) || null;
}

function groupedMoleculeRenderers(renderers) {
  const nodes = [];
  for (const renderer of renderers) {
    if (renderer.rendererType === "*group") {
      nodes.push({
        renderer,
        children: renderers.filter(
          (candidate) => candidate.rendererType !== "*group" && candidate.group === renderer.name,
        ),
      });
    } else if (!renderer.group) {
      nodes.push({ renderer, children: [] });
    }
  }
  return nodes;
}

function clearWorkspaceObjectDropMarkers() {
  document
    .querySelectorAll(".tree-object.drop-before, .tree-object.drop-after")
    .forEach((row) => row.classList.remove("drop-before", "drop-after"));
}

function configureWorkspaceObjectDrag(row, sceneUid, kind, uid) {
  row.draggable = true;
  row.addEventListener("dragstart", (event) => {
    draggedWorkspaceObject = { sceneUid, kind, uid };
    row.classList.add("dragging");
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData(
      "application/x-cuemol-object",
      JSON.stringify(draggedWorkspaceObject),
    );
  });
  row.addEventListener("dragover", (event) => {
    const source = draggedWorkspaceObject;
    if (!source || source.sceneUid !== sceneUid || (source.kind === kind && source.uid === uid)) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    clearWorkspaceObjectDropMarkers();
    const bounds = row.getBoundingClientRect();
    const position = event.clientY - bounds.top < bounds.height / 2 ? "before" : "after";
    row.classList.add(`drop-${position}`);
    row.dataset.dropPosition = position;
  });
  row.addEventListener("dragleave", (event) => {
    if (!row.contains(event.relatedTarget)) row.classList.remove("drop-before", "drop-after");
  });
  row.addEventListener("drop", async (event) => {
    const source = draggedWorkspaceObject;
    const position = row.dataset.dropPosition || "before";
    clearWorkspaceObjectDropMarkers();
    if (!source || source.sceneUid !== sceneUid || (source.kind === kind && source.uid === uid)) {
      return;
    }
    event.preventDefault();
    draggedWorkspaceObject = null;
    await mutateWithoutViewport("move_workspace_object", {
      input: {
        sceneUid,
        sourceKind: source.kind,
        sourceUid: source.uid,
        targetKind: kind,
        targetUid: uid,
        position,
      },
    });
  });
  row.addEventListener("dragend", () => {
    draggedWorkspaceObject = null;
    row.classList.remove("dragging");
    clearWorkspaceObjectDropMarkers();
  });
}

function closeWorkspaceContextMenu() {
  document.querySelector(".workspace-context-menu")?.remove();
}

function showWorkspaceContextMenu(event, items) {
  event.preventDefault();
  event.stopPropagation();
  closeWorkspaceContextMenu();
  const menu = document.createElement("div");
  menu.className = "workspace-context-menu";
  menu.setAttribute("role", "menu");
  const appendItems = (container, menuItems) => {
    for (const item of menuItems) {
      if (!item) {
        const separator = document.createElement("span");
        separator.className = "menu-separator";
        container.append(separator);
        continue;
      }
      if (item.children) {
        menu.classList.add("has-submenu");
        const submenu = document.createElement("span");
        submenu.className = "menu-submenu";
        const trigger = button(item.label, "menu-submenu-trigger", () => {});
        trigger.disabled = Boolean(item.disabled);
        trigger.setAttribute("aria-haspopup", "menu");
        const arrow = document.createElement("span");
        arrow.className = "menu-arrow";
        arrow.textContent = "›";
        trigger.append(arrow);
        const nested = document.createElement("div");
        nested.className = "menu-popup nested-menu";
        nested.setAttribute("role", "menu");
        appendItems(nested, item.children);
        submenu.append(trigger, nested);
        container.append(submenu);
        continue;
      }
      const entry = button(item.label, "", async () => {
        closeWorkspaceContextMenu();
        await item.action?.();
      });
      entry.disabled = Boolean(item.disabled);
      if (typeof item.checked === "boolean") {
        entry.classList.toggle("checked", item.checked);
        entry.setAttribute("role", "menuitemcheckbox");
        entry.setAttribute("aria-checked", String(item.checked));
      } else {
        entry.setAttribute("role", "menuitem");
      }
      container.append(entry);
    }
  };
  appendItems(menu, items);
  document.body.append(menu);
  const bounds = menu.getBoundingClientRect();
  menu.style.left = `${Math.max(4, Math.min(event.clientX, innerWidth - bounds.width - 4))}px`;
  menu.style.top = `${Math.max(4, Math.min(event.clientY, innerHeight - bounds.height - 4))}px`;
}

async function loadWorkspaceCameraFile(scene) {
  if (!dialog?.open) {
    showError("The Tauri dialog plugin is unavailable.");
    return;
  }
  const path = await dialog.open({
    multiple: false,
    directory: false,
    filters: [{ name: "Camera setting", extensions: ["cam"] }],
  });
  if (!path) return;
  const previous = new Set(
    (snapshot.cameras || [])
      .filter((camera) => camera.sceneUid === scene.uid)
      .map((camera) => camera.name),
  );
  if (!(await mutate("load_camera_file", { sceneUid: scene.uid, path }))) return;
  const loaded = (snapshot.cameras || []).find(
    (camera) => camera.sceneUid === scene.uid && !previous.has(camera.name),
  );
  selectedCameraName = loaded?.name ?? null;
  workspaceSelectionKind = loaded ? "camera" : "cameraRoot";
  renderShell();
}

async function saveWorkspaceCameraFile(scene, camera, saveAs) {
  let path = !saveAs ? camera.source : "";
  if (!path) {
    path = await chooseOutputPath(`${camera.name}.cam`, "Camera setting", ["cam"]);
  }
  if (!path) return;
  if (
    await mutateWithoutViewport("save_camera_file", {
      sceneUid: scene.uid,
      name: camera.name,
      path,
    })
  ) {
    selectedCameraName = camera.name;
    workspaceSelectionKind = "camera";
    renderShell();
  }
}

function invertWorkspaceSelection(selection) {
  const current = selection.trim();
  if (!current) return "*";
  const inverted = current.match(/^!\((.*)\)$/s);
  return inverted ? inverted[1] : `!(${current})`;
}

function toggleWorkspaceSidechain(selection) {
  const current = selection.trim();
  return current.startsWith("bysidech ") ? current.slice(9).trim() : `bysidech ${current}`;
}

function aroundWorkspaceSelection(selection, radius, byResidue) {
  let base = selection.trim();
  if (base.startsWith("byres ")) base = base.slice(6).trim();
  const previous = base.match(/^\((.*)\)\s+around\s+[-+]?\d+(?:\.\d+)?$/s);
  if (previous) base = previous[1];
  const expression = `(${base}) around ${radius}`;
  return byResidue ? `byres ${expression}` : expression;
}

async function copyWorkspaceRenderer(objectUid, rendererUid) {
  if (!invoke) return false;
  setBusy(true);
  try {
    const status = await invoke("copy_workspace_renderer", {
      input: { objectUid, rendererUid },
    });
    workspaceRendererClipboardAvailable = true;
    snapshot.status = status;
    renderStatusAndLog();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

function pasteWorkspaceRenderers(objectUid, destinationGroupUid = null) {
  return mutate("paste_workspace_renderers", {
    input: { objectUid, destinationGroupUid },
  });
}

async function copyWorkspaceObject(objectUid) {
  if (!invoke) return false;
  setBusy(true);
  try {
    const status = await invoke("copy_workspace_object", {
      input: { objectUid },
    });
    workspaceObjectClipboardAvailable = true;
    snapshot.status = status;
    renderStatusAndLog();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

function pasteWorkspaceObject(scene) {
  return mutate(
    "paste_workspace_object",
    { input: { sceneUid: scene.uid } },
    renderShell,
    Boolean(scene.active),
  );
}

function workspaceSceneContextItems(scene) {
  const currentScene = snapshot.scenes.find((candidate) => candidate.uid === scene.uid) || scene;
  const activate = async () => {
    if (snapshot.activeSceneUid === scene.uid) return true;
    return mutate("activate_scene", { sceneUid: scene.uid });
  };
  const setBackground = (background) => async () => {
    if (!(await activate()) || snapshot.background === background) return;
    await mutate("set_background", { background });
  };
  return [
    {
      label: "Background color · White",
      checked: currentScene.background === "white",
      action: setBackground("white"),
    },
    {
      label: "Background color · Black",
      checked: currentScene.background === "black",
      action: setBackground("black"),
    },
    {
      label: "Use color proofing",
      checked: Boolean(currentScene.useColorProof),
      action: async () => {
        const value = !currentScene.useColorProof;
        if (!(await activate())) return;
        await mutate("set_display_option", { option: "color-proof", value });
      },
    },
    {
      label: "Paste Object",
      disabled: !workspaceObjectClipboardAvailable,
      action: () => pasteWorkspaceObject(scene),
    },
    {
      label: "Properties...",
      action: async () => {
        if (!(await activate())) return;
        showReferenceDialog("Scene Properties");
      },
    },
  ];
}

async function copyWorkspaceCamera(sceneUid, name) {
  if (!invoke) return false;
  setBusy(true);
  try {
    const status = await invoke("copy_workspace_camera", {
      input: { sceneUid, name },
    });
    workspaceCameraClipboardAvailable = true;
    snapshot.status = status;
    renderStatusAndLog();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

async function pasteWorkspaceCamera(scene) {
  const previous = new Set(
    (snapshot.cameras || [])
      .filter((camera) => camera.sceneUid === scene.uid)
      .map((camera) => camera.name),
  );
  const changed = await mutateWithoutViewport(
    "paste_workspace_camera",
    { input: { sceneUid: scene.uid } },
    () => {},
  );
  if (!changed) return;
  const pasted = (snapshot.cameras || []).find(
    (camera) => camera.sceneUid === scene.uid && !previous.has(camera.name),
  );
  if (scene.active && pasted) {
    selectedCameraName = pasted.name;
    workspaceSelectionKind = "camera";
  }
  renderShell();
}

async function showCameraVisibilityEditor(scene, camera) {
  if (!invoke || !scene || !camera) return;
  let entries;
  setBusy(true);
  try {
    entries = await invoke("get_camera_visibility", {
      sceneUid: scene.uid,
      name: camera.name,
    });
  } catch (error) {
    showError(error);
    return;
  } finally {
    setBusy(false);
  }

  const draft = entries.map((entry) => ({ ...entry }));
  dialogTitle.textContent = "Edit visibility flags";
  dialogContent.replaceChildren();
  dialogFooter.hidden = false;
  referenceDialog.className = "reference-dialog wide has-actions visibility-flag-dialog";
  const editor = document.createElement("div");
  editor.className = "visibility-flag-editor";
  const header = document.createElement("div");
  header.className = "visibility-flag-row visibility-flag-header";
  for (const label of ["Inc", "Obj/Rend", "Vis"]) {
    const cell = document.createElement("span");
    cell.textContent = label;
    header.append(cell);
  }
  editor.append(header);
  for (const entry of draft) {
    const row = document.createElement("div");
    row.className = "visibility-flag-row";
    row.classList.toggle("renderer", !entry.isObject);
    const included = document.createElement("input");
    included.type = "checkbox";
    included.checked = entry.included;
    included.setAttribute("aria-label", `Include ${entry.name}`);
    const target = document.createElement("span");
    target.className = "visibility-flag-target";
    const name = document.createElement("span");
    name.textContent = entry.name || entry.typeName;
    const type = document.createElement("small");
    type.textContent = entry.typeName;
    target.append(name, type);
    const visible = document.createElement("input");
    visible.type = "checkbox";
    visible.checked = entry.included && entry.visible;
    visible.disabled = !entry.included;
    visible.setAttribute("aria-label", `Visible ${entry.name}`);
    included.addEventListener("change", () => {
      entry.included = included.checked;
      if (!entry.included) {
        entry.visible = false;
        visible.checked = false;
      }
      visible.disabled = !entry.included;
    });
    visible.addEventListener("change", () => {
      entry.visible = visible.checked;
    });
    row.append(included, target, visible);
    editor.append(row);
  }
  if (draft.length === 0) editor.append(dialogParagraph("No objects or renderers."));
  dialogContent.append(editor);

  const submit = async (closeAfter) => {
    const changed = await mutateWithoutViewport(
      "set_camera_visibility",
      {
        input: {
          sceneUid: scene.uid,
          name: camera.name,
          entries: draft
            .filter((entry) => entry.included)
            .map((entry) => ({
              targetUid: entry.targetUid,
              isObject: entry.isObject,
              visible: entry.visible,
            })),
        },
      },
      () => {},
    );
    if (!changed) return;
    selectedCameraName = camera.name;
    workspaceSelectionKind = "camera";
    renderShell();
    if (closeAfter) referenceDialog.close();
  };
  const cancel = button("Cancel", "", () => referenceDialog.close());
  const apply = button("Apply", "", () => submit(false));
  const ok = button("OK", "dialog-primary", () => submit(true));
  dialogActions(cancel, apply, ok);
  if (referenceDialog.showModal && !referenceDialog.open) referenceDialog.showModal();
}

function workspaceCameraContextItems(scene, camera = null) {
  const activate = async () => {
    if (snapshot.activeSceneUid === scene.uid) return true;
    return mutate("activate_scene", { sceneUid: scene.uid });
  };
  const items = [
    {
      label: "New Camera...",
      action: async () => {
        if (!(await activate())) return;
        const used = new Set(activeCameras().map((candidate) => candidate.name));
        let index = 0;
        while (used.has(`camera_${index}`)) index += 1;
        const name = globalThis.prompt?.("Camera name", `camera_${index}`)?.trim();
        if (name) await mutate("save_camera_view", { name });
      },
    },
    {
      label: "Delete",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        await mutate("destroy_camera", { name: camera.name });
      },
    },
  ];
  if (camera) {
    items.push({
      label: "Copy",
      action: () => copyWorkspaceCamera(scene.uid, camera.name),
    });
  }
  items.push({
    label: "Paste",
    disabled: !workspaceCameraClipboardAvailable,
    action: () => pasteWorkspaceCamera(scene),
  });
  items.push(
    null,
    {
      label: "Camera file",
      children: [
        {
          label: "Load ...",
          action: async () => {
            if (!(await activate())) return;
            await loadWorkspaceCameraFile(scene);
          },
        },
        {
          label: "Reload",
          disabled: !camera?.source,
          action: async () => {
            if (!(await activate())) return;
            await mutateWithoutViewport("reload_camera_file", {
              sceneUid: scene.uid,
              name: camera.name,
            });
          },
        },
        {
          label: "Save",
          disabled: !camera,
          action: async () => {
            if (!(await activate())) return;
            await saveWorkspaceCameraFile(scene, camera, false);
          },
        },
        {
          label: "Save as...",
          disabled: !camera,
          action: async () => {
            if (!(await activate())) return;
            await saveWorkspaceCameraFile(scene, camera, true);
          },
        },
      ],
    },
    null,
    {
      label: "Save from view",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        await mutateWithoutViewport("save_camera_view", { name: camera.name });
      },
    },
    {
      label: "Apply to view",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        await mutate("load_camera_view", { name: camera.name });
      },
    },
    null,
    {
      label: "Save from scene (with vis flags)",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        await mutate("save_camera", { name: camera.name });
      },
    },
    {
      label: "Apply to scene (with vis flags)",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        await mutate("load_camera", { name: camera.name });
      },
    },
    {
      label: "Edit vis flags...",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        await showCameraVisibilityEditor(scene, camera);
      },
    },
    {
      label: "Clear vis flags",
      disabled: !camera || camera.visibilityCount === 0,
      action: async () => {
        if (!(await activate())) return;
        if (!globalThis.confirm?.(`Clear visibility flags in ${camera.name}?`)) return;
        await mutateWithoutViewport("clear_camera_visibility", {
          sceneUid: scene.uid,
          name: camera.name,
        });
      },
    },
    null,
    {
      label: "Rename...",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        selectedCameraName = camera.name;
        await showReferenceDialog("Camera Properties");
      },
    },
    {
      label: "Properties...",
      disabled: !camera,
      action: async () => {
        if (!(await activate())) return;
        selectedCameraName = camera.name;
        await showReferenceDialog("Camera Properties");
      },
    },
  );
  return items;
}

async function copyWorkspaceStyle(sceneUid, style) {
  if (!invoke || style.localIndex === null) return false;
  setBusy(true);
  try {
    const status = await invoke("copy_workspace_style", {
      input: { sceneUid, localIndex: style.localIndex },
    });
    workspaceStyleClipboardName = style.name;
    snapshot.status = status;
    renderStatusAndLog();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

async function pasteWorkspaceStyle(scene) {
  if (!workspaceStyleClipboardName) return;
  const replace = (snapshot.styles || []).some(
    (style) =>
      style.ownerSceneUid === scene.uid &&
      style.localIndex !== null &&
      style.name === workspaceStyleClipboardName,
  );
  if (
    replace &&
    !globalThis.confirm?.(`Style "${workspaceStyleClipboardName}" already exists. Replace?`)
  ) {
    return;
  }
  const changed = await mutate(
    "paste_workspace_style",
    { input: { sceneUid: scene.uid, replace } },
    () => {},
    Boolean(scene.active),
  );
  if (!changed) return;
  if (scene.active) {
    selectedStyleIndex = activeStyles().findIndex(
      (style) => style.localIndex !== null && style.name === workspaceStyleClipboardName,
    );
    workspaceSelectionKind = "style";
  }
  renderShell();
}

async function updateWorkspaceStyle(style, update) {
  if (!invoke || !style || style.localIndex === null) return false;
  let properties;
  try {
    properties = await invoke("get_style_properties", {
      sceneUid: style.sceneUid,
      localIndex: style.localIndex,
    });
  } catch (error) {
    showError(error);
    return false;
  }
  return mutateWithoutViewport("set_style_properties", {
    properties: {
      sceneUid: properties.sceneUid,
      localIndex: properties.localIndex,
      name: update.name ?? properties.name,
      readOnly: update.readOnly ?? properties.readOnly,
      xml: properties.xml,
    },
  });
}

async function loadWorkspaceStyleFile(scene) {
  if (!dialog?.open) {
    showError("The Tauri dialog plugin is unavailable.");
    return;
  }
  const path = await dialog.open({
    multiple: false,
    directory: false,
    filters: [{ name: "Style file", extensions: ["xml"] }],
  });
  if (!path) return;
  const previous = new Set(
    activeStyles()
      .filter((style) => style.localIndex !== null)
      .map((style) => style.name),
  );
  if (!(await mutate("load_style_file", { sceneUid: scene.uid, path }))) return;
  const loaded = activeStyles().find(
    (style) => style.localIndex !== null && !previous.has(style.name),
  );
  selectedStyleIndex = loaded ? activeStyles().indexOf(loaded) : null;
  workspaceSelectionKind = loaded ? "style" : "styleRoot";
  renderShell();
}

async function saveWorkspaceStyleFile(scene, style, saveAs) {
  let path = !saveAs ? style.source : "";
  if (!path) {
    const baseName = style.name === "(anonymous)" ? "style" : style.name;
    path = await chooseOutputPath(`${baseName}.xml`, "Style file", ["xml"]);
  }
  if (!path) return;
  if (
    await mutateWithoutViewport("save_style_file", {
      sceneUid: scene.uid,
      localIndex: style.localIndex,
      path,
    })
  ) {
    selectedStyleIndex = activeStyles().findIndex(
      (candidate) => candidate.localIndex === style.localIndex,
    );
    workspaceSelectionKind = "style";
    renderShell();
  }
}

function workspaceStyleContextItems(scene, style = null) {
  const activate = async () => {
    if (scene.active) return true;
    return mutate("activate_scene", { sceneUid: scene.uid });
  };
  return [
    {
      label: "New Style...",
      action: async () => {
        if (!(await activate())) return;
        const used = new Set(
          activeStyles()
            .filter((candidate) => candidate.localIndex !== null)
            .map((candidate) => candidate.name),
        );
        let index = 0;
        while (used.has(`style_${index}`)) index += 1;
        const name = globalThis.prompt?.("Style name", `style_${index}`)?.trim();
        if (name) await mutateWithoutViewport("create_style", { name });
      },
    },
    {
      label: "Copy",
      disabled: !style || style.localIndex === null,
      action: () => copyWorkspaceStyle(scene.uid, style),
    },
    {
      label: "Paste",
      disabled: !workspaceStyleClipboardName,
      action: () => pasteWorkspaceStyle(scene),
    },
    {
      label: "Delete",
      disabled: !style || style.localIndex === null,
      action: () =>
        mutateWithoutViewport("destroy_style", {
          sceneUid: style.sceneUid,
          localIndex: style.localIndex,
        }),
    },
    null,
    {
      label: "Style file",
      children: [
        {
          label: "Load ...",
          action: async () => {
            if (!(await activate())) return;
            await loadWorkspaceStyleFile(scene);
          },
        },
        {
          label: "Reload",
          disabled: !style?.source,
          action: () => globalThis.alert?.("Not implemented!!"),
        },
        {
          label: "Save",
          disabled: !style || style.localIndex === null,
          action: async () => {
            if (!(await activate())) return;
            await saveWorkspaceStyleFile(scene, style, false);
          },
        },
        {
          label: "Save As...",
          disabled: !style || style.localIndex === null,
          action: async () => {
            if (!(await activate())) return;
            await saveWorkspaceStyleFile(scene, style, true);
          },
        },
      ],
    },
    null,
    {
      label: "Read-only",
      checked: Boolean(style?.readOnly),
      disabled:
        !style || style.localIndex === null || (!style.readOnly && Boolean(style.modified)),
      action: () => updateWorkspaceStyle(style, { readOnly: !style.readOnly }),
    },
    {
      label: "Rename...",
      disabled: !style || style.localIndex === null,
      action: async () => {
        const name = globalThis.prompt?.("Style name", style.name)?.trim();
        if (name && name !== style.name) await updateWorkspaceStyle(style, { name });
      },
    },
    {
      label: "Edit...",
      disabled: !style || style.localIndex === null,
      action: async () => {
        if (!(await activate())) return;
        await showStyleProperties(style);
      },
    },
  ];
}

function workspaceObjectContextItems(scene, object) {
  const select = (selection) => () =>
    runMoleculeSelectionAction(object.uid, selection, "select");
  const current = object.selection || "";
  const items = [
    { label: "Selection · Select All", action: select("*") },
    { label: "Selection · Unselect", action: select("") },
    { label: "Selection · Invert", action: select(invertWorkspaceSelection(current)) },
    {
      label: "Selection · Toggle Sidechain",
      disabled: !current,
      action: select(toggleWorkspaceSidechain(current)),
    },
  ];
  for (const [label, selection] of [
    ["Protein", "protein"],
    ["Nucleic acid", "nucleic"],
    ["Water", "water"],
    ["Ligand", "!protein & !nucleic & !water"],
    ["Sugar", "sugar"],
    ["Hydrogen", "hydrogen"],
  ]) {
    items.push({ label: `Selection · ${label}`, action: select(selection) });
  }
  for (const byResidue of [true, false]) {
    for (const radius of [3, 5, 7, 10]) {
      items.push({
        label: `Selection · ${byResidue ? "Around Byresid" : "Around"} · ${radius} Å`,
        disabled: !current,
        action: select(aroundWorkspaceSelection(current, radius, byResidue)),
      });
    }
  }
  items.push(
    null,
    {
      label: "Copy Object",
      action: () => copyWorkspaceObject(object.uid),
    },
    {
      label: "Paste Renderer",
      disabled: !workspaceRendererClipboardAvailable,
      action: () => pasteWorkspaceRenderers(object.uid),
    },
    null,
    {
      label: "New Renderer...",
      action: async () => {
        if (!scene.active && !(await mutate("activate_scene", { sceneUid: scene.uid }))) return;
        await mutate("add_renderer", {
          objectUid: object.uid,
          rendererType: rendererType.value,
          groupName: null,
        });
      },
    },
    {
      label: "New Group...",
      action: async () => {
        const used = new Set(
          object.renderers
            .filter((renderer) => renderer.rendererType === "*group")
            .map((renderer) => renderer.name),
        );
        let serial = 1;
        while (used.has(`group${serial}`)) serial += 1;
        const name = globalThis.prompt?.("Name for new group", `group${serial}`)?.trim();
        if (!name) return;
        await mutateWithoutViewport("create_workspace_renderer_group", {
          input: { objectUid: object.uid, name },
        });
      },
    },
    {
      label: "Save As ...",
      action: async () => {
        if (!scene.active && !(await mutate("activate_scene", { sceneUid: scene.uid }))) return;
        selectedObjectUid = object.uid;
        workspaceSelectionKind = "object";
        showReferenceDialog("Save File As");
      },
    },
    {
      label: "Delete Object",
      action: () =>
        mutate("delete_workspace_item", {
          target: { kind: "object", objectUid: object.uid, rendererUid: null },
        }),
    },
    null,
    {
      label: "Properties...",
      action: async () => {
        if (!scene.active) await mutate("activate_scene", { sceneUid: scene.uid });
        showReferenceDialog("Object Properties");
      },
    },
  );
  return items;
}

function selectWorkspaceAuxItem(kind, objectUid, rendererUid = null) {
  selectedObjectUid = null;
  selectedRendererUid = null;
  selectedDensityMapUid = kind.startsWith("density") ? objectUid : null;
  selectedDensityRendererUid = kind === "densityRenderer" ? rendererUid : null;
  selectedSurfaceUid = kind.startsWith("surface") ? objectUid : null;
  selectedSurfaceRendererUid = kind === "surfaceRenderer" ? rendererUid : null;
  workspaceSelectionKind = kind;
}

function workspaceAuxObjectContextItems(scene, kind, object) {
  const isDensityMap = kind === "densityMap";
  const activate = async () => {
    if (snapshot.activeSceneUid === scene.uid) return true;
    return mutate("activate_scene", { sceneUid: scene.uid });
  };
  const items = [
    {
      label: "New Renderer...",
      action: async () => {
        if (!(await activate())) return;
        if (isDensityMap) {
          await mutate("add_density_renderer", {
            mapUid: object.uid,
            rendererType: rendererType.value,
          });
        } else {
          await mutate("add_surface_renderer", { surfaceUid: object.uid });
        }
      },
    },
    {
      label: "Delete Object",
      action: () =>
        mutate("delete_workspace_item", {
          target: { kind, objectUid: object.uid, rendererUid: null },
        }),
    },
    null,
    {
      label: "Properties...",
      action: async () => {
        if (!(await activate())) return;
        selectWorkspaceAuxItem(kind, object.uid);
        showReferenceDialog("Object Properties");
      },
    },
  ];
  if (!isDensityMap && object.originalMolecule) {
    items.splice(1, 0, {
      label: "Regenerate surface...",
      action: async () => {
        if (!(await activate())) return;
        await mutate("regenerate_surface", { surfaceUid: object.uid });
      },
    });
  }
  return items;
}

function workspaceAuxRendererContextItems(scene, kind, object, renderer) {
  const activate = async () => {
    if (snapshot.activeSceneUid === scene.uid) return true;
    return mutate("activate_scene", { sceneUid: scene.uid });
  };
  const items = [
    {
      label: "Delete",
      action: () =>
        mutate("delete_workspace_item", {
          target: { kind, objectUid: object.uid, rendererUid: renderer.uid },
        }),
    },
    null,
    {
      label: "Properties...",
      action: async () => {
        if (!(await activate())) return;
        selectWorkspaceAuxItem(kind, object.uid, renderer.uid);
        showReferenceDialog("Renderer Properties");
      },
    },
  ];
  if (kind === "densityRenderer" && renderer.rendererType === "isosurf") {
    items.unshift(
      {
        label: "Generate surface obj",
        action: async () => {
          if (!(await activate())) return;
          await mutate("generate_density_surface", {
            mapUid: object.uid,
            rendererUid: renderer.uid,
          });
        },
      },
      null,
    );
  }
  return items;
}

function workspaceStyleSignature(sceneUid) {
  return JSON.stringify(
    (snapshot?.styles || [])
      .filter((style) => style.ownerSceneUid === sceneUid)
      .map((style) => [style.name, style.source, style.modified]),
  );
}

async function cachedWorkspaceRendererStyles(sceneUid, rendererType) {
  if (!invoke) return [];
  const key = `${sceneUid}:${rendererType}`;
  const signature = workspaceStyleSignature(sceneUid);
  const cached = workspaceRendererStyleCache.get(key);
  if (cached?.signature === signature) return cached.options;
  try {
    const options = await invoke("get_renderer_style_options", {
      sceneUid,
      rendererType,
    });
    workspaceRendererStyleCache.set(key, { signature, options });
    return options;
  } catch (error) {
    showError(error);
    return [];
  }
}

async function workspaceRendererContextItems(scene, object, renderer) {
  const items = [];
  if (renderer.rendererType === "*group") {
    items.push(
      {
        label: "Change Name...",
        action: async () => {
          const name = globalThis.prompt?.("Change renderer group name", renderer.name)?.trim();
          if (!name || name === renderer.name) return;
          await mutateWithoutViewport("rename_workspace_renderer_group", {
            input: { objectUid: object.uid, groupUid: renderer.uid, name },
          });
        },
      },
      {
        label: "Copy",
        action: () => copyWorkspaceRenderer(object.uid, renderer.uid),
      },
      {
        label: "Paste Renderer",
        disabled: !workspaceRendererClipboardAvailable,
        action: () => pasteWorkspaceRenderers(object.uid, renderer.uid),
      },
      null,
      {
        label: "New Renderer...",
        action: () =>
          mutate("add_renderer", {
            objectUid: object.uid,
            rendererType: rendererType.value,
            groupName: renderer.name,
          }),
      },
      {
        label: "Delete",
        action: () =>
          mutate("delete_workspace_item", {
            target: {
              kind: "rendererGroup",
              objectUid: object.uid,
              rendererUid: renderer.uid,
            },
          }),
      },
    );
    return items;
  }
  const styleOptions =
    renderer.rendererType === "*selection"
      ? []
      : await cachedWorkspaceRendererStyles(scene.uid, renderer.rendererType);
  const selectionReadonly = ["*selection", "selection", "*namelabel", "name_label"].includes(
    renderer.rendererType,
  );
  if (!selectionReadonly) {
    for (const [label, selection] of [
      ["Current", object.selection || ""],
      ["All", "*"],
      ["Protein", "protein"],
      ["Nucleic acid", "nucleic"],
      ["Water", "water"],
      ["Ligand", "!protein & !nucleic & !water"],
      ["Sugar", "sugar"],
    ]) {
      items.push({
        label: `Change selection · ${label}`,
        disabled: label === "Current" && !selection,
        action: () =>
          mutate("set_workspace_renderer_selection", {
            input: { objectUid: object.uid, rendererUid: renderer.uid, selection },
          }),
      });
    }
    items.push(null);
  }
  const typeChangeAllowed =
    renderer.rendererType === "*selection" ||
    (!renderer.rendererType.startsWith("*") &&
      !["atomintr", "disorder"].includes(renderer.rendererType));
  if (typeChangeAllowed) {
    for (const targetType of MOLECULE_RENDERER_TYPES) {
      if (targetType === renderer.rendererType || ["atomintr", "disorder"].includes(targetType)) {
        continue;
      }
      items.push({
        label: `Change type · ${targetType}`,
        action: () => changeWorkspaceRendererType(object, renderer, targetType),
      });
    }
    items.push(null);
  }
  if (renderer.rendererType !== "*selection") {
    const lowerType = renderer.rendererType.replace(/^\*/, "").toLowerCase();
    const suffix = ["name_label", "namelabel"].includes(lowerType)
      ? "label"
      : lowerType === "gpu_mapmesh"
        ? "contour"
        : lowerType;
    const shapeStyles = styleOptions.filter((style) => style.name.toLowerCase().endsWith(suffix));
    const edgeStyles = styleOptions.filter((style) => style.name.toLowerCase().startsWith("egline"));
    const paintStyles = styleOptions.filter((style) => style.name.toLowerCase().endsWith("paint"));
    const applyStyle = (styleName, component) => () =>
      mutate("apply_workspace_renderer_style", {
        input: {
          objectUid: object.uid,
          rendererUid: renderer.uid,
          styleName,
          component,
        },
      });
    const supportsColoring = !["selection", "*selection", "name_label", "*namelabel", "atomintr"].includes(
      renderer.rendererType,
    );
    if (supportsColoring) {
      for (const [label, styleName] of [
        ["CPK molcol", "DefaultCPKColoring"],
        ["CPK dark gray", "DarkCPKColoring"],
        ["CPK light gray", "LightCPKColoring"],
      ]) {
        if (styleOptions.some((style) => style.name.toLowerCase() === styleName.toLowerCase())) {
          items.push({ label: `Coloring · ${label}`, action: applyStyle(styleName, "coloring") });
        }
      }
      for (const style of paintStyles) {
        items.push({
          label: `Coloring · ${style.description || style.name}`,
          action: applyStyle(style.name, "coloring"),
        });
      }
      for (const [label, preset] of [["B-factor", "bfactor"], ["Rainbow", "rainbow"]]) {
        items.push({
          label: `Coloring · ${label}`,
          action: () =>
            mutate("set_workspace_renderer_coloring", {
              input: { objectUid: object.uid, rendererUid: renderer.uid, preset },
            }),
        });
      }
    }
    if (renderer.coloringType === "PaintColoring") {
      const addPaint = (group, label, color) => {
        items.push({
          label: `Paint · ${group} · ${label}`,
          action: () =>
            mutate("insert_workspace_renderer_paint", {
              input: { objectUid: object.uid, rendererUid: renderer.uid, color },
            }),
        });
      };
      for (const [label, color] of WORKSPACE_PAINT_MONOCHROME) {
        addPaint("Monochrome", label, color);
      }
      for (const [hueName, hue] of WORKSPACE_PAINT_HUES) {
        for (const [label, saturation, brightness] of [
          [hueName, "1.0", "1.0"],
          [`${hueName}, sat 25%`, "0.25", "1.0"],
          [`${hueName}, sat 50%`, "0.5", "1.0"],
          [`${hueName}, sat 75%`, "0.75", "1.0"],
          [`${hueName}, bri 75%`, "1.0", "0.75"],
          [`${hueName}, bri 50%`, "1.0", "0.50"],
          [`${hueName}, bri 25%`, "1.0", "0.25"],
        ]) {
          addPaint(hueName, label, `hsb(${hue}, ${saturation}, ${brightness})`);
        }
      }
    } else {
      items.push({ label: "Paint", disabled: true });
    }
    for (const style of shapeStyles) {
      items.push({
        label: `Style · ${style.description || style.name}`,
        action: applyStyle(style.name, "shape"),
      });
    }
    for (const style of edgeStyles) {
      items.push({
        label: `Style · ${style.description || style.name}`,
        action: applyStyle(style.name, "edge"),
      });
    }
    items.push({ label: "Edit style...", action: () => showReferenceDialog("Renderer Properties") });
  }
  items.push({
    label: "Create style...",
    action: async () => {
      if (!scene.active) await mutate("activate_scene", { sceneUid: scene.uid });
      await rendererStyleCreateEditor(object, renderer);
    },
  });
  if (renderer.rendererType === "atomintr") {
    items.push(null, {
      label: "Edit interaction list...",
      action: () => showReferenceDialog("Renderer Properties"),
    });
  }
  items.push(
    null,
    {
      label: "Copy",
      action: () => copyWorkspaceRenderer(object.uid, renderer.uid),
    },
    null,
    {
      label: "New Renderer...",
      action: () =>
        mutate("add_renderer", {
          objectUid: object.uid,
          rendererType: rendererType.value,
          groupName: null,
        }),
    },
    {
      label: "Delete",
      action: () =>
        mutate("delete_workspace_item", {
          target: {
            kind: "renderer",
            objectUid: object.uid,
            rendererUid: renderer.uid,
          },
        }),
    },
    null,
    { label: "Properties...", action: () => showReferenceDialog("Renderer Properties") },
  );
  return items;
}

function clearWorkspaceRendererDropMarkers() {
  document
    .querySelectorAll(".tree-renderer.drop-before, .tree-renderer.drop-after, .tree-renderer.drop-inside")
    .forEach((row) => row.classList.remove("drop-before", "drop-after", "drop-inside"));
}

function workspaceRendererDropPosition(event, renderer) {
  const bounds = event.currentTarget.getBoundingClientRect();
  const fraction = bounds.height > 0 ? (event.clientY - bounds.top) / bounds.height : 0.5;
  if (
    renderer.rendererType === "*group" &&
    !draggedWorkspaceRenderer?.isGroup &&
    fraction >= 0.25 &&
    fraction <= 0.75
  ) {
    return "inside";
  }
  return fraction < 0.5 ? "before" : "after";
}

function canDropWorkspaceRenderer(object, renderer, position) {
  const source = draggedWorkspaceRenderer;
  if (!source || source.objectUid !== object.uid || source.rendererUid === renderer.uid) return false;
  if (position === "inside") return !source.isGroup && renderer.rendererType === "*group";
  if (source.isGroup && renderer.group) return false;
  return true;
}

function selectedDensityMap() {
  return snapshot?.densityMaps.find((map) => map.uid === selectedDensityMapUid) || null;
}

function selectedDensityRenderer() {
  return (
    selectedDensityMap()?.renderers.find(
      (renderer) => renderer.uid === selectedDensityRendererUid,
    ) || null
  );
}

function selectedSurface() {
  return activeSurfaces().find((surface) => surface.uid === selectedSurfaceUid) || null;
}

function selectedSurfaceRenderer() {
  return (
    selectedSurface()?.renderers.find(
      (renderer) => renderer.uid === selectedSurfaceRendererUid,
    ) || null
  );
}

function selectedWorkspaceTarget() {
  if (workspaceSelectionKind === "object") {
    const object = selectedObject();
    return object ? { kind: "object", objectUid: object.uid, rendererUid: null } : null;
  }
  if (["renderer", "rendererGroup"].includes(workspaceSelectionKind)) {
    const object = selectedObject();
    const renderer = selectedRenderer();
    return object && renderer
      ? { kind: workspaceSelectionKind, objectUid: object.uid, rendererUid: renderer.uid }
      : null;
  }
  if (workspaceSelectionKind === "densityMap") {
    const map = selectedDensityMap();
    return map ? { kind: "densityMap", objectUid: map.uid, rendererUid: null } : null;
  }
  if (workspaceSelectionKind === "densityRenderer") {
    const map = selectedDensityMap();
    const renderer = selectedDensityRenderer();
    return map && renderer
      ? { kind: "densityRenderer", objectUid: map.uid, rendererUid: renderer.uid }
      : null;
  }
  if (workspaceSelectionKind === "surface") {
    const surface = selectedSurface();
    return surface ? { kind: "surface", objectUid: surface.uid, rendererUid: null } : null;
  }
  if (workspaceSelectionKind === "surfaceRenderer") {
    const surface = selectedSurface();
    const renderer = selectedSurfaceRenderer();
    return surface && renderer
      ? { kind: "surfaceRenderer", objectUid: surface.uid, rendererUid: renderer.uid }
      : null;
  }
  return null;
}

function normalizeSelection() {
  if (workspaceSelectionKind === "camera" && !selectedCamera()) {
    selectedCameraName = null;
    workspaceSelectionKind = "cameraRoot";
  }
  if (workspaceSelectionKind === "style" && !selectedStyle()) {
    selectedStyleIndex = null;
    workspaceSelectionKind = "styleRoot";
  }
  if (["densityMap", "densityRenderer"].includes(workspaceSelectionKind)) {
    if (!selectedDensityMap()) {
      selectedDensityMapUid = null;
      selectedDensityRendererUid = null;
      workspaceSelectionKind = snapshot.objects.length > 0 ? "object" : "scene";
    } else if (workspaceSelectionKind === "densityRenderer" && !selectedDensityRenderer()) {
      selectedDensityRendererUid = null;
      workspaceSelectionKind = "densityMap";
    }
  }
  if (["surface", "surfaceRenderer"].includes(workspaceSelectionKind)) {
    if (!selectedSurface()) {
      selectedSurfaceUid = null;
      selectedSurfaceRendererUid = null;
      workspaceSelectionKind = snapshot.objects.length > 0 ? "object" : "scene";
    } else if (workspaceSelectionKind === "surfaceRenderer" && !selectedSurfaceRenderer()) {
      selectedSurfaceRendererUid = null;
      workspaceSelectionKind = "surface";
    }
  }
  const objects = activeObjects();
  if (!objects.some((object) => object.uid === selectedObjectUid)) {
    selectedObjectUid = objects[0]?.uid ?? null;
    selectedRendererUid = null;
    if (
      ![
        "scene",
        "cameraRoot",
        "camera",
        "styleRoot",
        "style",
        "densityMap",
        "densityRenderer",
        "surface",
        "surfaceRenderer",
      ].includes(workspaceSelectionKind)
    ) {
      const surface = activeSurfaces()[0];
      const map = snapshot.densityMaps.find(
        (candidate) => candidate.sceneUid === snapshot.activeSceneUid,
      );
      if (objects.length > 0) {
        workspaceSelectionKind = "object";
      } else if (surface) {
        selectedSurfaceUid = surface.uid;
        workspaceSelectionKind = "surface";
      } else if (map) {
        selectedDensityMapUid = map.uid;
        workspaceSelectionKind = "densityMap";
      } else {
        workspaceSelectionKind = "scene";
      }
    }
  }
  if (selectedRendererUid !== null && !selectedRenderer()) {
    selectedRendererUid = null;
    if (["renderer", "rendererGroup"].includes(workspaceSelectionKind)) {
      workspaceSelectionKind = "object";
    }
  }
}

function workspaceChromeRenderKey() {
  const rendererKey = (renderer) => [
    renderer.uid,
    renderer.name,
    renderer.rendererType,
    renderer.group,
    renderer.visible,
    renderer.locked,
  ];
  return JSON.stringify({
    selection: [
      workspaceSelectionKind,
      selectedObjectUid,
      selectedRendererUid,
      selectedDensityMapUid,
      selectedDensityRendererUid,
      selectedSurfaceUid,
      selectedSurfaceRendererUid,
      selectedCameraName,
      selectedStyleIndex,
    ],
    scenes: snapshot.scenes.map((scene) => [
      scene.uid,
      scene.name,
      scene.active,
      scene.objectCount,
      scene.mapCount,
    ]),
    objects: snapshot.objects.map((object) => [
      object.sceneUid,
      object.uid,
      object.uiOrder,
      object.name,
      object.selection,
      object.visible,
      object.locked,
      object.atomCount,
      object.residueCount,
      object.bondCount,
      object.renderers.map(rendererKey),
    ]),
    maps: snapshot.densityMaps.map((map) => [
      map.sceneUid,
      map.uid,
      map.uiOrder,
      map.name,
      map.visible,
      map.locked,
      map.counts,
      map.renderers.map(rendererKey),
    ]),
    surfaces: (snapshot.surfaces || []).map((surface) => [
      surface.sceneUid,
      surface.uid,
      surface.uiOrder,
      surface.name,
      surface.visible,
      surface.locked,
      surface.className,
      surface.format,
      surface.renderers.map(rendererKey),
    ]),
    cameras: (snapshot.cameras || []).map((camera) => [
      camera.sceneUid,
      camera.name,
      camera.source,
      camera.visibilityCount,
    ]),
    styles: (snapshot.styles || []).map((style) => [
      style.ownerSceneUid,
      style.sceneUid,
      style.localIndex,
      style.name,
      style.source,
      style.readOnly,
      style.modified,
    ]),
  });
}

function renderSceneTabs(renderKey = workspaceChromeRenderKey()) {
  if (renderedSceneTabsKey === renderKey) return;
  renderedSceneTabsKey = renderKey;
  sceneTabs.replaceChildren();
  for (const scene of snapshot.scenes) {
    const tab = button(scene.name || `Scene ${scene.uid}`, `scene-tab${scene.active ? " active" : ""}`, () => {
      if (!scene.active) mutate("activate_scene", { sceneUid: scene.uid });
    });
    tab.title = `${scene.objectCount} objects / ${scene.mapCount} maps`;
    sceneTabs.append(tab);
  }
}

function renderSceneTree(renderKey = workspaceChromeRenderKey()) {
  if (renderedSceneTreeKey === renderKey) return;
  renderedSceneTreeKey = renderKey;
  sceneTree.replaceChildren();
  if (snapshot.scenes.length === 0) {
    workspaceProperties.disabled = true;
    workspaceDelete.disabled = true;
    workspaceZoom.disabled = true;
    const empty = document.createElement("div");
    empty.className = "tree-empty";
    empty.textContent = "No scenes";
    sceneTree.append(empty);
    return;
  }
  for (const scene of snapshot.scenes) {
    const sceneRow = button(
      scene.name || `Scene ${scene.uid}`,
      `tree-scene${workspaceSelectionKind === "scene" && scene.active ? " active" : ""}`,
      async () => {
        workspaceSelectionKind = "scene";
        selectedRendererUid = null;
        selectedDensityMapUid = null;
        selectedDensityRendererUid = null;
        selectedSurfaceUid = null;
        selectedSurfaceRendererUid = null;
        selectedCameraName = null;
        selectedStyleIndex = null;
        if (!scene.active) {
          await mutate("activate_scene", { sceneUid: scene.uid });
        } else {
          renderShell();
        }
      },
    );
    sceneRow.addEventListener("dblclick", () => showReferenceDialog("Scene Properties"));
    sceneRow.addEventListener("contextmenu", (event) => {
      workspaceSelectionKind = "scene";
      selectedObjectUid = null;
      selectedRendererUid = null;
      selectedDensityMapUid = null;
      selectedDensityRendererUid = null;
      selectedSurfaceUid = null;
      selectedSurfaceRendererUid = null;
      selectedCameraName = null;
      selectedStyleIndex = null;
      showWorkspaceContextMenu(event, workspaceSceneContextItems(scene));
    });
    sceneTree.append(sceneRow);
    const workspaceObjectBlocks = [];
    for (const object of snapshot.objects.filter((item) => item.sceneUid === scene.uid)) {
      const objectBlock = document.createDocumentFragment();
      const objectRow = button(
        object.name,
        `tree-object${
          workspaceSelectionKind === "object" && object.uid === selectedObjectUid ? " active" : ""
        }${object.visible ? "" : " hidden"}`,
        () => {
          selectedObjectUid = object.uid;
          selectedRendererUid = null;
          selectedDensityMapUid = null;
          selectedDensityRendererUid = null;
          selectedSurfaceUid = null;
          selectedSurfaceRendererUid = null;
          workspaceSelectionKind = "object";
          renderShell();
        },
      );
      objectRow.addEventListener("dblclick", () => showReferenceDialog("Object Properties"));
      objectRow.addEventListener("contextmenu", (event) => {
        selectedObjectUid = object.uid;
        selectedRendererUid = null;
        selectedDensityMapUid = null;
        selectedDensityRendererUid = null;
        selectedSurfaceUid = null;
        selectedSurfaceRendererUid = null;
        workspaceSelectionKind = "object";
        showWorkspaceContextMenu(event, workspaceObjectContextItems(scene, object));
      });
      objectRow.prepend(
        workspaceStateControls(
          { kind: "object", objectUid: object.uid },
          object.visible,
          object.locked,
        ),
      );
      const count = document.createElement("small");
      count.textContent = `${object.atomCount} AT`;
      objectRow.append(count);
      configureWorkspaceObjectDrag(objectRow, scene.uid, "object", object.uid);
      objectBlock.append(objectRow);
      const appendMoleculeRendererRow = (renderer, isChild = false) => {
        const isGroup = renderer.rendererType === "*group";
        const selectionKind = isGroup ? "rendererGroup" : "renderer";
        const rendererRow = button(
          isGroup ? `${renderer.name} (group)` : `${renderer.rendererType} · ${renderer.name}`,
          `tree-renderer${
            workspaceSelectionKind === selectionKind && renderer.uid === selectedRendererUid
              ? " active"
              : ""
          }${isGroup ? " renderer-group" : ""}${isChild ? " group-child" : ""}${
            renderer.visible ? "" : " hidden"
          }`,
          () => {
            selectedObjectUid = object.uid;
            selectedRendererUid = renderer.uid;
            selectedDensityMapUid = null;
            selectedDensityRendererUid = null;
            selectedSurfaceUid = null;
            selectedSurfaceRendererUid = null;
            workspaceSelectionKind = selectionKind;
            renderShell();
          },
        );
        rendererRow.draggable = true;
        rendererRow.addEventListener("dragstart", (event) => {
          draggedWorkspaceRenderer = {
            objectUid: object.uid,
            rendererUid: renderer.uid,
            isGroup,
          };
          rendererRow.classList.add("dragging");
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData(
            "application/x-cuemol-renderer",
            JSON.stringify(draggedWorkspaceRenderer),
          );
        });
        rendererRow.addEventListener("dragover", (event) => {
          const position = workspaceRendererDropPosition(event, renderer);
          if (!canDropWorkspaceRenderer(object, renderer, position)) return;
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          clearWorkspaceRendererDropMarkers();
          rendererRow.classList.add(`drop-${position}`);
          rendererRow.dataset.dropPosition = position;
        });
        rendererRow.addEventListener("dragleave", (event) => {
          if (!rendererRow.contains(event.relatedTarget)) {
            rendererRow.classList.remove("drop-before", "drop-after", "drop-inside");
          }
        });
        rendererRow.addEventListener("drop", async (event) => {
          const source = draggedWorkspaceRenderer;
          const position = rendererRow.dataset.dropPosition || "before";
          clearWorkspaceRendererDropMarkers();
          if (!source || !canDropWorkspaceRenderer(object, renderer, position)) return;
          event.preventDefault();
          draggedWorkspaceRenderer = null;
          await mutateWithoutViewport("move_workspace_renderer", {
            input: {
              objectUid: object.uid,
              rendererUid: source.rendererUid,
              targetUid: renderer.uid,
              position,
            },
          });
        });
        rendererRow.addEventListener("dragend", () => {
          draggedWorkspaceRenderer = null;
          rendererRow.classList.remove("dragging");
          clearWorkspaceRendererDropMarkers();
        });
        rendererRow.addEventListener("dblclick", () =>
          showReferenceDialog("Renderer Properties"),
        );
        rendererRow.addEventListener("contextmenu", async (event) => {
          event.preventDefault();
          selectedObjectUid = object.uid;
          selectedRendererUid = renderer.uid;
          workspaceSelectionKind = isGroup ? "rendererGroup" : "renderer";
          showWorkspaceContextMenu(
            event,
            await workspaceRendererContextItems(scene, object, renderer),
          );
        });
        rendererRow.prepend(
          workspaceStateControls(
            {
              kind: selectionKind,
              objectUid: object.uid,
              rendererUid: renderer.uid,
            },
            renderer.visible,
            renderer.locked,
            !isGroup,
          ),
        );
        objectBlock.append(rendererRow);
      };
      for (const node of groupedMoleculeRenderers(object.renderers)) {
        appendMoleculeRendererRow(node.renderer);
        for (const child of node.children) appendMoleculeRendererRow(child, true);
      }
      workspaceObjectBlocks.push({
        uiOrder: object.uiOrder,
        kindOrder: 0,
        uid: object.uid,
        block: objectBlock,
      });
    }
    for (const surface of (snapshot.surfaces || []).filter(
      (item) => item.sceneUid === scene.uid,
    )) {
      const surfaceBlock = document.createDocumentFragment();
      const surfaceRow = button(
        surface.name,
        `tree-object surface-object${
          workspaceSelectionKind === "surface" && surface.uid === selectedSurfaceUid
            ? " active"
            : ""
        }${surface.visible ? "" : " hidden"}`,
        () => {
          selectedObjectUid = null;
          selectedRendererUid = null;
          selectedDensityMapUid = null;
          selectedDensityRendererUid = null;
          selectedSurfaceUid = surface.uid;
          selectedSurfaceRendererUid = null;
          workspaceSelectionKind = "surface";
          renderShell();
        },
      );
      surfaceRow.addEventListener("dblclick", () => showReferenceDialog("Object Properties"));
      surfaceRow.addEventListener("contextmenu", (event) => {
        selectWorkspaceAuxItem("surface", surface.uid);
        showWorkspaceContextMenu(
          event,
          workspaceAuxObjectContextItems(scene, "surface", surface),
        );
      });
      surfaceRow.prepend(
        workspaceStateControls(
          { kind: "surface", objectUid: surface.uid },
          surface.visible,
          surface.locked,
        ),
      );
      const count = document.createElement("small");
      count.textContent = "SURF";
      surfaceRow.append(count);
      configureWorkspaceObjectDrag(surfaceRow, scene.uid, "surface", surface.uid);
      surfaceBlock.append(surfaceRow);
      for (const renderer of surface.renderers) {
        const rendererRow = button(
          `${renderer.rendererType} · ${renderer.name}`,
          `tree-renderer surface-renderer${
            workspaceSelectionKind === "surfaceRenderer" &&
            renderer.uid === selectedSurfaceRendererUid
              ? " active"
              : ""
          }${renderer.visible ? "" : " hidden"}`,
          () => {
            selectedObjectUid = null;
            selectedRendererUid = null;
            selectedDensityMapUid = null;
            selectedDensityRendererUid = null;
            selectedSurfaceUid = surface.uid;
            selectedSurfaceRendererUid = renderer.uid;
            workspaceSelectionKind = "surfaceRenderer";
            renderShell();
          },
        );
        rendererRow.addEventListener("dblclick", () =>
          showReferenceDialog("Renderer Properties"),
        );
        rendererRow.addEventListener("contextmenu", (event) => {
          selectWorkspaceAuxItem("surfaceRenderer", surface.uid, renderer.uid);
          showWorkspaceContextMenu(
            event,
            workspaceAuxRendererContextItems(scene, "surfaceRenderer", surface, renderer),
          );
        });
        rendererRow.prepend(
          workspaceStateControls(
            {
              kind: "surfaceRenderer",
              objectUid: surface.uid,
              rendererUid: renderer.uid,
            },
            renderer.visible,
            renderer.locked,
          ),
        );
        surfaceBlock.append(rendererRow);
      }
      workspaceObjectBlocks.push({
        uiOrder: surface.uiOrder,
        kindOrder: 2,
        uid: surface.uid,
        block: surfaceBlock,
      });
    }
    for (const map of snapshot.densityMaps.filter((item) => item.sceneUid === scene.uid)) {
      const mapBlock = document.createDocumentFragment();
      const mapRow = button(
        map.name,
        `tree-object density-map${
          workspaceSelectionKind === "densityMap" && map.uid === selectedDensityMapUid
            ? " active"
            : ""
        }${map.visible ? "" : " hidden"}`,
        () => {
          selectedObjectUid = null;
          selectedRendererUid = null;
          selectedDensityMapUid = map.uid;
          selectedDensityRendererUid = null;
          selectedSurfaceUid = null;
          selectedSurfaceRendererUid = null;
          workspaceSelectionKind = "densityMap";
          renderShell();
        },
      );
      mapRow.addEventListener("dblclick", () => showReferenceDialog("Object Properties"));
      mapRow.addEventListener("contextmenu", (event) => {
        selectWorkspaceAuxItem("densityMap", map.uid);
        showWorkspaceContextMenu(
          event,
          workspaceAuxObjectContextItems(scene, "densityMap", map),
        );
      });
      mapRow.prepend(
        workspaceStateControls(
          { kind: "densityMap", objectUid: map.uid },
          map.visible,
          map.locked,
        ),
      );
      const count = document.createElement("small");
      count.textContent = `${map.counts.join("×")} MAP`;
      mapRow.append(count);
      configureWorkspaceObjectDrag(mapRow, scene.uid, "densityMap", map.uid);
      mapBlock.append(mapRow);
      for (const renderer of map.renderers) {
        const rendererRow = button(
          `${renderer.rendererType} · ${renderer.name}`,
          `tree-renderer density-renderer${
            workspaceSelectionKind === "densityRenderer" &&
            renderer.uid === selectedDensityRendererUid
              ? " active"
              : ""
          }${renderer.visible ? "" : " hidden"}`,
          () => {
            selectedObjectUid = null;
            selectedRendererUid = null;
            selectedDensityMapUid = map.uid;
            selectedDensityRendererUid = renderer.uid;
            selectedSurfaceUid = null;
            selectedSurfaceRendererUid = null;
            workspaceSelectionKind = "densityRenderer";
            renderShell();
          },
        );
        rendererRow.addEventListener("dblclick", () =>
          showReferenceDialog("Renderer Properties"),
        );
        rendererRow.addEventListener("contextmenu", (event) => {
          selectWorkspaceAuxItem("densityRenderer", map.uid, renderer.uid);
          showWorkspaceContextMenu(
            event,
            workspaceAuxRendererContextItems(scene, "densityRenderer", map, renderer),
          );
        });
        rendererRow.prepend(
          workspaceStateControls(
            {
              kind: "densityRenderer",
              objectUid: map.uid,
              rendererUid: renderer.uid,
            },
            renderer.visible,
            renderer.locked,
          ),
        );
        mapBlock.append(rendererRow);
      }
      workspaceObjectBlocks.push({
        uiOrder: map.uiOrder,
        kindOrder: 1,
        uid: map.uid,
        block: mapBlock,
      });
    }
    workspaceObjectBlocks.sort(
      (left, right) =>
        (left.uiOrder ?? Number.MAX_SAFE_INTEGER) -
          (right.uiOrder ?? Number.MAX_SAFE_INTEGER) ||
        left.kindOrder - right.kindOrder ||
        left.uid - right.uid,
    );
    for (const entry of workspaceObjectBlocks) sceneTree.append(entry.block);
    const sceneCameras = (snapshot.cameras || []).filter(
      (camera) => camera.sceneUid === scene.uid,
    );
    const cameraRoot = button(
      "Camera",
      `tree-object camera-root${
        workspaceSelectionKind === "cameraRoot" && scene.active ? " active" : ""
      }`,
      async () => {
        workspaceSelectionKind = "cameraRoot";
        selectedCameraName = null;
        if (!scene.active) {
          await mutate("activate_scene", { sceneUid: scene.uid });
        } else {
          renderShell();
        }
      },
    );
    const count = document.createElement("small");
    count.textContent = `${sceneCameras.length} CAM`;
    cameraRoot.append(count);
    cameraRoot.addEventListener("contextmenu", (event) => {
      workspaceSelectionKind = "cameraRoot";
      selectedCameraName = null;
      showWorkspaceContextMenu(event, workspaceCameraContextItems(scene));
    });
    sceneTree.append(cameraRoot);
    for (const camera of sceneCameras) {
      const markers = `${camera.source ? " ↗" : ""}${camera.visibilityCount > 0 ? " ◉" : ""}`;
      const cameraRow = button(
        `${camera.name}${markers}`,
        `tree-renderer camera-node${
          workspaceSelectionKind === "camera" && camera.name === selectedCameraName
            ? " active"
            : ""
        }`,
        async () => {
          workspaceSelectionKind = "camera";
          selectedCameraName = camera.name;
          if (!scene.active) {
            await mutate("activate_scene", { sceneUid: scene.uid });
          } else {
            renderShell();
          }
        },
      );
      cameraRow.addEventListener("dblclick", async () => {
        workspaceSelectionKind = "camera";
        selectedCameraName = camera.name;
        if (!scene.active) await mutate("activate_scene", { sceneUid: scene.uid });
        await mutate("load_camera", { name: camera.name });
      });
      cameraRow.addEventListener("contextmenu", (event) => {
        workspaceSelectionKind = "camera";
        selectedCameraName = camera.name;
        showWorkspaceContextMenu(event, workspaceCameraContextItems(scene, camera));
      });
      sceneTree.append(cameraRow);
    }
    const sceneStyles = (snapshot.styles || []).filter(
      (style) => style.ownerSceneUid === scene.uid,
    );
    const styleRoot = button(
      "Styles",
      `tree-object style-root${
        workspaceSelectionKind === "styleRoot" && scene.active ? " active" : ""
      }`,
      async () => {
        workspaceSelectionKind = "styleRoot";
        selectedStyleIndex = null;
        if (!scene.active) {
          await mutate("activate_scene", { sceneUid: scene.uid });
        } else {
          renderShell();
        }
      },
    );
    const styleCount = document.createElement("small");
    styleCount.textContent = `${sceneStyles.length} STYLE`;
    styleRoot.append(styleCount);
    styleRoot.addEventListener("contextmenu", (event) => {
      workspaceSelectionKind = "styleRoot";
      selectedStyleIndex = null;
      showWorkspaceContextMenu(event, workspaceStyleContextItems(scene));
    });
    sceneTree.append(styleRoot);
    sceneStyles.forEach((style, styleIndex) => {
      const markers = `${style.readOnly || style.sceneUid === 0 ? " 🔒" : ""}${
        style.source ? " ↗" : ""
      }`;
      const styleRow = button(
        `${style.name}${markers}`,
        `tree-renderer style-node${
          workspaceSelectionKind === "style" && styleIndex === selectedStyleIndex
            ? " active"
            : ""
        }`,
        async () => {
          workspaceSelectionKind = "style";
          selectedStyleIndex = styleIndex;
          if (!scene.active) {
            await mutate("activate_scene", { sceneUid: scene.uid });
          } else {
            renderShell();
          }
        },
      );
      if (style.localIndex !== null) {
        styleRow.addEventListener("dblclick", () => showStyleProperties(style));
      }
      styleRow.addEventListener("contextmenu", (event) => {
        workspaceSelectionKind = "style";
        selectedStyleIndex = styleIndex;
        showWorkspaceContextMenu(event, workspaceStyleContextItems(scene, style));
      });
      sceneTree.append(styleRow);
    });
  }
  workspaceProperties.disabled =
    ["renderer", "rendererGroup"].includes(workspaceSelectionKind)
      ? !selectedRenderer()
      : workspaceSelectionKind === "densityRenderer"
        ? !selectedDensityRenderer()
        : workspaceSelectionKind === "surfaceRenderer"
          ? !selectedSurfaceRenderer()
        : workspaceSelectionKind === "densityMap"
          ? !selectedDensityMap()
          : workspaceSelectionKind === "surface"
            ? !selectedSurface()
          : workspaceSelectionKind === "camera"
            ? !selectedCamera()
          : workspaceSelectionKind === "cameraRoot"
            ? true
          : workspaceSelectionKind === "style"
            ? !selectedStyle() || selectedStyle().localIndex === null
          : workspaceSelectionKind === "styleRoot"
            ? true
          : workspaceSelectionKind === "object"
            ? !selectedObject()
            : !snapshot.scenes.some((scene) => scene.active);
  workspaceDelete.disabled =
    ["renderer", "rendererGroup"].includes(workspaceSelectionKind)
      ? !selectedRenderer()
      : workspaceSelectionKind === "densityRenderer"
        ? !selectedDensityRenderer()
        : workspaceSelectionKind === "surfaceRenderer"
          ? !selectedSurfaceRenderer()
          : workspaceSelectionKind === "densityMap"
            ? !selectedDensityMap()
            : workspaceSelectionKind === "surface"
              ? !selectedSurface()
              : workspaceSelectionKind === "camera"
                ? !selectedCamera()
              : workspaceSelectionKind === "style"
                ? !selectedStyle() || selectedStyle().localIndex === null
              : workspaceSelectionKind === "object"
                ? !selectedObject()
                : true;
  workspaceZoom.disabled =
    workspaceDelete.disabled ||
    ["rendererGroup", "cameraRoot", "camera", "styleRoot", "style"].includes(
      workspaceSelectionKind,
    );
}

function renderInspector(renderKey = workspaceChromeRenderKey()) {
  if (renderedInspectorKey === renderKey) return;
  renderedInspectorKey = renderKey;
  const map = ["densityMap", "densityRenderer"].includes(workspaceSelectionKind)
    ? selectedDensityMap()
    : null;
  const surface = ["surface", "surfaceRenderer"].includes(workspaceSelectionKind)
    ? selectedSurface()
    : null;
  const object =
    !map && !surface && ["object", "renderer", "rendererGroup"].includes(workspaceSelectionKind)
      ? selectedObject()
      : null;
  const camera = workspaceSelectionKind === "camera" ? selectedCamera() : null;
  const style = workspaceSelectionKind === "style" ? selectedStyle() : null;
  inspectorTitle.textContent =
    object?.name || map?.name || surface?.name || camera?.name || style?.name || "No object";
  const labels = style
    ? ["Scope", "Read only", "Source"]
    : camera
    ? ["Source", "Visibility", "Kind"]
    : map
    ? ["Columns", "Rows", "Sections"]
    : surface
      ? ["Type", "Format", "Renderers"]
      : ["Atoms", "Residues", "Bonds"];
  const values = style
    ? [style.sceneUid === 0 ? "Global" : "Scene", style.readOnly, style.source || "—"]
    : camera
    ? [camera.source || "—", camera.visibilityCount, "Camera"]
    : object
    ? [object.atomCount, object.residueCount, object.bondCount]
    : map
      ? map.counts
      : surface
        ? [surface.className, surface.format || "—", surface.renderers.length]
        : ["—", "—", "—"];
  objectStats.querySelectorAll("dt").forEach((node, index) => {
    node.textContent = labels[index];
  });
  objectStats.querySelectorAll("dd").forEach((node, index) => {
    node.textContent = String(values[index]);
  });
  const rendererChoices = map
    ? [
        ["contour", "Contour"],
        ["isosurf", "Isosurface"],
        ["gpu_mapvol", "Volume"],
      ]
    : surface
      ? [["molsurf", "Molecular surface"]]
      : MOLECULE_RENDERER_TYPES.map((rendererType) => [rendererType, rendererType]);
  rendererType.replaceChildren();
  for (const [value, label] of rendererChoices) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    rendererType.append(option);
  }
  rendererList.replaceChildren();
  if (!object && !map && !surface) {
    const empty = document.createElement("div");
    empty.className = "renderer-empty";
    empty.textContent = "Select an object.";
    rendererList.append(empty);
    return;
  }
  const renderers = object?.renderers || map?.renderers || surface.renderers;
  if (renderers.length === 0) {
    const empty = document.createElement("div");
    empty.className = "renderer-empty";
    empty.textContent = "No render representations.";
    rendererList.append(empty);
  }
  const rendererEntries = object
    ? groupedMoleculeRenderers(renderers).flatMap((node) => [
        { renderer: node.renderer, child: false },
        ...node.children.map((renderer) => ({ renderer, child: true })),
      ])
    : renderers.map((renderer) => ({ renderer, child: false }));
  for (const { renderer, child } of rendererEntries) {
    const row = document.createElement("div");
    const selectedUid = map
      ? selectedDensityRendererUid
      : surface
        ? selectedSurfaceRendererUid
        : selectedRendererUid;
    row.className = `renderer-row${renderer.rendererType === "*group" ? " renderer-group" : ""}${
      child ? " group-child" : ""
    }${renderer.uid === selectedUid ? " selected" : ""}`;
    row.tabIndex = 0;
    const visible = document.createElement("input");
    visible.type = "checkbox";
    visible.checked = renderer.visible;
    visible.disabled = renderer.locked;
    visible.addEventListener("click", (event) => event.stopPropagation());
    visible.addEventListener("change", () => {
      if (map) {
        mutate("set_density_renderer_visible", {
          mapUid: map.uid,
          rendererUid: renderer.uid,
          visible: visible.checked,
        });
      } else if (surface) {
        mutate("set_surface_renderer_visible", {
          surfaceUid: surface.uid,
          rendererUid: renderer.uid,
          visible: visible.checked,
        });
      } else {
        mutate("set_renderer_visible", {
          objectUid: object.uid,
          rendererUid: renderer.uid,
          visible: visible.checked,
        });
      }
    });
    const description = document.createElement("span");
    const name = document.createElement("strong");
    name.textContent = renderer.name;
    const type = document.createElement("small");
    type.textContent = `${renderer.rendererType}${renderer.locked ? " · locked" : ""}`;
    description.append(name, type);
    row.append(visible, description);
    const selectRow = () => {
      if (map) {
        selectedDensityMapUid = map.uid;
        selectedDensityRendererUid = renderer.uid;
        workspaceSelectionKind = "densityRenderer";
      } else if (surface) {
        selectedSurfaceUid = surface.uid;
        selectedSurfaceRendererUid = renderer.uid;
        workspaceSelectionKind = "surfaceRenderer";
      } else {
        selectedRendererUid = renderer.uid;
        workspaceSelectionKind = renderer.rendererType === "*group" ? "rendererGroup" : "renderer";
      }
      const renderKey = workspaceChromeRenderKey();
      renderSceneTree(renderKey);
      renderInspector(renderKey);
    };
    row.addEventListener("click", selectRow);
    row.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") selectRow();
    });
    row.addEventListener("dblclick", () => showReferenceDialog("Renderer Properties"));
    rendererList.append(row);
  }
}

function replaceOptions(select, objects, selectedUid = selectedObjectUid) {
  select.replaceChildren();
  for (const object of objects) {
    const option = document.createElement("option");
    option.value = String(object.uid ?? object.objectUid);
    option.textContent = object.name ?? object.objectName;
    select.append(option);
  }
  select.disabled = objects.length === 0;
  if (selectedUid !== null) select.value = String(selectedUid);
}

function activeMoleculeObjects() {
  if (!snapshot) return [];
  return snapshot.objects.filter((object) => object.sceneUid === snapshot.activeSceneUid);
}

function moleculeStructureCacheKey(objectUid, structureKey = snapshot?.structureKey) {
  return `${structureKey || ""}|${objectUid || ""}`;
}

function cachedMoleculeStructure(objectUid) {
  if (!objectUid || !snapshot) return null;
  return moleculeStructureCache.get(moleculeStructureCacheKey(objectUid)) || null;
}

function requestedMoleculeObjectUids() {
  const objects = activeMoleculeObjects();
  const activeUids = new Set(objects.map((object) => object.uid));
  const visibleObjectUids = [];
  if (sidePanelIsExpanded("MolStruct")) visibleObjectUids.push(molstructObjectUid);
  if (sidePanelIsExpanded("Selection")) visibleObjectUids.push(selectionObjectUid);
  const requested = [...new Set(visibleObjectUids)].filter((uid) => activeUids.has(uid));
  if (requested.length === 0 && visibleObjectUids.length > 0) {
    const fallback = activeUids.has(selectedObjectUid) ? selectedObjectUid : objects[0]?.uid;
    if (fallback) requested.push(fallback);
  }
  return requested;
}

function moleculeStructureIsCurrent(objectUid = requestedMoleculeObjectUids()[0] ?? null) {
  return !objectUid || Boolean(cachedMoleculeStructure(objectUid));
}

function syncEmptyMoleculeDetails() {
  if (!snapshot) return;
  if (moleculeDetails.structureKey !== snapshot.structureKey) {
    moleculeStructureCache.clear();
    pendingMoleculeObjectUids.clear();
    moleculeDetails = {
      ...moleculeDetails,
      structureKey: snapshot.structureKey,
      structureObjectUid: null,
      structure: [],
    };
  }
  if (activeMoleculeObjects().length === 0) {
    moleculeDetails = {
      structureKey: snapshot.structureKey,
      structureObjectUid: null,
      sequenceKey: snapshot.sequenceKey,
      structure: [],
      sequence: [],
    };
  }
}

function moleculeSequenceIsCurrent() {
  return Boolean(snapshot && moleculeDetails.sequenceKey === snapshot.sequenceKey);
}

function sequencePanelIsActive() {
  return Boolean(
    document.querySelector('[data-bottom-panel="sequence"]')?.classList.contains("active"),
  );
}

function moleculeDetailsAreCurrent() {
  return requestedMoleculeObjectUids().every((uid) => moleculeStructureIsCurrent(uid)) &&
    (!sequencePanelIsActive() || moleculeSequenceIsCurrent());
}

function scheduleMoleculeDetailsRefresh(delay = 0, objectUid = null) {
  if (!invoke || !snapshot) return;
  syncEmptyMoleculeDetails();
  if (objectUid && !cachedMoleculeStructure(objectUid)) {
    pendingMoleculeObjectUids.add(objectUid);
  }
  for (const uid of requestedMoleculeObjectUids()) {
    if (!cachedMoleculeStructure(uid)) pendingMoleculeObjectUids.add(uid);
  }
  if (moleculeDetailsAreCurrent() || moleculeDetailsRequest) return;
  clearTimeout(moleculeDetailsTimer);
  moleculeDetailsTimer = setTimeout(() => {
    moleculeDetailsTimer = null;
    refreshMoleculeDetails();
  }, delay);
}

async function refreshMoleculeDetails() {
  if (!invoke || !snapshot || moleculeDetailsRequest) return;
  syncEmptyMoleculeDetails();
  const activeUids = new Set(activeMoleculeObjects().map((object) => object.uid));
  const requestedUids = new Set(requestedMoleculeObjectUids());
  for (const uid of requestedUids) {
    if (!cachedMoleculeStructure(uid)) pendingMoleculeObjectUids.add(uid);
  }
  for (const uid of pendingMoleculeObjectUids) {
    if (!activeUids.has(uid) || !requestedUids.has(uid) || cachedMoleculeStructure(uid)) {
      pendingMoleculeObjectUids.delete(uid);
    }
  }
  const objectUid = pendingMoleculeObjectUids.values().next().value ??
    requestedUids.values().next().value ?? null;
  const knownStructureObjectUid = cachedMoleculeStructure(objectUid) ? objectUid : null;
  const includeSequence = sequencePanelIsActive() && !moleculeSequenceIsCurrent();
  if (!objectUid && !includeSequence) return;
  if (objectUid) pendingMoleculeObjectUids.delete(objectUid);
  const expectedStructureKey = snapshot.structureKey;
  const expectedSequenceKey = snapshot.sequenceKey;
  moleculeDetailsRequest = invoke("get_molecule_details", {
    structureKey: moleculeDetails.structureKey,
    structureObjectUid: knownStructureObjectUid,
    sequenceKey: moleculeDetails.sequenceKey,
    includeSequence,
    objectUid,
  });
  try {
    const details = await moleculeDetailsRequest;
    const structureMatches =
      snapshot?.structureKey === details.structureKey &&
      details.structureKey === expectedStructureKey;
    const sequenceMatches =
      snapshot?.sequenceKey === details.sequenceKey &&
      details.sequenceKey === expectedSequenceKey;
    if (structureMatches) {
      if (details.structure) {
        for (const object of details.structure) {
          moleculeStructureCache.set(
            moleculeStructureCacheKey(object.objectUid, details.structureKey),
            object,
          );
        }
      }
      moleculeDetails = {
        structureKey: details.structureKey,
        structureObjectUid: details.structureObjectUid ?? moleculeDetails.structureObjectUid,
        sequenceKey:
          includeSequence && details.sequence && sequenceMatches
            ? details.sequenceKey
            : moleculeDetails.sequenceKey,
        structure: details.structure || moleculeDetails.structure,
        sequence:
          includeSequence && details.sequence && sequenceMatches
            ? details.sequence
            : moleculeDetails.sequence,
      };
      renderMolStruct();
      renderSelectionPanel();
      if (includeSequence && sequenceMatches) renderSequence();
    }
  } catch (error) {
    showError(error);
  } finally {
    moleculeDetailsRequest = null;
    if (!moleculeDetailsAreCurrent()) scheduleMoleculeDetailsRefresh();
  }
}

function visibleMolstructRows() {
  return [...molstructTree.querySelectorAll("[data-molstruct-key]")].filter(
    (row) => row.getClientRects().length > 0,
  );
}

function updateMolstructSelectionUi() {
  for (const row of molstructTree.querySelectorAll("[data-molstruct-key]")) {
    row.classList.toggle("selected", molstructSelectedKeys.has(row.dataset.molstructKey));
  }
  const enabled = visibleMolstructRows().some((row) =>
    molstructSelectedKeys.has(row.dataset.molstructKey),
  );
  molstructSelect.disabled = !enabled;
  molstructCenter.disabled = !enabled;
  molstructZoom.disabled = !enabled;
  molstructProperties.disabled = !molstructObjectUid;
}

function selectMolstructRow(event, row) {
  const key = row.dataset.molstructKey;
  const rows = visibleMolstructRows();
  const rangeStart = rows.findIndex(
    (item) => item.dataset.molstructKey === molstructSelectionAnchor,
  );
  const rangeEnd = rows.indexOf(row);
  if (event.shiftKey && rangeStart >= 0 && rangeEnd >= 0) {
    if (!(event.metaKey || event.ctrlKey)) molstructSelectedKeys.clear();
    const [start, end] = [rangeStart, rangeEnd].sort((lhs, rhs) => lhs - rhs);
    for (const item of rows.slice(start, end + 1)) {
      molstructSelectedKeys.add(item.dataset.molstructKey);
    }
  } else if (event.metaKey || event.ctrlKey) {
    if (molstructSelectedKeys.has(key)) molstructSelectedKeys.delete(key);
    else molstructSelectedKeys.add(key);
  } else {
    molstructSelectedKeys.clear();
    molstructSelectedKeys.add(key);
  }
  molstructSelectionAnchor = key;
  updateMolstructSelectionUi();
}

function molstructSelectorForRow(row) {
  switch (row.dataset.molstructType) {
    case "chain":
      return `c;'${row.dataset.chain}'`;
    case "residue":
      return `'${row.dataset.chain}'.${row.dataset.residueIndex}.*`;
    case "atom":
      return `aid ${row.dataset.atomId}`;
    default:
      return "";
  }
}

function selectedMolstructExpression() {
  const rows = visibleMolstructRows();
  const selectors = [];
  for (let index = 0; index < rows.length; index += 1) {
    const row = rows[index];
    if (!molstructSelectedKeys.has(row.dataset.molstructKey)) continue;
    if (row.dataset.molstructType === "residue") {
      let end = index;
      while (
        end + 1 < rows.length &&
        molstructSelectedKeys.has(rows[end + 1].dataset.molstructKey) &&
        rows[end + 1].dataset.molstructType === "residue" &&
        rows[end + 1].dataset.chain === row.dataset.chain
      ) {
        end += 1;
      }
      const endRow = rows[end];
      selectors.push(
        end === index
          ? molstructSelectorForRow(row)
          : `'${row.dataset.chain}'.${row.dataset.residueIndex}:${endRow.dataset.residueIndex}.*`,
      );
      index = end;
    } else {
      selectors.push(molstructSelectorForRow(row));
    }
  }
  return selectors.filter(Boolean).join("|");
}

async function runMoleculeSelectionAction(objectUid, selection, mode = "select") {
  setBusy(true);
  const sequence = ++requestSequence;
  try {
    const update = await invoke("molecule_selection_action", {
      objectUid,
      selection,
      mode,
      ...viewportDimensions(),
    });
    snapshot = update.snapshot;
    syncEmptyMoleculeDetails();
    applyViewport(update.viewport, sequence);
    renderShell();
    rememberSelection(selection);
    scheduleMoleculeDetailsRefresh();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

async function runMolstructAction(mode, selectionOverride = null) {
  const objectUid = Number(molstructObject.value) || null;
  const selection = selectionOverride ?? selectedMolstructExpression();
  if (!objectUid || !selection) {
    showError("Select a molecular structure row first.");
    return false;
  }
  return runMoleculeSelectionAction(objectUid, selection, mode);
}

function configureMolstructRow(row, properties) {
  row.dataset.molstructKey = properties.key;
  row.dataset.molstructType = properties.type;
  if (properties.chain !== undefined) row.dataset.chain = properties.chain;
  if (properties.residueIndex !== undefined) {
    row.dataset.residueIndex = properties.residueIndex;
  }
  if (properties.atomId !== undefined) row.dataset.atomId = properties.atomId;
  row.classList.add("molstruct-row");
  row.classList.toggle("selected", molstructSelectedKeys.has(properties.key));
  row.addEventListener("click", (event) => {
    const disclosureClick =
      row.tagName === "SUMMARY" && event.clientX - row.getBoundingClientRect().left <= 18;
    if (row.tagName === "SUMMARY" && !disclosureClick) event.preventDefault();
    selectMolstructRow(event, row);
  });
  row.addEventListener("dblclick", (event) => {
    event.preventDefault();
    event.stopPropagation();
    molstructSelectedKeys.clear();
    molstructSelectedKeys.add(properties.key);
    molstructSelectionAnchor = properties.key;
    updateMolstructSelectionUi();
    void runMolstructAction("center", molstructSelectorForRow(row));
  });
}

function renderMolStruct() {
  const objects = activeMoleculeObjects();
  const previousObjectUid = molstructObjectUid;
  if (!objects.some((object) => object.uid === molstructObjectUid)) {
    molstructObjectUid = objects.some((object) => object.uid === selectedObjectUid)
      ? selectedObjectUid
      : objects[0]?.uid ?? null;
  }
  if (previousObjectUid !== null && previousObjectUid !== molstructObjectUid) {
    molstructSelectedKeys.clear();
    molstructOpenKeys.clear();
    molstructSelectionAnchor = null;
  }
  replaceOptions(
    molstructObject,
    objects,
    molstructObjectUid,
  );
  const structureCurrent = moleculeStructureIsCurrent(molstructObjectUid);
  const renderKey = structureCurrent
    ? `${moleculeDetails.structureKey}|${molstructObjectUid ?? ""}`
    : `loading|${snapshot?.structureKey ?? ""}|${molstructObjectUid ?? ""}`;
  if (renderedMolstructKey === renderKey) {
    updateMolstructSelectionUi();
    return;
  }
  renderedMolstructKey = renderKey;
  molstructTree.replaceChildren();
  if (!structureCurrent) {
    const loading = document.createElement("p");
    loading.textContent = "Loading molecular hierarchy…";
    molstructTree.append(loading);
    scheduleMoleculeDetailsRefresh(0, molstructObjectUid);
    updateMolstructSelectionUi();
    return;
  }
  const object = cachedMoleculeStructure(molstructObjectUid);
  if (!object) {
    const empty = document.createElement("p");
    empty.textContent = "No molecule selected.";
    molstructTree.append(empty);
    updateMolstructSelectionUi();
    return;
  }

  for (const chain of object.chains) {
    const chainNode = document.createElement("details");
    chainNode.className = "structure-node chain-node";
    const chainLabel = chain.chainId || "_";
    const chainKey = `chain:${chain.chainId}`;
    chainNode.open = molstructOpenKeys.has(chainKey);
    const chainSummary = document.createElement("summary");
    chainSummary.textContent = `chain "${chainLabel}"`;
    configureMolstructRow(chainSummary, {
      key: chainKey,
      type: "chain",
      chain: chain.chainId,
    });
    chainNode.append(chainSummary);

    const loadResidues = () => {
      if (chainNode.dataset.loaded) return;
      chainNode.dataset.loaded = "true";
      for (const residue of chain.residues) {
        const residueIndex = `${residue.resId}${residue.insCode || ""}`;
        const residueKey = `residue:${chain.chainId}:${residueIndex}`;
        const residueNode = document.createElement("details");
        residueNode.className = "structure-node residue-node";
        residueNode.open = molstructOpenKeys.has(residueKey);
        const residueSummary = document.createElement("summary");
        residueSummary.textContent = `${residueIndex} ${residue.resName}`;
        configureMolstructRow(residueSummary, {
          key: residueKey,
          type: "residue",
          chain: chain.chainId,
          residueIndex,
        });
        residueNode.append(residueSummary);

        const loadAtoms = () => {
          if (residueNode.dataset.loaded) return;
          residueNode.dataset.loaded = "true";
          const atomList = document.createElement("div");
          atomList.className = "atom-list";
          for (const atom of residue.atoms) {
            const atomButton = document.createElement("button");
            atomButton.type = "button";
            atomButton.className = "atom-row";
            atomButton.textContent = `${atom.name} (${atom.element || "?"})`;
            atomButton.title = `atom index ${atom.atomIndex}`;
            configureMolstructRow(atomButton, {
              key: `atom:${atom.serial}`,
              type: "atom",
              atomId: atom.serial,
            });
            atomList.append(atomButton);
          }
          residueNode.append(atomList);
        };
        residueNode.addEventListener("toggle", () => {
          if (residueNode.open) {
            molstructOpenKeys.add(residueKey);
            loadAtoms();
          } else {
            molstructOpenKeys.delete(residueKey);
          }
          updateMolstructSelectionUi();
        });
        if (residueNode.open) loadAtoms();
        chainNode.append(residueNode);
      }
    };
    chainNode.addEventListener("toggle", () => {
      if (chainNode.open) {
        molstructOpenKeys.add(chainKey);
        loadResidues();
      } else {
        molstructOpenKeys.delete(chainKey);
      }
      updateMolstructSelectionUi();
    });
    if (chainNode.open) loadResidues();
    molstructTree.append(chainNode);
  }
  updateMolstructSelectionUi();
}

function replaceDatalist(datalist, values, cacheKey) {
  if (datalist.dataset.cacheKey === cacheKey) return;
  datalist.replaceChildren();
  for (const value of values) {
    const option = document.createElement("option");
    option.value = value;
    datalist.append(option);
  }
  datalist.dataset.cacheKey = cacheKey;
}

function selectionCandidateValues() {
  const cacheKey = `${moleculeDetails.structureKey}|${selectionObjectUid ?? ""}`;
  if (selectionCandidateCache.key === cacheKey && selectionCandidateCache.values) {
    return selectionCandidateCache.values;
  }
  const object = cachedMoleculeStructure(selectionObjectUid);
  if (!object) {
    scheduleMoleculeDetailsRefresh(0, selectionObjectUid);
    const values = {
      chains: ["A", "B", "C"],
      residueNames: [],
      atomNames: ["CA", "CB", "CG"],
      elements: ["C", "N", "O", "S"],
    };
    return values;
  }
  const residueNames = new Set();
  const atomNames = new Set();
  const elements = new Set();
  for (const chain of object.chains) {
    for (const residue of chain.residues) {
      residueNames.add(residue.resName);
      for (const atom of residue.atoms) {
        atomNames.add(atom.name);
        if (atom.element) elements.add(atom.element);
      }
    }
  }
  const values = {
    chains: object.chains.map((chain) => chain.chainId),
    residueNames: [...residueNames].sort(),
    atomNames: [...atomNames].sort(),
    elements: [...elements].sort(),
  };
  selectionCandidateCache = { key: cacheKey, values };
  return values;
}

function selectionTermDefault(command, candidates = selectionCandidateValues()) {
  switch (command) {
    case "c;":
      return candidates.chains[0] || "A";
    case "r;":
      return candidates.residueNames[0] || "ALA";
    case "n;":
      return candidates.atomNames[0] || "CA";
    case "e;":
      return candidates.elements[0] || "C";
    default:
      return "1:100";
  }
}

function selectionTermDatalist(command) {
  return {
    "c;": "selection-chain-candidates",
    "r;": "selection-resname-candidates",
    "n;": "selection-atom-candidates",
    "e;": "selection-element-candidates",
  }[command];
}

function newSelectionEditorItem(type) {
  if (type === "hier") {
    return { type, boolOp: "|", chain: "*", residue: "10:100", atom: "*" };
  }
  if (type === "term") {
    return {
      type,
      boolOp: "|",
      command: "c;",
      args: selectionTermDefault("c;"),
    };
  }
  return { type: "around", command: "around", args: "5" };
}

function addSelectionEditorItem(type) {
  if (type === "around" && selectionEditorItems.length === 0) return;
  const index =
    selectionEditorItems.length === 0
      ? 0
      : Math.min(selectionEditorSelectedIndex + 1, selectionEditorItems.length);
  selectionEditorItems.splice(index, 0, newSelectionEditorItem(type));
  selectionEditorSelectedIndex = index;
  renderSelectionEditor();
}

function deleteSelectionEditorItem() {
  if (selectionEditorSelectedIndex < 0) return;
  selectionEditorItems.splice(selectionEditorSelectedIndex, 1);
  if (selectionEditorItems[0]?.type === "around") selectionEditorItems.shift();
  selectionEditorSelectedIndex = selectionEditorItems.length
    ? Math.min(selectionEditorSelectedIndex, selectionEditorItems.length - 1)
    : -1;
  renderSelectionEditor();
}

function composeSelectionEditorExpression() {
  let selection = "";
  for (const item of selectionEditorItems) {
    if (item.type === "around") {
      if (!selection) continue;
      if (item.command === "around" || item.command === "expand") {
        selection = `(${selection}) ${item.command} ${item.args}`;
      } else if (item.command === "around byres") {
        selection = `byres ((${selection}) around ${item.args})`;
      } else if (item.command === "extend byres") {
        selection = `byres ((${selection}) extend ${item.args})`;
      }
      continue;
    }
    const term =
      item.type === "hier"
        ? `${item.chain}.${item.residue}.${item.atom}`
        : `${item.command} ${item.args}`;
    selection = selection ? `${selection} ${item.boolOp || "|"} ${term}` : term;
  }
  return selection;
}

function updateSelectionEditorUi() {
  for (const [index, row] of [...selectionEditorList.children].entries()) {
    row.classList.toggle("selected", index === selectionEditorSelectedIndex);
  }
  const selected = selectionEditorSelectedIndex >= 0;
  selectionEditorApply.disabled = !selectionObjectUid || !selected;
  selectionEditorDelete.disabled = !selected;
  selectionEditorClear.disabled = !selected;
  const aroundOption = selectionEditorAddKind.querySelector('option[value="around"]');
  if (aroundOption) aroundOption.disabled = selectionEditorItems.length === 0;
}

function selectionBoolOpControl(item, index) {
  if (index === 0 || item.type === "around") {
    const spacer = document.createElement("span");
    spacer.className = "selection-boolop";
    return spacer;
  }
  const select = document.createElement("select");
  select.className = "selection-boolop";
  for (const [value, label] of [
    ["&", "&"],
    ["|", "|"],
  ]) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    select.append(option);
  }
  select.value = item.boolOp;
  select.addEventListener("change", () => {
    item.boolOp = select.value;
  });
  return select;
}

function selectionInputControl(value, className, onInput, list = null) {
  const input = document.createElement("input");
  input.className = className;
  input.value = value;
  input.spellcheck = false;
  if (list) input.setAttribute("list", list);
  input.addEventListener("input", () => onInput(input.value));
  return input;
}

function renderSelectionEditor() {
  selectionEditorList.replaceChildren();
  if (selectionEditorItems.length === 0) {
    const empty = document.createElement("div");
    empty.className = "selection-editor-empty";
    empty.textContent = "No selection items.";
    selectionEditorList.append(empty);
    updateSelectionEditorUi();
    return;
  }
  for (const [index, item] of selectionEditorItems.entries()) {
    const row = document.createElement("div");
    row.className = `selection-editor-row${index === selectionEditorSelectedIndex ? " selected" : ""}`;
    row.append(selectionBoolOpControl(item, index));
    if (item.type === "hier") {
      row.append(
        selectionInputControl(
          item.chain,
          "selection-hier-chain",
          (value) => {
            item.chain = value;
          },
          "selection-chain-candidates",
        ),
      );
      const dot1 = document.createElement("span");
      dot1.textContent = ".";
      row.append(dot1);
      row.append(
        selectionInputControl(item.residue, "selection-hier-residue", (value) => {
          item.residue = value;
        }),
      );
      const dot2 = document.createElement("span");
      dot2.textContent = ".";
      row.append(dot2);
      row.append(
        selectionInputControl(
          item.atom,
          "selection-hier-atom",
          (value) => {
            item.atom = value;
          },
          "selection-atom-candidates",
        ),
      );
    } else {
      const command = document.createElement("select");
      command.className = "selection-item-command";
      const commands =
        item.type === "term"
          ? [
              ["c;", "chain"],
              ["i;", "resid"],
              ["r;", "resname"],
              ["n;", "atom"],
              ["e;", "element"],
              ["aid", "aid"],
            ]
          : [
              ["around", "around"],
              ["expand", "expand"],
              ["around byres", "around byres"],
              ["extend byres", "extend byres"],
            ];
      for (const [value, label] of commands) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = label;
        command.append(option);
      }
      command.value = item.command;
      const args = selectionInputControl(
        item.args,
        "selection-item-args",
        (value) => {
          item.args = value;
        },
        item.type === "term" ? selectionTermDatalist(item.command) : null,
      );
      command.addEventListener("change", () => {
        item.command = command.value;
        item.args =
          item.type === "term" ? selectionTermDefault(item.command) : item.args || "5";
        renderSelectionEditor();
      });
      row.append(command, args);
    }
    row.addEventListener("click", () => {
      selectionEditorSelectedIndex = index;
      updateSelectionEditorUi();
    });
    selectionEditorList.append(row);
  }
  updateSelectionEditorUi();
}

function renderSelectionPanel() {
  const objects = activeMoleculeObjects();
  if (!objects.some((object) => object.uid === selectionObjectUid)) {
    selectionObjectUid = objects.some((object) => object.uid === selectedObjectUid)
      ? selectedObjectUid
      : objects[0]?.uid ?? null;
  }
  replaceOptions(
    selectionObject,
    objects,
    selectionObjectUid,
  );
  document.querySelectorAll("[data-selection-tab]").forEach((tab) => {
    const active = tab.dataset.selectionTab === selectionActiveTab;
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-selected", String(active));
  });
  selectionCommandPanel.hidden = selectionActiveTab !== "command";
  selectionEditorPanel.hidden = selectionActiveTab !== "editor";
  renderSelectionHistory();
  const candidates = selectionCandidateValues();
  const candidateKey = `${moleculeDetails.structureKey}|${selectionObjectUid ?? ""}|${
    moleculeStructureIsCurrent(selectionObjectUid) ? "ready" : "loading"
  }`;
  replaceDatalist(selectionChainCandidates, candidates.chains, `${candidateKey}|chain`);
  replaceDatalist(
    selectionResnameCandidates,
    candidates.residueNames,
    `${candidateKey}|resname`,
  );
  replaceDatalist(selectionAtomCandidates, candidates.atomNames, `${candidateKey}|atom`);
  replaceDatalist(selectionElementCandidates, candidates.elements, `${candidateKey}|element`);
  if (selectionActiveTab === "editor" && selectionEditorList.childElementCount === 0) {
    renderSelectionEditor();
  } else {
    updateSelectionEditorUi();
  }
}

async function applySelectionCommand() {
  const objectUid = Number(selectionObject.value) || null;
  if (!objectUid) {
    showError("Select a molecule first.");
    return false;
  }
  return runMoleculeSelectionAction(objectUid, selectionInput.value, "select");
}

async function applySelectionEditor() {
  const objectUid = Number(selectionObject.value) || null;
  const selection = composeSelectionEditorExpression();
  if (!objectUid || !selection) {
    showError("Add and select an editor item first.");
    return false;
  }
  return runMoleculeSelectionAction(objectUid, selection, "select");
}

function coloringTargets() {
  const targets = [];
  for (const object of activeObjects()) {
    targets.push({
      key: `object:${object.uid}`,
      kind: "object",
      objectUid: object.uid,
      rendererUid: null,
      name: object.name,
      state: object,
    });
    for (const renderer of object.renderers.filter((item) =>
      item.supportsColoring || item.supportsElectrostaticColoring || item.supportsMultiGradient
    )) {
      targets.push({
        key: `renderer:${object.uid}:${renderer.uid}`,
        kind: "renderer",
        objectUid: object.uid,
        rendererUid: renderer.uid,
        name: `  ↳ ${renderer.name}`,
        state: renderer,
      });
    }
  }
  for (const map of snapshot?.densityMaps || []) {
    for (const renderer of map.renderers.filter((item) =>
      item.supportsSolidColoring || item.supportsMultiGradient
    )) {
      targets.push({
        key: `densityRenderer:${map.uid}:${renderer.uid}`,
        kind: "densityRenderer",
        objectUid: map.uid,
        rendererUid: renderer.uid,
        name: `${map.name} → ${renderer.name}`,
        state: renderer,
      });
    }
  }
  for (const surface of snapshot?.surfaces || []) {
    for (const renderer of surface.renderers.filter((item) =>
      item.supportsSolidColoring || item.supportsElectrostaticColoring || item.supportsMultiGradient
    )) {
      targets.push({
        key: `surfaceRenderer:${surface.uid}:${renderer.uid}`,
        kind: "surfaceRenderer",
        objectUid: surface.uid,
        rendererUid: renderer.uid,
        name: `${surface.name} → ${renderer.name}`,
        state: renderer,
      });
    }
  }
  return targets;
}

function coloringTypeOptions(target) {
  const options = [["", "Reset to default"]];
  if (target?.state.supportsColoring || target?.kind === "object") {
    options.push(
      ["PaintColoring", "Paint coloring"],
      ["SolidColoring", "Solid color"],
      ["CPKColoring", "CPK coloring"],
      ["BfacColoring", "B-factor / Occupancy"],
      ["RainbowColoring", "Rainbow"],
    );
  } else if (target?.state.supportsSolidColoring) {
    options.push(["SolidColoring", "Solid color"]);
  }
  if (target?.state.supportsElectrostaticColoring) {
    options.push(["ElectrostaticColoring", "Electrostatic potential"]);
  }
  if (target?.state.supportsMultiGradient) {
    options.push(["MultiGradientColoring", "Multi-gradient"]);
  }
  if (target?.state.coloringType === "ScriptColoring") {
    options.push(["ScriptColoring", "Script coloring"]);
  }
  return options;
}

function selectedColoringTarget() {
  return coloringTargets().find((target) => target.key === coloringTargetKey) || null;
}

function coloringMapOptions(electrostaticOnly = false) {
  return (snapshot?.densityMaps || [])
    .filter((map) => !electrostaticOnly || map.className === "ElePotMap")
    .map((map) => [map.name, map.name]);
}

function coloringMapByName(name) {
  return (snapshot?.densityMaps || []).find((map) => map.name === name) || null;
}

function rainbowGradientForMap(name) {
  const map = coloringMapByName(name);
  const minimum = Number.isFinite(map?.minDensity) ? map.minDensity : -1;
  const maximum = Number.isFinite(map?.maxDensity) ? map.maxDensity : 1;
  const delta = (maximum - minimum) / 5;
  return ["#ff0000", "#ffff00", "#00ff00", "#00ffff", "#0000ff", "#ff00ff"]
    .map((color, index) => ({ value: minimum + delta * index, color }));
}

function defaultColoringState(type, target) {
  switch (type) {
    case "PaintColoring":
      return {
        attrs: {},
        paintEntries: [
          { selection: "sheet", color: "#4682b4" },
          { selection: "helix", color: "#f0e68c" },
          { selection: "nucleic", color: "#ffff00" },
          { selection: "*", color: "#fffaf0" },
        ],
        defaultColor: null,
      };
    case "SolidColoring":
      return { attrs: {}, paintEntries: [], defaultColor: target.state.defaultColor || "#ffffff" };
    case "CPKColoring":
      return {
        attrs: {
          col_H: "#ffffff",
          col_C: "$molcol",
          col_N: "#0000ff",
          col_O: "#ff0000",
          col_P: "#ffff00",
          col_S: "#00ff00",
          col_X: "#808080",
        },
        paintEntries: [],
        defaultColor: null,
      };
    case "RainbowColoring":
      return {
        attrs: {
          mode: "mol",
          incr_mode: "resid",
          start_hue: "240",
          end_hue: "0",
          sat: "1",
          bri: "1",
        },
        paintEntries: [],
        defaultColor: null,
      };
    case "BfacColoring":
      return {
        attrs: {
          mode: "bfac",
          auto: "rend",
          lowcol: "#0000ff",
          highcol: "#ff0000",
          lowpar: "-2",
          highpar: "2",
        },
        paintEntries: [],
        defaultColor: null,
      };
    case "ScriptColoring":
      return { attrs: { script: "" }, paintEntries: [], defaultColor: null };
    case "ElectrostaticColoring":
      return {
        attrs: {
          elepot: coloringMapOptions(true)[0]?.[0] || "",
          lowpar: "-10",
          midpar: "0",
          highpar: "10",
          lowcol: "#ff0000",
          midcol: "#ffffff",
          highcol: "#0000ff",
          ramp_above: "false",
          ramp_value: "1.4",
        },
        paintEntries: [],
        gradientEntries: [],
        defaultColor: null,
      };
    case "MultiGradientColoring": {
      const mapName = coloringMapOptions(false)[0]?.[0] || "";
      return {
        attrs: { color_mapname: mapName, ramp_above: "false", ramp_value: "1.4" },
        paintEntries: [],
        gradientEntries: rainbowGradientForMap(mapName),
        defaultColor: null,
      };
    }
    default:
      return { attrs: {}, paintEntries: [], gradientEntries: [], defaultColor: null };
  }
}

function coloringState(target) {
  return {
    type: target.state.coloringType || "",
    attrs: { ...(target.state.coloringAttrs || {}) },
    paintEntries: (target.state.paintEntries || []).map((entry) => ({ ...entry })),
    gradientEntries: (target.state.gradientEntries || []).map((entry) => ({ ...entry })),
    defaultColor: target.state.defaultColor || "#ffffff",
  };
}

async function commitColoring(target, type, state) {
  if (!invoke) {
    showError("Tauri IPC is unavailable. Run this frontend through the Tauri binary.");
    return false;
  }
  setBusy(true);
  const sequence = ++requestSequence;
  try {
    const result = await invoke("set_coloring", {
      update: {
        kind: target.kind,
        objectUid: target.objectUid,
        rendererUid: target.rendererUid,
        coloringType: type || null,
        attrs: state.attrs || {},
        paintEntries: state.paintEntries || [],
        gradientEntries: state.gradientEntries || [],
        defaultColor: state.defaultColor || null,
      },
      ...viewportDimensions(),
    });
    snapshot = result.snapshot;
    syncEmptyMoleculeDetails();
    applyViewport(result.viewport, sequence);
    renderShell();
    scheduleMoleculeDetailsRefresh();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

function coloringControlRow(labelText, control) {
  const row = document.createElement("label");
  row.className = "coloring-control-row";
  const label = document.createElement("span");
  label.textContent = labelText;
  row.append(label, control);
  coloringDeck.append(row);
  return control;
}

function coloringSelect(id, value, entries, onChange) {
  const select = document.createElement("select");
  select.id = id;
  for (const [optionValue, label] of entries) {
    const option = document.createElement("option");
    option.value = optionValue;
    option.textContent = label;
    select.append(option);
  }
  select.value = value;
  select.addEventListener("change", () => onChange(select.value));
  return select;
}

function coloringInput(id, value, type, onChange) {
  const input = document.createElement("input");
  input.id = id;
  input.type = type;
  input.value = String(value);
  input.addEventListener("change", () => onChange(input.value));
  return input;
}

function renderPaintColoring(target, state) {
  const list = document.createElement("div");
  list.id = "coloring-paint-list";
  list.className = "coloring-paint-list";
  if (coloringPaintSelectedIndex >= state.paintEntries.length) {
    coloringPaintSelectedIndex = state.paintEntries.length - 1;
  }
  state.paintEntries.forEach((entry, index) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "coloring-paint-row";
    row.classList.toggle("selected", index === coloringPaintSelectedIndex);
    row.dataset.paintIndex = String(index);
    const swatch = document.createElement("span");
    swatch.className = "coloring-swatch";
    swatch.style.background = entry.color;
    const selection = document.createElement("span");
    selection.textContent = entry.selection;
    const color = document.createElement("code");
    color.textContent = entry.color;
    row.append(swatch, selection, color);
    row.addEventListener("click", () => {
      coloringPaintSelectedIndex = index;
      renderColoring();
    });
    list.append(row);
  });
  coloringDeck.append(list);

  const selected = state.paintEntries[coloringPaintSelectedIndex] || { selection: "*", color: "#ffffff" };
  const editor = document.createElement("div");
  editor.className = "coloring-paint-editor";
  const selection = coloringInput("coloring-paint-selection", selected.selection, "text", () => {});
  selection.placeholder = "Selection";
  const color = coloringInput("coloring-paint-color", selected.color, "text", () => {});
  color.placeholder = "Color";
  editor.append(selection, color);
  coloringDeck.append(editor);

  const toolbar = document.createElement("div");
  toolbar.className = "coloring-paint-toolbar";
  const action = (label, title, handler, disabled = false) => {
    const control = button(label, "", handler);
    control.title = title;
    control.disabled = disabled;
    toolbar.append(control);
  };
  action("＋", "Add before selected", () => {
    const index = coloringPaintSelectedIndex < 0 ? state.paintEntries.length : coloringPaintSelectedIndex;
    state.paintEntries.splice(index, 0, { selection: selection.value || "*", color: color.value || "#ffffff" });
    coloringPaintSelectedIndex = index;
    commitColoring(target, "PaintColoring", state);
  });
  action("−", "Delete selected", () => {
    state.paintEntries.splice(coloringPaintSelectedIndex, 1);
    commitColoring(target, "PaintColoring", state);
  }, coloringPaintSelectedIndex < 0);
  action("×", "Delete all", () => {
    coloringPaintSelectedIndex = -1;
    state.paintEntries = [];
    commitColoring(target, "PaintColoring", state);
  }, state.paintEntries.length === 0);
  action("Change", "Change selected", () => {
    state.paintEntries[coloringPaintSelectedIndex] = {
      selection: selection.value || "*",
      color: color.value || "#ffffff",
    };
    commitColoring(target, "PaintColoring", state);
  }, coloringPaintSelectedIndex < 0);
  action("↑", "Move up", () => {
    const index = coloringPaintSelectedIndex;
    [state.paintEntries[index - 1], state.paintEntries[index]] = [state.paintEntries[index], state.paintEntries[index - 1]];
    coloringPaintSelectedIndex -= 1;
    commitColoring(target, "PaintColoring", state);
  }, coloringPaintSelectedIndex <= 0);
  action("↓", "Move down", () => {
    const index = coloringPaintSelectedIndex;
    [state.paintEntries[index], state.paintEntries[index + 1]] = [state.paintEntries[index + 1], state.paintEntries[index]];
    coloringPaintSelectedIndex += 1;
    commitColoring(target, "PaintColoring", state);
  }, coloringPaintSelectedIndex < 0 || coloringPaintSelectedIndex >= state.paintEntries.length - 1);
  coloringDeck.append(toolbar);
}

function renderElectrostaticColoring(target, state) {
  const mapOptions = coloringMapOptions(true);
  coloringControlRow(
    "Potential map",
    coloringSelect(
      "coloring-electrostatic-map",
      state.attrs.elepot || "",
      mapOptions.length ? mapOptions : [["", "(none)"]],
      (value) => {
        state.attrs.elepot = value;
        commitColoring(target, state.type, state);
      },
    ),
  );
  for (const [name, label, fallback, type] of [
    ["lowpar", "Low value", "-10", "number"],
    ["midpar", "Middle value", "0", "number"],
    ["highpar", "High value", "10", "number"],
    ["lowcol", "Low color", "#ff0000", "text"],
    ["midcol", "Middle color", "#ffffff", "text"],
    ["highcol", "High color", "#0000ff", "text"],
  ]) {
    coloringControlRow(
      label,
      coloringInput(`coloring-electrostatic-${name}`, state.attrs[name] || fallback, type, (value) => {
        state.attrs[name] = value;
        commitColoring(target, state.type, state);
      }),
    );
  }
  coloringControlRow(
    "Ramp above",
    coloringSelect(
      "coloring-electrostatic-ramp-above",
      state.attrs.ramp_above || "false",
      [["false", "No"], ["true", "Yes"]],
      (value) => {
        state.attrs.ramp_above = value;
        commitColoring(target, state.type, state);
      },
    ),
  );
  coloringControlRow(
    "Ramp distance",
    coloringInput(
      "coloring-electrostatic-ramp-value",
      state.attrs.ramp_value || "1.4",
      "number",
      (value) => {
        state.attrs.ramp_value = value;
        commitColoring(target, state.type, state);
      },
    ),
  );
}

function gradientPreset(name, mapName) {
  const map = coloringMapByName(mapName);
  const minimum = Number.isFinite(map?.minDensity) ? map.minDensity : -1;
  const maximum = Number.isFinite(map?.maxDensity) ? map.maxDensity : 1;
  if (name === "resmap1") {
    const delta = (maximum - minimum) / 4;
    return ["#0f77cf", "#87e3e7", "#ffffff", "#cf8faf", "#9e205e"]
      .map((color, index) => ({ value: minimum + delta * index, color }));
  }
  if (name === "heatmap1") {
    return [
      { value: minimum, color: "#ff0000" },
      { value: minimum + (maximum - minimum) * 0.6666, color: "#ffff00" },
      { value: maximum, color: "#ffffff" },
    ];
  }
  return rainbowGradientForMap(mapName);
}

function renderMultiGradientColoring(target, state) {
  const mapOptions = coloringMapOptions(false);
  coloringControlRow(
    "Color map",
    coloringSelect(
      "coloring-gradient-map",
      state.attrs.color_mapname || "",
      mapOptions.length ? mapOptions : [["", "(none)"]],
      (value) => {
        state.attrs.color_mapname = value;
        commitColoring(target, state.type, state);
      },
    ),
  );
  if (coloringGradientSelectedIndex >= state.gradientEntries.length) {
    coloringGradientSelectedIndex = state.gradientEntries.length - 1;
  }
  const list = document.createElement("div");
  list.id = "coloring-gradient-list";
  list.className = "coloring-paint-list";
  state.gradientEntries.forEach((entry, index) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "coloring-paint-row";
    row.classList.toggle("selected", index === coloringGradientSelectedIndex);
    row.dataset.gradientIndex = String(index);
    const swatch = document.createElement("span");
    swatch.className = "coloring-swatch";
    swatch.style.background = entry.color;
    const value = document.createElement("span");
    value.textContent = Number(entry.value).toPrecision(6);
    const color = document.createElement("code");
    color.textContent = entry.color;
    row.append(swatch, value, color);
    row.addEventListener("click", () => {
      coloringGradientSelectedIndex = index;
      renderColoring();
    });
    list.append(row);
  });
  coloringDeck.append(list);

  const selected = state.gradientEntries[coloringGradientSelectedIndex] || { value: 0, color: "#ffffff" };
  const editor = document.createElement("div");
  editor.className = "coloring-paint-editor";
  const value = coloringInput("coloring-gradient-value", selected.value, "number", () => {});
  value.step = "any";
  const color = coloringInput("coloring-gradient-color", selected.color, "text", () => {});
  editor.append(value, color);
  coloringDeck.append(editor);

  const toolbar = document.createElement("div");
  toolbar.className = "coloring-paint-toolbar";
  const action = (label, title, handler, disabled = false) => {
    const control = button(label, "", handler);
    control.title = title;
    control.disabled = disabled;
    toolbar.append(control);
  };
  const sortedEntries = () => state.gradientEntries.sort((left, right) => left.value - right.value);
  action("＋", "Add gradient node", () => {
    state.gradientEntries.push({ value: Number(value.value), color: color.value || "#ffffff" });
    sortedEntries();
    coloringGradientSelectedIndex = state.gradientEntries.findIndex((entry) => entry.value === Number(value.value));
    commitColoring(target, state.type, state);
  });
  action("−", "Delete gradient node", () => {
    state.gradientEntries.splice(coloringGradientSelectedIndex, 1);
    commitColoring(target, state.type, state);
  }, coloringGradientSelectedIndex < 0);
  action("×", "Delete all gradient nodes", () => {
    state.gradientEntries = [];
    coloringGradientSelectedIndex = -1;
    commitColoring(target, state.type, state);
  }, state.gradientEntries.length === 0);
  action("Change", "Change gradient node", () => {
    state.gradientEntries[coloringGradientSelectedIndex] = {
      value: Number(value.value),
      color: color.value || "#ffffff",
    };
    sortedEntries();
    coloringGradientSelectedIndex = state.gradientEntries.findIndex((entry) => entry.value === Number(value.value));
    commitColoring(target, state.type, state);
  }, coloringGradientSelectedIndex < 0);
  coloringDeck.append(toolbar);

  const preset = coloringSelect(
    "coloring-gradient-preset",
    "rainbow1",
    [["rainbow1", "Rainbow"], ["resmap1", "ResMap"], ["heatmap1", "Heat map"]],
    () => {},
  );
  const applyPreset = button("Apply", "", () => {
    state.gradientEntries = gradientPreset(preset.value, state.attrs.color_mapname || "");
    coloringGradientSelectedIndex = -1;
    commitColoring(target, state.type, state);
  });
  const presetRow = document.createElement("div");
  presetRow.className = "coloring-paint-editor";
  presetRow.append(preset, applyPreset);
  coloringDeck.append(presetRow);
}

function renderColoringDeck(target) {
  coloringDeck.replaceChildren();
  const state = coloringState(target);
  const updateAttr = (name, value) => {
    state.attrs[name] = value;
    commitColoring(target, state.type, state);
  };
  switch (state.type) {
    case "SolidColoring":
      coloringControlRow(
        "Color",
        coloringInput("coloring-solid-color", state.defaultColor, "text", (value) => {
          state.defaultColor = value;
          commitColoring(target, state.type, state);
        }),
      );
      break;
    case "CPKColoring":
      for (const element of ["H", "C", "N", "O", "P", "S", "X"]) {
        coloringControlRow(
          element === "X" ? "Other" : element,
          coloringInput(
            `coloring-cpk-${element.toLowerCase()}`,
            state.attrs[`col_${element}`] || defaultColoringState(state.type, target).attrs[`col_${element}`],
            "text",
            (value) => updateAttr(`col_${element}`, value),
          ),
        );
      }
      break;
    case "RainbowColoring":
      coloringControlRow(
        "Mode",
        coloringSelect("coloring-rainbow-mode", state.attrs.mode || "mol", [["mol", "Molecule"], ["chain", "Chain"]], (value) => updateAttr("mode", value)),
      );
      coloringControlRow(
        "Increment",
        coloringSelect("coloring-rainbow-increment", state.attrs.incr_mode || "resid", [["chain", "Chain"], ["resid", "Residue"], ["protss", "Secondary structure"]], (value) => updateAttr("incr_mode", value)),
      );
      for (const [name, label, fallback] of [["start_hue", "Start hue", "240"], ["end_hue", "End hue", "0"]]) {
        coloringControlRow(label, coloringInput(`coloring-rainbow-${name.replace("_", "-")}`, state.attrs[name] || fallback, "number", (value) => updateAttr(name, value)));
      }
      for (const [name, label] of [["sat", "Saturation %"], ["bri", "Brightness %"]]) {
        const percent = Math.round(Number(state.attrs[name] ?? 1) * 100);
        coloringControlRow(label, coloringInput(`coloring-rainbow-${name}`, percent, "number", (value) => updateAttr(name, String(Number(value) / 100))));
      }
      break;
    case "BfacColoring":
      coloringControlRow(
        "Source",
        coloringSelect("coloring-bfac-mode", state.attrs.mode || "bfac", [["bfac", "B-factor"], ["occ", "Occupancy"], ["center", "Distance from center"]], (value) => updateAttr("mode", value)),
      );
      coloringControlRow(
        "Auto range",
        coloringSelect("coloring-bfac-auto", state.attrs.auto || "rend", [["none", "None"], ["mol", "Molecule"], ["rend", "Renderer"]], (value) => updateAttr("auto", value)),
      );
      for (const [name, label, fallback, type] of [
        ["lowcol", "Low color", "#0000ff", "text"],
        ["highcol", "High color", "#ff0000", "text"],
        ["lowpar", "Low value", "-2", "number"],
        ["highpar", "High value", "2", "number"],
      ]) {
        coloringControlRow(label, coloringInput(`coloring-bfac-${name}`, state.attrs[name] || fallback, type, (value) => updateAttr(name, value)));
      }
      break;
    case "ScriptColoring": {
      const editor = document.createElement("textarea");
      editor.id = "coloring-script-text";
      editor.className = "coloring-script-text";
      editor.rows = 10;
      editor.value = state.attrs.script || "";
      const update = button("Update", "", () => {
        state.attrs.script = editor.value;
        commitColoring(target, state.type, state);
      });
      update.id = "coloring-script-update";
      update.disabled = true;
      editor.addEventListener("input", () => {
        update.disabled = false;
      });
      coloringControlRow("Script coloring", editor);
      coloringDeck.append(update);
      break;
    }
    case "PaintColoring":
      renderPaintColoring(target, state);
      break;
    case "ElectrostaticColoring":
      renderElectrostaticColoring(target, state);
      break;
    case "MultiGradientColoring":
      renderMultiGradientColoring(target, state);
      break;
    default: {
      const note = document.createElement("p");
      note.className = "coloring-note";
      note.textContent = "The target inherits its default coloring.";
      coloringDeck.append(note);
      break;
    }
  }
}

function renderColoring() {
  const targets = coloringTargets();
  if (!targets.some((target) => target.key === coloringTargetKey)) {
    const rendererKey = selectedRendererUid === null ? null : `renderer:${selectedObjectUid}:${selectedRendererUid}`;
    coloringTargetKey = targets.some((target) => target.key === rendererKey)
      ? rendererKey
      : targets.find((target) => target.objectUid === selectedObjectUid)?.key || targets[0]?.key || null;
    coloringPaintSelectedIndex = -1;
    coloringGradientSelectedIndex = -1;
  }
  coloringTarget.replaceChildren();
  for (const target of targets) {
    const option = document.createElement("option");
    option.value = target.key;
    option.textContent = target.name;
    coloringTarget.append(option);
  }
  coloringTarget.disabled = targets.length === 0;
  if (coloringTargetKey) coloringTarget.value = coloringTargetKey;
  const target = selectedColoringTarget();
  coloringType.replaceChildren();
  for (const [value, label] of coloringTypeOptions(target)) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    coloringType.append(option);
  }
  coloringType.disabled = !target;
  coloringType.value = target?.state.coloringType || "";
  if (target) renderColoringDeck(target);
  else coloringDeck.replaceChildren();
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
  const changeSymmetry = button("Change symmetry…", "", () =>
    showReferenceDialog("Change symmetry"),
  );
  changeSymmetry.disabled = activeObjects().length === 0;
  symmetryContent.append(changeSymmetry);
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
  if (!document.querySelector('[data-bottom-panel="sequence"]')?.classList.contains("active")) {
    return;
  }
  if (!moleculeSequenceIsCurrent()) {
    renderedSequenceKey = "";
    sequenceView.replaceChildren();
    const loading = document.createElement("p");
    loading.textContent = "Loading sequence…";
    sequenceView.append(loading);
    return;
  }
  if (renderedSequenceKey === moleculeDetails.sequenceKey) return;
  renderedSequenceKey = moleculeDetails.sequenceKey;
  sequenceView.replaceChildren();
  if (moleculeDetails.sequence.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "No molecule loaded.";
    sequenceView.append(empty);
    return;
  }
  for (const chain of moleculeDetails.sequence) {
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
        const size = viewportDimensions();
        const sequence = ++requestSequence;
        const frame = await invoke("tick_animation_frame", { dt: 0.1, ...size });
        snapshot.animationElapsed = frame.animationElapsed;
        snapshot.animationDuration = frame.animationDuration;
        snapshot.animationPlaying = frame.animationPlaying;
        snapshot.status = frame.status;
        renderAnimation();
        applyViewport(frame.viewport, sequence);
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

function selectedTrajectory() {
  return (snapshot?.trajectories || []).find(
    (trajectory) => trajectory.objectUid === selectedTrajectoryUid,
  );
}

async function seekTrajectory(frame) {
  const trajectory = selectedTrajectory();
  if (!trajectory || !invoke) return false;
  const last = Math.max(0, trajectory.frameCount - 1);
  const target = Math.max(0, Math.min(last, Math.round(Number(frame) || 0)));
  trajectoryFrames.set(trajectory.objectUid, target);
  const size = viewportDimensions();
  const sequence = ++requestSequence;
  setBusy(true);
  try {
    const result = await invoke("set_trajectory_frame_viewport", {
      objectUid: trajectory.objectUid,
      frame: target,
      ...size,
    });
    trajectory.currentFrame = result.frame;
    snapshot.status = result.status;
    renderTrajectory();
    applyViewport(result.viewport, sequence);
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

function syncTrajectoryTimer() {
  const trajectory = selectedTrajectory();
  if (trajectoryPlaying && trajectory?.frameCount > 1 && invoke) {
    if (trajectoryTimer) return;
    const interval = Math.max(25, 100 / (Number(trajectorySpeed.value) || 1));
    trajectoryTimer = setInterval(async () => {
      if (trajectoryTickBusy) return;
      const active = selectedTrajectory();
      if (!active) {
        trajectoryPlaying = false;
        renderTrajectory();
        return;
      }
      const last = active.frameCount - 1;
      let next = (trajectoryFrames.get(active.objectUid) || 0) + 1;
      if (next > last) {
        if (trajectoryLoop.checked) {
          next = 0;
        } else {
          trajectoryPlaying = false;
          renderTrajectory();
          return;
        }
      }
      trajectoryTickBusy = true;
      try {
        await seekTrajectory(next);
      } finally {
        trajectoryTickBusy = false;
      }
    }, interval);
  } else if (trajectoryTimer) {
    clearInterval(trajectoryTimer);
    trajectoryTimer = null;
  }
}

function renderTrajectory() {
  const trajectories = snapshot.trajectories || [];
  if (!trajectories.some((trajectory) => trajectory.objectUid === selectedTrajectoryUid)) {
    selectedTrajectoryUid = trajectories[0]?.objectUid ?? null;
  }
  trajectoryObject.replaceChildren();
  for (const trajectory of trajectories) {
    const option = document.createElement("option");
    option.value = String(trajectory.objectUid);
    option.textContent = `${trajectory.objectName} (${trajectory.frameCount} frames)`;
    trajectoryObject.append(option);
  }
  if (selectedTrajectoryUid !== null) {
    trajectoryObject.value = String(selectedTrajectoryUid);
  }

  const trajectory = selectedTrajectory();
  const enabled = Boolean(trajectory?.frameCount);
  const blocks = trajectory?.blocks || [];
  const last = enabled ? trajectory.frameCount - 1 : 0;
  const current = enabled
    ? Math.min(last, trajectory.currentFrame ?? trajectoryFrames.get(trajectory.objectUid) ?? 0)
    : 0;
  if (trajectory) trajectoryFrames.set(trajectory.objectUid, current);
  trajectoryObject.disabled = trajectories.length === 0;
  trajectorySlider.disabled = !enabled;
  trajectorySlider.max = String(last);
  trajectorySlider.value = String(current);
  trajectoryFrame.disabled = !enabled;
  trajectoryFrame.max = String(last);
  trajectoryFrame.value = String(current);
  trajectoryLastFrame.textContent = String(last);
  trajectoryEmpty.hidden = blocks.length > 0;
  trajectoryEmpty.textContent = trajectory
    ? "No trajectory blocks — add a trajectory file."
    : "No trajectory in this scene — use File > Open MD Trajectory…";

  if (!blocks.some((block) => block.uid === selectedTrajectoryBlockUid)) {
    selectedTrajectoryBlockUid = null;
  }
  trajectoryBlockStrip.replaceChildren();
  for (const [index, block] of blocks.entries()) {
    const item = document.createElement("button");
    item.type = "button";
    item.draggable = true;
    item.className = `trajectory-block trajectory-block-c${block.uid % 6}`;
    item.classList.toggle("selected", block.uid === selectedTrajectoryBlockUid);
    item.style.flex = `${Math.max(1, block.frameCount)} 1 0px`;
    item.textContent = `${block.name} · ${block.frameCount}`;
    item.title = `${block.source || block.name}\n${block.format} · frames ${block.startIndex}–${block.startIndex + block.frameCount - 1}`;
    item.addEventListener("click", () => {
      selectedTrajectoryBlockUid = block.uid;
      renderTrajectory();
    });
    item.addEventListener("dragstart", (event) => {
      draggedTrajectoryBlockUid = block.uid;
      item.classList.add("dragging");
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", String(block.uid));
    });
    item.addEventListener("dragover", (event) => {
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
    });
    item.addEventListener("drop", async (event) => {
      event.preventDefault();
      const active = selectedTrajectory();
      const from = active?.blocks.findIndex(
        (candidate) => candidate.uid === draggedTrajectoryBlockUid,
      );
      if (active && from >= 0 && from !== index) {
        await mutate("move_trajectory_block", {
          objectUid: active.objectUid,
          from,
          to: index,
        });
      }
    });
    item.addEventListener("dragend", () => {
      draggedTrajectoryBlockUid = null;
      item.classList.remove("dragging");
    });
    trajectoryBlockStrip.append(item);
  }
  const playhead = enabled && last > 0 ? (current / last) * 100 : 0;
  trajectoryBlockStrip.style.setProperty("--trajectory-playhead", `${playhead}%`);
  trajectoryBlockCount.textContent = `${blocks.length} block${blocks.length === 1 ? "" : "s"}`;
  const selectedBlockIndex = blocks.findIndex(
    (block) => block.uid === selectedTrajectoryBlockUid,
  );
  trajectoryBlockAdd.disabled = !trajectory;
  trajectoryBlockRemove.disabled = selectedBlockIndex < 0;
  trajectoryBlockLeft.disabled = selectedBlockIndex <= 0;
  trajectoryBlockRight.disabled =
    selectedBlockIndex < 0 || selectedBlockIndex >= blocks.length - 1;

  for (const control of [
    trajectoryFirst,
    trajectoryPrevious,
    trajectoryPlay,
    trajectoryNext,
    trajectoryLast,
  ]) {
    control.disabled = !enabled;
  }
  trajectoryFirst.disabled ||= current === 0;
  trajectoryPrevious.disabled ||= current === 0;
  trajectoryNext.disabled ||= current === last;
  trajectoryLast.disabled ||= current === last;
  trajectoryPlay.textContent = trajectoryPlaying ? "Ⅱ" : "▶";
  trajectoryPlay.title = trajectoryPlaying ? "Pause trajectory" : "Play trajectory";
  if (!enabled) trajectoryPlaying = false;
  syncTrajectoryTimer();
}

function renderProjectionControls() {
  projectionReadout.textContent = snapshot.perspective ? "PERSPECTIVE" : "ORTHO";
  document.querySelectorAll("input[name='projection']").forEach((input) => {
    input.checked = input.value === (snapshot.perspective ? "perspective" : "orthographic");
  });
  document.querySelectorAll("[data-projection]").forEach((item) => {
    item.classList.toggle(
      "checked",
      item.dataset.projection === (snapshot.perspective ? "perspective" : "orthographic"),
    );
  });
}

function scheduleSecondaryPanelRender() {
  const generation = ++secondaryPanelRenderGeneration;
  const render = () => {
    if (generation !== secondaryPanelRenderGeneration || !snapshot) return;
    if (sidePanelIsExpanded("Selection")) renderSelectionPanel();
    if (sidePanelIsExpanded("Coloring")) renderColoring();
    if (sidePanelIsExpanded("Symmetry")) renderSymmetry();
    if (sidePanelIsExpanded("Density map")) renderDensity();
    renderSequence();
    if (document.querySelector('[data-bottom-panel="trajectory"]')?.classList.contains("active")) {
      renderTrajectory();
    }
    if (
      sidePanelIsExpanded("Animation") ||
      document.querySelector('[data-ribbon-panel="animation"]')?.classList.contains("active")
    ) {
      renderAnimation();
    }
  };
  if (typeof globalThis.requestIdleCallback === "function") {
    globalThis.requestIdleCallback(render, { timeout: 100 });
  } else {
    setTimeout(render, 0);
  }
}

function renderShell() {
  if (!snapshot) return;
  normalizeSelection();
  const workspaceRenderKey = workspaceChromeRenderKey();
  renderProjectionControls();
  document.querySelectorAll("input[name='background']").forEach((input) => {
    input.checked = input.value === snapshot.background;
  });
  document.querySelectorAll("[data-background]").forEach((item) => {
    item.classList.toggle("checked", item.dataset.background === snapshot.background);
  });
  document
    .querySelector("[data-local-action='color-proof']")
    ?.classList.toggle("checked", snapshot.colorProofing);
  renderUndoControls();
  measureReadout.textContent = snapshot.measureResult || "Select 2 / 3 / 4 atoms";
  renderStatusAndLog();
  metrics.textContent = `${snapshot.scenes.length} scenes / ${snapshot.objects.length} objects`;
  renderSceneTabs(workspaceRenderKey);
  renderSceneTree(workspaceRenderKey);
  renderInspector(workspaceRenderKey);
  renderMolStruct();
  scheduleSecondaryPanelRender();
}

function renderUndoControls() {
  document.querySelectorAll("[data-action='undo']").forEach((item) => {
    item.disabled = !snapshot.canUndo;
  });
  document.querySelectorAll("[data-action='redo']").forEach((item) => {
    item.disabled = !snapshot.canRedo;
  });
  renderHistoryMenu("undo", snapshot.undoDescriptions, snapshot.canUndo);
  renderHistoryMenu("redo", snapshot.redoDescriptions, snapshot.canRedo);
}

function renderStatusAndLog() {
  statusText.textContent = snapshot.status;
  const modelLog = snapshot.logLines || [];
  if (modelLog.length < lastModelLogCount) {
    statusTrail = [];
    statusHistory.textContent = "";
    lastModelLogCount = 0;
  }
  for (const line of modelLog.slice(lastModelLogCount)) recordStatus(line);
  lastModelLogCount = modelLog.length;
  recordStatus(snapshot.status);
}

function renderUndoAndStatus() {
  renderUndoControls();
  renderStatusAndLog();
}

function viewportDimensions() {
  const bounds = viewport.getBoundingClientRect();
  const pixelRatio = preferences?.useHidpi ? window.devicePixelRatio || 1 : 1;
  return {
    width: Math.max(64, Math.round(bounds.width * pixelRatio)),
    height: Math.max(64, Math.round(bounds.height * pixelRatio)),
  };
}

function applyViewport(result, sequence) {
  if (sequence !== requestSequence) return;
  renderedViewportSize = `${result.width}x${result.height}`;
  viewport.classList.toggle(
    "has-image",
    Boolean(result.image || result.lines?.length || result.labels?.length),
  );
  viewportImage.hidden = !result.image;
  if (result.image) viewportImage.src = result.image;
  viewportLines.replaceChildren();
  viewportLines.setAttribute("viewBox", `0 0 ${result.width} ${result.height}`);
  const lineDefs = document.createElementNS("http://www.w3.org/2000/svg", "defs");
  viewportLines.append(lineDefs);
  const solidLineGroups = new Map();
  const solidPointGroups = new Map();
  for (const [index, line] of (result.lines || []).entries()) {
    const x1 = Number(line.x1) || 0;
    const y1 = Number(line.y1) || 0;
    const x2 = Number(line.x2) || 0;
    const y2 = Number(line.y2) || 0;
    const startColor = line.color || "#ffff00";
    const width = Math.max(0.1, Number(line.width) || 1);
    if (line.point) {
      const key = `${startColor}|${width}`;
      const group = solidPointGroups.get(key) || {
        color: startColor,
        width,
        commands: [],
      };
      const half = width / 2;
      group.commands.push(
        `M ${x1 - half} ${y1 - half} h ${width} v ${width} h ${-width} Z`,
      );
      solidPointGroups.set(key, group);
      continue;
    }
    if (!line.endColor || line.endColor === startColor) {
      const key = `${startColor}|${width}|${Boolean(line.stipple)}`;
      const group = solidLineGroups.get(key) || {
        color: startColor,
        width,
        stipple: Boolean(line.stipple),
        commands: [],
      };
      group.commands.push(`M ${x1} ${y1} L ${x2} ${y2}`);
      solidLineGroups.set(key, group);
      continue;
    }

    const element = document.createElementNS("http://www.w3.org/2000/svg", "line");
    element.setAttribute("x1", String(x1));
    element.setAttribute("y1", String(y1));
    element.setAttribute("x2", String(x2));
    element.setAttribute("y2", String(y2));
    if (line.endColor && line.endColor !== startColor) {
      const gradientId = `viewport-line-gradient-${index}`;
      const gradient = document.createElementNS(
        "http://www.w3.org/2000/svg",
        "linearGradient",
      );
      gradient.id = gradientId;
      gradient.setAttribute("gradientUnits", "userSpaceOnUse");
      gradient.setAttribute("x1", String(x1));
      gradient.setAttribute("y1", String(y1));
      gradient.setAttribute("x2", String(x2));
      gradient.setAttribute("y2", String(y2));
      for (const [offset, color] of [["0", startColor], ["1", line.endColor]]) {
        const stop = document.createElementNS("http://www.w3.org/2000/svg", "stop");
        stop.setAttribute("offset", offset);
        stop.setAttribute("stop-color", color);
        gradient.append(stop);
      }
      lineDefs.append(gradient);
      element.setAttribute("stroke", `url(#${gradientId})`);
    }
    element.setAttribute("stroke-width", String(width));
    element.setAttribute("stroke-linecap", "butt");
    element.setAttribute("vector-effect", "non-scaling-stroke");
    if (line.stipple) element.setAttribute("stroke-dasharray", "8 8");
    viewportLines.append(element);
  }
  for (const group of solidLineGroups.values()) {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", group.commands.join(" "));
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", group.color);
    path.setAttribute("stroke-width", String(group.width));
    path.setAttribute("stroke-linecap", "butt");
    path.setAttribute("vector-effect", "non-scaling-stroke");
    if (group.stipple) path.setAttribute("stroke-dasharray", "8 8");
    viewportLines.append(path);
  }
  for (const group of solidPointGroups.values()) {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", group.commands.join(" "));
    path.setAttribute("fill", group.color);
    viewportLines.append(path);
  }
  viewportLabels.replaceChildren();
  for (const label of result.labels || []) {
    const element = document.createElement("span");
    element.className = "viewport-label";
    element.textContent = label.text;
    element.style.left = `${(Number(label.x) / Math.max(1, result.width)) * 100}%`;
    element.style.top = `${(Number(label.y) / Math.max(1, result.height)) * 100}%`;
    element.style.color = label.color || "#ffff00";
    element.style.fontSize = `${Number(label.fontSize) || 12}px`;
    element.style.fontFamily = label.fontName || "sans-serif";
    element.style.fontStyle = label.fontStyle || "normal";
    element.style.fontWeight = label.fontWeight || "normal";
    viewportLabels.append(element);
  }
  renderSize.textContent = `${result.width} × ${result.height}`;
  if (
    snapshot &&
    typeof result.perspective === "boolean" &&
    snapshot.perspective !== result.perspective
  ) {
    snapshot.perspective = result.perspective;
    renderProjectionControls();
  }
  if (typeof result.centerMark === "string") {
    const mark = result.centerMark === "crosshair" ? "cross" : result.centerMark;
    if (mark !== centerMark) applyCenterMark(mark);
  }
  if (result.status) {
    statusText.textContent = result.status;
    recordStatus(result.status);
  }
}

async function renderViewport() {
  if (!invoke) return;
  clearTimeout(renderTimer);
  renderTimer = null;
  if (viewportRenderRequest) {
    viewportRenderQueued = true;
    return viewportRenderRequest;
  }
  viewportRenderRequest = (async () => {
    do {
      viewportRenderQueued = false;
      const size = viewportDimensions();
      const sequence = ++requestSequence;
      try {
        const result = await invoke("render_viewport", size);
        applyViewport(result, sequence);
      } catch (error) {
        showError(error);
      }
    } while (viewportRenderQueued);
  })();
  try {
    await viewportRenderRequest;
  } finally {
    viewportRenderRequest = null;
  }
}

function scheduleRender(delay = 100) {
  clearTimeout(renderTimer);
  renderTimer = setTimeout(renderViewport, delay);
}

async function mutate(command, args = {}, render = renderShell, refreshViewport = true) {
  if (!invoke) {
    showError("Tauri IPC is unavailable. Run this frontend through the Tauri binary.");
    return false;
  }
  setBusy(true);
  try {
    snapshot = await invoke(command, args);
    if (
      [
        "create_style",
        "create_renderer_style",
        "set_style_properties",
        "destroy_style",
        "paste_workspace_style",
        "undo",
        "redo",
      ].includes(command)
    ) {
      workspaceRendererStyleCache.clear();
    }
    syncEmptyMoleculeDetails();
    if (refreshViewport) void renderViewport();
    render();
    scheduleMoleculeDetailsRefresh();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

function mutateWithoutViewport(command, args = {}, render = renderShell) {
  return mutate(command, args, render, false);
}

async function changeWorkspaceRendererType(object, renderer, targetType) {
  const changed = await mutate(
    "change_workspace_renderer_type",
    {
      input: {
        objectUid: object.uid,
        rendererUid: renderer.uid,
        rendererType: targetType,
      },
    },
    () => {},
  );
  if (!changed) return;
  const target = snapshot.objects
    .find((candidate) => candidate.uid === object.uid)
    ?.renderers.filter(
      (candidate) =>
        candidate.rendererType === targetType &&
        (renderer.rendererType === "*selection" || candidate.name === renderer.name),
    )
    .sort((left, right) => right.uid - left.uid)[0];
  selectedObjectUid = object.uid;
  selectedRendererUid = target?.uid ?? null;
  workspaceSelectionKind = target ? "renderer" : "object";
  renderShell();
}

async function mutateWithViewport(command, args = {}, render = renderShell) {
  if (!invoke) {
    showError("Tauri IPC is unavailable. Run this frontend through the Tauri binary.");
    return false;
  }
  setBusy(true);
  const size = viewportDimensions();
  const sequence = ++requestSequence;
  try {
    const update = await invoke(command, { ...args, ...size });
    snapshot = update.snapshot;
    syncEmptyMoleculeDetails();
    applyViewport(update.viewport, sequence);
    if (!update.viewport.image && snapshotHasRenderableContent()) void renderViewport();
    render();
    scheduleMoleculeDetailsRefresh();
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    setBusy(false);
  }
}

function snapshotHasRenderableContent() {
  return Boolean(
    snapshot &&
      (snapshot.objects.length || snapshot.densityMaps.length || snapshot.surfaces.length),
  );
}

function enqueueCameraCommand(action, x, y) {
  const pending = cameraQueue.at(-1);
  if (pending?.action === action) {
    if (action === "pan" || action === "rotate") {
      pending.x += x;
      pending.y += y;
      return;
    }
    if (action === "zoom" || action === "slab") {
      pending.x *= x;
      return;
    }
  }
  cameraQueue.push({ action, x, y });
}

async function flushCameraCommands() {
  try {
    while (cameraQueue.length) {
      const command = cameraQueue.shift();
      const size = viewportDimensions();
      const sequence = ++requestSequence;
      try {
        const result = await invoke("update_camera", { ...command, ...size });
        applyViewport(result, sequence);
      } catch (error) {
        showError(error);
      }
    }
  } finally {
    cameraRequest = null;
  }
}

function cameraCommand(action, x = 0, y = 0) {
  if (!invoke) return Promise.resolve();
  enqueueCameraCommand(action, x, y);
  if (!cameraRequest) cameraRequest = flushCameraCommands();
  return cameraRequest;
}

async function fitWorkspaceItem() {
  if (!invoke) return;
  const target = selectedWorkspaceTarget();
  if (!target) {
    showError("Select an object or renderer first.");
    return;
  }
  const size = viewportDimensions();
  const sequence = ++requestSequence;
  try {
    const result = await invoke("fit_workspace_item", { target, ...size });
    applyViewport(result, sequence);
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
        extensions: ["pdb", "ent", "cif", "mmcif", "qsc", "qdfpdb", "pse", "sdf", "mol2", "gro", "dx", "map", "mrc", "ccp4", "obj", "ply", "view", "cam"],
      },
    ],
  });
  if (!chosen) return;
  const paths = Array.isArray(chosen) ? chosen : [chosen];
  await openSelectedPaths(paths);
}

async function openSelectedPaths(paths) {
  const viewPaths = paths.filter((path) => /\.(view|cam)$/i.test(path));
  const objectPaths = paths.filter((path) => !/\.(view|cam)$/i.test(path));
  let opened = false;
  if (objectPaths.length && (await mutateWithViewport("open_paths", { paths: objectPaths }))) {
    const openedPaths = Array.isArray(snapshot?.openedPaths) ? snapshot.openedPaths : [];
    addRecentFiles(openedPaths.map((path) => ({ path })));
    opened = openedPaths.length > 0;
  }
  for (const path of viewPaths) {
    opened = (await mutate("load_view", { path })) || opened;
  }
  return opened;
}

async function saveScene(saveAs = false) {
  const source = snapshot?.scenes.find((scene) => scene.active)?.source || "";
  if (!saveAs && source) {
    await mutateWithoutViewport("save_scene", { path: source });
    return;
  }
  const path = await dialog?.save?.({
    defaultPath: "scene.qsc",
    filters: [{ name: "CueMol scene", extensions: ["qsc"] }],
  });
  if (path && (await mutateWithoutViewport("save_scene", { path }))) {
    addRecentFile(path, "scene");
  }
}

async function exportPng() {
  const path = await dialog?.save?.({
    defaultPath: "cuemol.png",
    filters: [{ name: "PNG image", extensions: ["png"] }],
  });
  if (!path) return;
  await mutateWithoutViewport(
    "export_png",
    { path, ...viewportDimensions() },
    renderStatusAndLog,
  );
}

async function handleAction(action, depth = 0) {
  switch (action) {
    case "open":
      await openFiles();
      break;
    case "undo":
      await mutate("undo", { depth });
      break;
    case "redo":
      await mutate("redo", { depth });
      break;
    case "clear-undo":
      await mutateWithoutViewport("clear_undo", {}, renderUndoAndStatus);
      break;
    case "save":
      await saveScene();
      break;
    case "save-as":
      await saveScene(true);
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
      await cameraCommand("projection");
      break;
    case "selection-command-apply":
      await applySelectionCommand();
      break;
    case "selection-command-clear":
      selectionInput.value = "";
      break;
    case "selection-editor-apply":
      await applySelectionEditor();
      break;
    case "selection-editor-add":
      addSelectionEditorItem(selectionEditorAddKind.value);
      break;
    case "selection-editor-delete":
      deleteSelectionEditorItem();
      break;
    case "selection-editor-clear":
      selectionEditorItems = [];
      selectionEditorSelectedIndex = -1;
      renderSelectionEditor();
      break;
    case "add-renderer": {
      if (["styleRoot", "style"].includes(workspaceSelectionKind)) {
        const used = new Set(
          activeStyles()
            .filter((style) => style.localIndex !== null)
            .map((style) => style.name),
        );
        let index = 0;
        while (used.has(`style_${index}`)) index += 1;
        const name = globalThis.prompt?.("Style name", `style_${index}`)?.trim();
        if (name) {
          await mutate("create_style", { name });
          selectedStyleIndex = activeStyles().findIndex(
            (style) => style.localIndex !== null && style.name === name,
          );
          workspaceSelectionKind = "style";
          renderShell();
        }
        break;
      }
      if (["cameraRoot", "camera"].includes(workspaceSelectionKind)) {
        const used = new Set(activeCameras().map((camera) => camera.name));
        let index = 0;
        while (used.has(`camera_${index}`)) index += 1;
        const name = globalThis.prompt?.("Camera name", `camera_${index}`)?.trim();
        if (name) {
          await mutate("save_camera", { name });
          selectedCameraName = name;
          workspaceSelectionKind = "camera";
          renderShell();
        }
        break;
      }
      const object = ["object", "renderer", "rendererGroup"].includes(workspaceSelectionKind)
        ? selectedObject()
        : null;
      const map = ["densityMap", "densityRenderer"].includes(workspaceSelectionKind)
        ? selectedDensityMap()
        : null;
      const surface = ["surface", "surfaceRenderer"].includes(workspaceSelectionKind)
        ? selectedSurface()
        : null;
      if (!object && !map && !surface) {
        showError("Select an object first.");
        return;
      }
      const rendererTypeValue = rendererType.value;
      if (map) {
        await mutate("add_density_renderer", {
          mapUid: map.uid,
          rendererType: rendererTypeValue,
        });
      } else if (surface) {
        await mutate("add_surface_renderer", {
          surfaceUid: surface.uid,
        });
      } else {
        await mutate("add_renderer", {
          objectUid: object.uid,
          rendererType: rendererTypeValue,
          groupName:
            workspaceSelectionKind === "rendererGroup" ? selectedRenderer()?.name || null : null,
        });
      }
      break;
    }
    case "delete-workspace-item": {
      if (
        workspaceSelectionKind === "style" &&
        selectedStyle() &&
        selectedStyle().localIndex !== null
      ) {
        const style = selectedStyle();
        await mutate("destroy_style", {
          sceneUid: style.sceneUid,
          localIndex: style.localIndex,
        });
        selectedStyleIndex = null;
        workspaceSelectionKind = "styleRoot";
        renderShell();
        break;
      }
      if (workspaceSelectionKind === "camera" && selectedCamera()) {
        const name = selectedCameraName;
        await mutate("destroy_camera", { name });
        selectedCameraName = null;
        workspaceSelectionKind = "cameraRoot";
        renderShell();
        break;
      }
      const target = selectedWorkspaceTarget();
      if (!target) {
        showError("Select an object or renderer first.");
        return;
      }
      await mutate("delete_workspace_item", { target });
      break;
    }
    case "fit-workspace-item": {
      await fitWorkspaceItem();
      break;
    }
    case "molstruct-select":
      await runMolstructAction("select");
      break;
    case "molstruct-center":
      await runMolstructAction("center");
      break;
    case "molstruct-zoom":
      await runMolstructAction("zoom");
      break;
    case "molstruct-properties":
      break;
  }
}

document.querySelectorAll("[data-history-toggle]").forEach((element) => {
  element.addEventListener("click", (event) => {
    event.stopPropagation();
    const group = element.closest("[data-history-group]");
    const kind = element.dataset.historyToggle;
    const open = !group.classList.contains("open");
    closeMenus();
    if (open) closeHistoryMenus(kind);
  });
});

document.addEventListener("pointerdown", (event) => {
  if (!event.target.closest("[data-history-group]")) closeHistoryMenus();
});

document.querySelectorAll("[data-action]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    const depth = Number(element.dataset.depth || 0);
    handleAction(element.dataset.action, Number.isFinite(depth) ? depth : 0);
  });
});

document.querySelectorAll("[data-dialog]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    showReferenceDialog(element.dataset.dialog);
  });
});

workspaceProperties.addEventListener("click", async () => {
  if (
    workspaceSelectionKind === "style" &&
    selectedStyle() &&
    selectedStyle().localIndex !== null
  ) {
    await showStyleProperties(selectedStyle());
  } else if (workspaceSelectionKind === "camera" && selectedCamera()) {
    await showReferenceDialog("Camera Properties");
  } else if (["renderer", "rendererGroup"].includes(workspaceSelectionKind) && selectedRenderer()) {
    showReferenceDialog("Renderer Properties");
  } else if (workspaceSelectionKind === "densityRenderer" && selectedDensityRenderer()) {
    showReferenceDialog("Renderer Properties");
  } else if (workspaceSelectionKind === "surfaceRenderer" && selectedSurfaceRenderer()) {
    showReferenceDialog("Renderer Properties");
  } else if (workspaceSelectionKind === "object" && selectedObject()) {
    showReferenceDialog("Object Properties");
  } else if (workspaceSelectionKind === "densityMap" && selectedDensityMap()) {
    showReferenceDialog("Object Properties");
  } else if (workspaceSelectionKind === "surface" && selectedSurface()) {
    showReferenceDialog("Object Properties");
  } else {
    showReferenceDialog("Scene Properties");
  }
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
      cameraCommand("projection");
    }
  });
});

document.querySelectorAll("[data-center-mark]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    cameraCommand(`center-mark-${element.dataset.centerMark}`);
  });
});

document.querySelectorAll("[data-panel-menu]").forEach((element) => {
  element.addEventListener("click", () => {
    closeMenus();
    const card = sidePanelCard(element.dataset.panelMenu);
    if (!card) return;
    card.hidden = !card.hidden;
    syncSidePanelVisibility();
    if (!card.hidden) card.scrollIntoView({ block: "nearest" });
    if (!card.hidden && card.classList.contains("open")) renderSidePanel(element.dataset.panelMenu);
    scheduleRender(140);
  });
});

windowsMenuTrigger?.addEventListener("pointerenter", renderWindowsMenu);
windowsMenuTrigger?.addEventListener("focus", renderWindowsMenu);

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
    if (target === "animation") renderAnimation();
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
    const name = element.querySelector("span:nth-child(2)")?.textContent;
    if (open && name) renderSidePanel(name);
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

document.querySelectorAll("[data-selection-tab]").forEach((element) => {
  element.addEventListener("click", () => {
    selectionActiveTab = element.dataset.selectionTab;
    renderSelectionPanel();
  });
});

selectionObject.addEventListener("change", () => {
  selectionObjectUid = Number(selectionObject.value) || null;
  renderSelectionPanel();
});

selectionHistory.addEventListener("change", () => {
  if (selectionHistory.value) selectionInput.value = selectionHistory.value;
  selectionHistory.value = "";
});

molstructObject.addEventListener("change", () => {
  molstructObjectUid = Number(molstructObject.value) || null;
  molstructSelectedKeys.clear();
  molstructOpenKeys.clear();
  molstructSelectionAnchor = null;
  renderMolStruct();
});

coloringTarget.addEventListener("change", () => {
  coloringTargetKey = coloringTarget.value || null;
  coloringPaintSelectedIndex = -1;
  coloringGradientSelectedIndex = -1;
  renderColoring();
});

coloringType.addEventListener("change", () => {
  const target = selectedColoringTarget();
  if (!target) return;
  coloringPaintSelectedIndex = -1;
  coloringGradientSelectedIndex = -1;
  commitColoring(target, coloringType.value, defaultColoringState(coloringType.value, target));
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

trajectoryObject.addEventListener("change", () => {
  selectedTrajectoryUid = Number(trajectoryObject.value) || null;
  selectedTrajectoryBlockUid = null;
  trajectoryPlaying = false;
  renderTrajectory();
});

trajectorySlider.addEventListener("input", () => {
  const trajectory = selectedTrajectory();
  if (!trajectory) return;
  const frame = Number(trajectorySlider.value);
  trajectoryFrames.set(trajectory.objectUid, frame);
  trajectoryFrame.value = String(frame);
});

trajectorySlider.addEventListener("change", () => {
  seekTrajectory(trajectorySlider.value);
});

trajectoryFrame.addEventListener("change", () => {
  seekTrajectory(trajectoryFrame.value);
});

trajectoryFirst.addEventListener("click", () => seekTrajectory(0));
trajectoryPrevious.addEventListener("click", () => {
  const trajectory = selectedTrajectory();
  if (trajectory) seekTrajectory((trajectoryFrames.get(trajectory.objectUid) || 0) - 1);
});
trajectoryNext.addEventListener("click", () => {
  const trajectory = selectedTrajectory();
  if (trajectory) seekTrajectory((trajectoryFrames.get(trajectory.objectUid) || 0) + 1);
});
trajectoryLast.addEventListener("click", () => {
  const trajectory = selectedTrajectory();
  if (trajectory) seekTrajectory(trajectory.frameCount - 1);
});
trajectoryPlay.addEventListener("click", () => {
  trajectoryPlaying = !trajectoryPlaying;
  renderTrajectory();
});
trajectorySpeed.addEventListener("change", () => {
  if (trajectoryTimer) {
    clearInterval(trajectoryTimer);
    trajectoryTimer = null;
  }
  syncTrajectoryTimer();
});

trajectoryBlockAdd.addEventListener("click", async () => {
  const trajectory = selectedTrajectory();
  if (!trajectory) return;
  if (!dialog?.open) {
    showError("The Tauri dialog plugin is unavailable.");
    return;
  }
  const selected = await dialog.open({
    multiple: true,
    directory: false,
    filters: [{ name: "MD trajectory", extensions: ["dcd", "xtc", "trr", "nc"] }],
  });
  const paths = selected ? (Array.isArray(selected) ? selected : [selected]) : [];
  for (const path of paths) {
    const changed = await mutate("append_trajectory_block", {
      objectUid: trajectory.objectUid,
      path,
      nevery: 1,
    });
    if (!changed) break;
    selectedTrajectoryBlockUid = selectedTrajectory()?.blocks.at(-1)?.uid ?? null;
    renderTrajectory();
  }
});

trajectoryBlockRemove.addEventListener("click", async () => {
  const trajectory = selectedTrajectory();
  const index = trajectory?.blocks.findIndex(
    (block) => block.uid === selectedTrajectoryBlockUid,
  );
  if (!trajectory || index < 0) return;
  const changed = await mutate("remove_trajectory_block", {
    objectUid: trajectory.objectUid,
    index,
  });
  if (changed) {
    const blocks = selectedTrajectory()?.blocks || [];
    selectedTrajectoryBlockUid = blocks[Math.min(index, blocks.length - 1)]?.uid ?? null;
    renderTrajectory();
  }
});

async function moveSelectedTrajectoryBlock(direction) {
  const trajectory = selectedTrajectory();
  const from = trajectory?.blocks.findIndex(
    (block) => block.uid === selectedTrajectoryBlockUid,
  );
  const to = from + direction;
  if (!trajectory || from < 0 || to < 0 || to >= trajectory.blocks.length) return;
  await mutate("move_trajectory_block", {
    objectUid: trajectory.objectUid,
    from,
    to,
  });
}

trajectoryBlockLeft.addEventListener("click", () => moveSelectedTrajectoryBlock(-1));
trajectoryBlockRight.addEventListener("click", () => moveSelectedTrajectoryBlock(1));

document.querySelectorAll("input[name='projection']").forEach((input) => {
  input.addEventListener("change", () => {
    const wantsPerspective = input.value === "perspective";
    if (input.checked && snapshot && snapshot.perspective !== wantsPerspective) {
      cameraCommand("projection");
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
  if (!event.target.closest(".workspace-context-menu")) closeWorkspaceContextMenu();
});

referenceDialog.addEventListener("close", () => {
  atomPickerTarget = null;
  viewport.classList.remove("atom-picking");
});

selectionInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    void applySelectionCommand();
  }
});

viewport.addEventListener("contextmenu", (event) => event.preventDefault());
function configuredPointerAction(event) {
  if (event.button === 0 && interactionMode === "rect-select" && !event.shiftKey) {
    return "rect-select";
  }
  const button =
    preferences?.emulateRightButton && event.button === 0 && event.ctrlKey ? 2 : event.button;
  switch (preferences?.mousePreset) {
    case "CootViewInConf":
      if (button === 2) return "zoom";
      if (button === 0 && event.ctrlKey) return "pan";
      if (button === 0 && event.shiftKey) return "slab";
      return button === 0 ? "rotate" : null;
    case "PymolViewInConf":
      if (button === 1) return "pan";
      if (button === 2) return "zoom";
      return button === 0 ? "rotate" : null;
    case "IphoneViewInConf":
      if (button === 2) return "pan";
      if (button === 0 && event.shiftKey) return "slab";
      return button === 0 ? "rotate" : null;
    case "TrackpadViewInConf":
    case "DefaultViewInConf":
    default:
      if (button === 2) return "pan";
      if (button === 0 && event.shiftKey) return "zoom-slab";
      return button === 0 ? "rotate" : null;
  }
}

viewport.addEventListener("pointerdown", (event) => {
  const action = configuredPointerAction(event);
  if (!action) return;
  viewport.setPointerCapture(event.pointerId);
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

async function fillAtomPicker(x, y) {
  const target = atomPickerTarget;
  if (!target) return;
  try {
    const atom = await invoke("pick_atom_identifier", {
      x,
      y,
      ...viewportDimensions(),
    });
    if (atomPickerTarget !== target) return;
    if (!atom) {
      statusText.textContent = "No atom at cursor";
      return;
    }
    if (Number(target.object.value) !== Number(atom.objectUid)) {
      statusText.textContent = "Pick an atom from the selected molecule";
      return;
    }
    target.input.value = String(atom.aid);
    target.input.focus();
    statusText.textContent = `Picked atom ${atom.aid} for bond editor`;
  } catch (error) {
    showError(error);
  }
}

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
      if (atomPickerTarget) {
        void fillAtomPicker(startX, startY);
        return;
      }
      mutateWithViewport("pick_atom", {
        x: startX,
        y: startY,
        ...viewportDimensions(),
      });
    }
    return;
  }
  if (completed.action === "rect-select") {
    mutateWithViewport("select_screen_rect", {
      x1: startX,
      y1: startY,
      x2: startX + completed.dx,
      y2: startY + completed.dy,
      ...viewportDimensions(),
    });
  } else if (completed.action === "zoom" || completed.action === "slab") {
    const delta = Math.abs(completed.dx) > Math.abs(completed.dy) ? completed.dx : completed.dy;
    cameraCommand(completed.action, Math.exp(delta * 0.01));
  } else if (completed.action === "zoom-slab") {
    if (Math.abs(completed.dx) > Math.abs(completed.dy)) {
      cameraCommand("slab", Math.exp(completed.dx * 0.01));
    } else {
      cameraCommand("zoom", Math.exp(completed.dy * 0.01));
    }
  } else {
    const radius = Math.max(0.1, preferences?.trackballRadius || 0.8);
    const scale = completed.action === "rotate" ? 0.35 * (0.8 / radius) : 1;
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
    const preset = preferences?.mousePreset || "DefaultViewInConf";
    const trackpad = preset === "TrackpadViewInConf" || preferences?.multitouchPad;
    let action = "zoom";
    let x = Math.exp(Math.max(-500, Math.min(500, event.deltaY)) * 0.001);
    let y = 0;
    if (trackpad && !event.ctrlKey) {
      action = "pan";
      x = event.deltaX;
      y = event.deltaY;
    } else if (preset === "PymolViewInConf" && !(event.ctrlKey && event.shiftKey)) {
      action = "slab";
    }
    if (!preferences?.momentumScroll) {
      cameraCommand(action, x, y);
      return;
    }
    clearTimeout(wheelTimer);
    if (wheelPending?.action === action) {
      if (action === "pan") {
        wheelPending.x += x;
        wheelPending.y += y;
      } else {
        wheelPending.x *= x;
      }
    } else {
      wheelPending = { action, x, y };
    }
    wheelTimer = setTimeout(() => {
      const pending = wheelPending;
      wheelPending = null;
      if (pending) cameraCommand(pending.action, pending.x, pending.y);
    }, 28);
  },
  { passive: false },
);

window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeWorkspaceContextMenu();
  const editing = ["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement?.tagName);
  const shortcut = !editing && preferences?.shortcuts?.find(
    (item) =>
      item.key === event.key &&
      item.ctrl === event.ctrlKey &&
      item.alt === event.altKey &&
      item.shift === event.shiftKey,
  );
  if (shortcut) {
    event.preventDefault();
    cameraCommand("pan", shortcut.id === "view-trans-plusx" ? 1 : -1, 0);
  } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "o") {
    event.preventDefault();
    openFiles();
  } else if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    saveScene(event.shiftKey);
  } else if (!editing && event.key.toLowerCase() === "f") {
    cameraCommand("fit");
  } else if (!editing && event.key.toLowerCase() === "p") {
    cameraCommand("projection");
  }
});

window.addEventListener("dragover", (event) => event.preventDefault());
window.addEventListener("drop", (event) => {
  event.preventDefault();
  const paths = [...event.dataTransfer.files].map((file) => file.path).filter(Boolean);
  if (paths.length) {
    openSelectedPaths(paths);
  }
});

async function start() {
  if (!invoke) {
    statusText.textContent = "Tauri IPC unavailable";
    showError("Run cargo run -p cuemol-desktop --bin cuemol-desktop-tauri --features tauri-gui");
    return;
  }
  try {
    renderRecentMenu();
    const sequence = ++requestSequence;
    const initial = await invoke("get_initial_state", viewportDimensions());
    preferences = initial.preferences;
    snapshot = initial.snapshot;
    syncEmptyMoleculeDetails();
    applyViewport(initial.viewport, sequence);
    if (snapshotHasRenderableContent()) void renderViewport();
    renderShell();
    scheduleMoleculeDetailsRefresh();
    new ResizeObserver(() => {
      const size = viewportDimensions();
      if (`${size.width}x${size.height}` !== renderedViewportSize) scheduleRender(140);
    }).observe(viewport);
    if (initial.startupPathsPending) {
      requestAnimationFrame(() => {
        setTimeout(() => {
          statusText.textContent = "Loading startup files…";
          void mutateWithViewport("open_initial_paths");
        }, 0);
      });
    }
  } catch (error) {
    showError(error);
  }
}

start();

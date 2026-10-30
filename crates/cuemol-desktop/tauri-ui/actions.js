"use strict";

let actionData = { steps: [], files: [], done: 0, running: false };
let actionSelection = new Set();
let actionAnchor = null;
let actionExpanded = new Set();
let actionLibrarySelection = null;
let actionInspection = null;
let actionRefreshKey = "";
let actionRefreshing = false;
let actionRefreshPending = false;
let actionRefreshForced = false;

function actionTargets() {
  const selected = workspaceSelectionKind === "multiple" ? workspaceMultipleSelection.map(item => item.objectUid)
    : ["object", "renderer", "rendererGroup"].includes(workspaceSelectionKind) ? [selectedObjectUid]
      : ["densityMap", "densityRenderer"].includes(workspaceSelectionKind) ? [selectedDensityMapUid]
        : ["surface", "surfaceRenderer"].includes(workspaceSelectionKind) ? [selectedSurfaceUid]
          : ["lwObject", "lwRenderer"].includes(workspaceSelectionKind) ? [selectedLwObjectUid] : [];
  const scene = snapshot?.activeSceneUid;
  const objects = [...(snapshot?.objects || []), ...(snapshot?.densityMaps || []), ...(snapshot?.surfaces || []), ...(snapshot?.lwObjects || [])].filter(item => item.sceneUid === scene);
  const ids = new Set(selected.filter(Number.isFinite));
  return ids.size ? objects.filter(item => ids.has(item.uid)) : objects.length === 1 ? objects : [];
}

async function actionsRequest(operation, args = {}) {
  return invoke("actions_request", { operation, args });
}

async function refreshActionsPane(force = false) {
  if (cueMolActivity !== "actions" || !snapshot) return;
  if (actionRefreshing) {
    actionRefreshPending = true;
    actionRefreshForced ||= force;
    return;
  }
  const key = JSON.stringify([snapshot.activeSceneUid, snapshot.actionHistoryKey, !consoleStop.hidden, actionTargets().map(item => [item.uid, item.name])]);
  if (!force && key === actionRefreshKey) return;
  actionRefreshing = true;
  try {
    const value = await actionsRequest("list");
    if (!value?.steps || !value?.files) return;
    actionData = value;
    actionRefreshKey = key;
    const live = new Set([0, ...value.steps.map(item => item.id)]);
    actionSelection = new Set([...actionSelection].filter(id => live.has(id)));
    actionExpanded = new Set([...actionExpanded].filter(id => live.has(id)));
    if (actionInspection && !live.has(actionInspection.id)) actionInspection = null;
    renderActionsPane();
  } catch (error) { showError(error); }
  finally {
    actionRefreshing = false;
    if (actionRefreshPending) {
      const forced = actionRefreshForced;
      actionRefreshPending = false;
      actionRefreshForced = false;
      void refreshActionsPane(forced);
    }
  }
}

function actionButton(label, callback, enabled = true) {
  const item = button(label, "action-command", () => void Promise.resolve().then(callback).catch(showError));
  item.disabled = !enabled;
  return item;
}

function selectedActionIds() {
  return actionData.steps.filter(item => actionSelection.has(item.id)).map(item => item.id);
}

async function applyActionUpdate(update) {
  const generation = ++consoleGeneration;
  clearTimeout(consolePollTimer);
  showConsoleUpdate(update);
  if (update.running) scheduleConsolePoll(generation);
  await refreshActionsPane(true);
}

async function inspectAction(id, entryIndex = null) {
  if (!id) return;
  const value = await actionsRequest("inspect", { id });
  if (!actionData.steps.some(item => item.id === id)) return;
  if (entryIndex !== null) value.entries = value.entries.slice(entryIndex, entryIndex + 1);
  actionInspection = value;
  cueMolInspectorClosed = false;
  renderActionInspector();
}

function renderActionInspector() {
  if (!actionInspection) return false;
  contentHolder.classList.toggle("has-inspector", !cueMolInspectorClosed);
  document.getElementById("inspector-title").textContent = actionInspection.label;
  const fields = document.getElementById("inspector-fields");
  fields.replaceChildren();
  const source = document.createElement("p"); source.textContent = `Source: ${actionInspection.source}`; fields.append(source);
  for (const entry of actionInspection.entries) {
    const title = document.createElement("h3"); title.textContent = entry.command; fields.append(title);
    for (const [name, value] of entry.args) {
      const row = document.createElement("label"); row.textContent = name;
      const text = document.createElement("pre"); text.textContent = value; row.append(text); fields.append(row);
    }
  }
  if (!actionInspection.entries.length) fields.append(document.createTextNode("No operation was recorded for this edit."));
  return true;
}

function selectAction(id, event) {
  const ids = [0, ...actionData.steps.map(item => item.id)];
  if (event.shiftKey && ids.includes(actionAnchor)) {
    if (!event.metaKey && !event.ctrlKey) actionSelection.clear();
    const a = ids.indexOf(actionAnchor), b = ids.indexOf(id);
    ids.slice(Math.min(a, b), Math.max(a, b) + 1).forEach(value => actionSelection.add(value));
  } else {
    if (!event.metaKey && !event.ctrlKey) actionSelection.clear();
    if (actionSelection.has(id)) actionSelection.delete(id); else actionSelection.add(id);
    actionAnchor = id;
  }
  renderActionsPane();
}

function askActionName(title, initial, allowMacro, submit) {
  const modal = document.createElement("dialog"); modal.className = "action-name-dialog";
  const form = document.createElement("form");
  const heading = document.createElement("h2"); heading.textContent = title;
  const label = document.createElement("label"); label.textContent = "Name";
  const input = document.createElement("input"); input.value = initial; input.required = true; input.setAttribute("aria-label", "Action name"); label.append(input);
  const check = document.createElement("input"); check.type = "checkbox"; check.checked = true;
  const macroLabel = document.createElement("label"); macroLabel.append(check, " Reusable macro for another object");
  const error = document.createElement("p"); error.className = "action-error"; error.setAttribute("role", "alert");
  const save = document.createElement("button"); save.type = "submit"; save.textContent = "Save";
  const cancel = document.createElement("button"); cancel.type = "button"; cancel.textContent = "Cancel"; cancel.onclick = () => modal.close();
  form.append(heading, label); if (allowMacro) form.append(macroLabel); form.append(error, save, cancel); modal.append(form);
  modal.addEventListener("close", () => modal.remove());
  form.addEventListener("submit", async event => {
    event.preventDefault(); save.disabled = true;
    try { await submit(input.value, check.checked); modal.close(); await refreshActionsPane(true); }
    catch (reason) { error.textContent = String(reason); }
    finally { save.disabled = false; }
  });
  document.body.append(modal); modal.showModal(); input.select();
}

function renderActionsPane() {
  const selected = selectedActionIds();
  const busy = actionData.running || !consoleStop.hidden;
  const current = actionData.steps.filter(item => !item.undone).at(-1)?.id || 0;
  const goto = id => applyActionUpdateThenGoto(id);
  const historyControls = document.getElementById("action-history-controls");
  historyControls.replaceChildren(
    actionButton("Undo", () => goto(actionData.done > 1 ? actionData.steps[actionData.done - 2].id : 0), !busy && actionData.done > 0),
    actionButton("Redo", () => goto(actionData.steps[actionData.done].id), !busy && actionData.done < actionData.steps.length),
    actionButton("Go to step", () => goto([...actionSelection][0]), !busy && actionSelection.size === 1),
    actionButton("Copy commands", async () => navigator.clipboard.writeText((await actionsRequest("commands", { ids: selected })).text), !busy && selected.length > 0),
    actionButton("Save QAX…", async () => {
      const path = await chooseOutputPath("actions.qax", "CueMol actions", ["qax"]);
      if (path) await actionsRequest("export_history", { ids: selected, path });
    }, !busy && selected.length > 0),
    actionButton("Save to Actions…", () => askActionName("Save to Actions", "My action", true, (name, asMacro) => actionsRequest("save_history", { ids: selected, name, asMacro })), !busy && selected.length > 0),
  );
  const history = document.getElementById("action-history-list"); history.replaceChildren();
  for (const step of [{ id: 0, label: "Initial state", entryCount: 0 }, ...actionData.steps]) {
    const row = document.createElement("div"); row.className = "action-row"; row.setAttribute("role", "treeitem"); row.setAttribute("aria-selected", String(actionSelection.has(step.id))); row.dataset.stepId = step.id;
    row.classList.toggle("undone", Boolean(step.undone)); row.classList.toggle("current", current === step.id);
    if (step.entryCount > 1) row.append(actionButton(actionExpanded.has(step.id) ? "▾" : "▸", async () => {
      if (actionExpanded.has(step.id)) actionExpanded.delete(step.id); else actionExpanded.add(step.id);
      renderActionsPane();
      if (actionExpanded.has(step.id)) await renderActionEntries(step.id);
    }));
    const title = document.createElement("span"); title.textContent = `${current === step.id ? "● " : ""}${step.label}`; row.append(title);
    row.onclick = event => { if (!event.target.closest("button")) selectAction(step.id, event); };
    row.ondblclick = () => void inspectAction(step.id).catch(showError);
    row.oncontextmenu = event => { event.preventDefault(); selectAction(step.id, event); void inspectAction(step.id).catch(showError); };
    history.append(row);
    if (actionExpanded.has(step.id)) void renderActionEntries(step.id).catch(showError);
  }
  renderActionLibrary(busy);
}

async function renderActionEntries(id) {
  const data = await actionsRequest("inspect", { id });
  if (!actionExpanded.has(id)) return;
  const row = document.querySelector(`#action-history-list [data-step-id="${id}"]`);
  if (!row || row.nextElementSibling?.dataset.entriesFor === String(id)) return;
  const group = document.createElement("div"); group.setAttribute("role", "group"); group.dataset.entriesFor = id;
  data.entries.forEach((entry, index) => {
    const child = actionButton(entry.command, async () => { selectAction(id, {}); await inspectAction(id, index); });
    child.classList.add("action-operation"); group.append(child);
  });
  row.after(group);
}

async function applyActionUpdateThenGoto(id) {
  await applyActionUpdate(await actionsRequest("goto", { id }));
}

function renderActionLibrary(busy) {
  const list = document.getElementById("action-library-list"); list.replaceChildren();
  let chosen = null;
  for (const bundled of [true, false]) {
    const group = document.createElement("details"); group.open = true;
    const heading = document.createElement("summary"); heading.textContent = bundled ? "Presets" : "User Actions"; group.append(heading);
    for (const file of actionData.files.filter(item => item.bundled === bundled)) {
      const entries = [...(file.actionCount || file.error ? [{ name: null, label: file.name }] : []), ...file.macros];
      for (const macro of entries) {
        const key = JSON.stringify([bundled, file.key || file.name, macro.name]);
        const row = actionButton(macro.label, () => { actionLibrarySelection = key; renderActionLibrary(busy); });
        row.classList.add("action-library-row"); row.setAttribute("role", "treeitem"); row.setAttribute("aria-selected", String(key === actionLibrarySelection));
        row.title = macro.error || macro.description || file.error || ""; group.append(row);
        if (key === actionLibrarySelection) chosen = { file, macro };
      }
    }
    list.append(group);
  }
  const targets = actionTargets();
  document.getElementById("action-targets").textContent = targets.length ? `Target: ${targets.map(item => item.name).join(", ")}` : "Target: select an object in the scene tree";
  document.getElementById("action-description").textContent = chosen?.macro.error || chosen?.file.error || chosen?.macro.description || "";
  const args = chosen ? { file: chosen.file.key || chosen.file.name, bundled: chosen.file.bundled, macro: chosen.macro.name } : {};
  const controls = document.getElementById("action-library-controls");
  controls.replaceChildren(
    actionButton("Run", async () => applyActionUpdate(await actionsRequest("run", { ...args, targetIds: targets.map(item => item.uid) })), Boolean(chosen && !busy && !chosen.file.error && !chosen.macro.error && (!chosen.macro.target_type || targets.length))),
    actionButton("Refresh", () => refreshActionsPane(true)),
    actionButton("Import…", async () => {
      const path = await dialog.open({ multiple: false, filters: [{ name: "CueMol actions", extensions: ["qax"] }] });
      if (path) { await actionsRequest("import", { path }); await refreshActionsPane(true); }
    }),
  );
  if (!chosen) return;
  if (chosen.file.bundled) controls.append(actionButton("Copy to User Actions…", () => askActionName("Copy action", chosen.macro.name || chosen.file.name, false, name => actionsRequest("copy", { ...args, name }))));
  else controls.append(
    actionButton("Rename…", () => askActionName("Rename action", chosen.file.name, false, name => actionsRequest("rename", { ...args, name }))),
    actionButton("Delete…", async () => {
      if (await showConfirmationDialog({ title: "Delete action", message: `Delete ${chosen.file.name}?`, confirmLabel: "Delete" })) { await actionsRequest("delete", args); actionLibrarySelection = null; await refreshActionsPane(true); }
    }),
  );
  controls.append(actionButton("Export…", async () => {
    const path = await chooseOutputPath(`${chosen.file.name}.qax`, "CueMol actions", ["qax"]);
    if (path) await actionsRequest("export_file", { ...args, path });
  }));
}

document.getElementById("action-history-list").addEventListener("keydown", event => {
  const ids = [0, ...actionData.steps.map(step => step.id)];
  const index = Math.max(0, ids.indexOf(actionAnchor));
  if (["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
    event.preventDefault();
    const next = event.key === "Home" ? 0 : event.key === "End" ? ids.length - 1 : Math.max(0, Math.min(ids.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)));
    selectAction(ids[next], event);
    document.querySelector(`#action-history-list [data-step-id="${ids[next]}"]`)?.scrollIntoView({ block: "nearest" });
  } else if (event.key === "Enter") { event.preventDefault(); void inspectAction(actionAnchor).catch(showError); }
});

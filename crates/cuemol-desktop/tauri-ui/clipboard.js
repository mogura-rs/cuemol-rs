"use strict";

let clipboardScope = null;
let clipboardTextTarget = null;

function clipboardEditable(element) {
  return element?.matches("input, textarea, select, [contenteditable=true]") && !element.disabled;
}

function clipboardMenu(element) {
  return element?.closest(".menu, .workspace-context-menu, [data-history-group]");
}

document.addEventListener("pointerdown", event => {
  if (clipboardMenu(event.target)) return;
  clipboardTextTarget = clipboardEditable(event.target) ? event.target : null;
  clipboardScope = event.target.closest("#scene-tree") ? "tree"
    : event.target.closest("#coloring-paint-list") ? "paint"
      : event.target.closest("#action-history-list") ? "actions" : null;
}, true);

document.addEventListener("focusin", event => {
  if (!clipboardMenu(event.target)) clipboardTextTarget = clipboardEditable(event.target) ? event.target : null;
}, true);

function clipboardTextContext() {
  if (clipboardEditable(document.activeElement)) return document.activeElement;
  return clipboardTextTarget?.isConnected ? clipboardTextTarget : null;
}

async function textClipboardAction(action, field) {
  field?.focus();
  if (action === "select-all") {
    if (typeof field?.select === "function") field.select();
    else document.execCommand("selectAll");
    return;
  }
  if (action === "undo" || action === "redo") {
    document.execCommand(action);
    return;
  }
  const start = field?.selectionStart;
  const end = field?.selectionEnd;
  const original = field?.value;
  if (action === "paste") {
    if (!field || field.readOnly) return;
    const text = await invoke("clipboard_text");
    if (!field.isConnected || clipboardTextContext() !== field || field.value !== original) return;
    field.focus();
    if (typeof start === "number") field.setSelectionRange(start, end);
    document.execCommand("insertText", false, text);
  } else {
    const text = typeof start === "number" ? field.value.slice(start, end) : window.getSelection()?.toString();
    if (!text) return;
    await invoke("set_clipboard_text", {text});
    if (action === "cut" && field?.isConnected && !field.readOnly && clipboardTextContext() === field && field.value === original) {
      field.focus();
      if (typeof start === "number") field.setSelectionRange(start, end);
      document.execCommand("delete");
    }
  }
}

async function treeClipboardAction(action) {
  const before = snapshot;
  const target = currentWorkspaceSelectionTarget();
  const scene = snapshot?.scenes?.find(scene => scene.uid === (target?.kind === "scene" ? target.objectUid : snapshot.activeSceneUid));
  if (!scene) return;
  if (action === "paste") {
    const kind = await invoke("clipboard_kind");
    if (snapshot !== before) return;
    if (kind === "object") await pasteWorkspaceObject(scene);
    else if (kind === "camera") await pasteWorkspaceCamera(scene);
    else if (kind === "style") await pasteWorkspaceStyle(scene);
    else if (kind === "renderer" && target && !["scene", "camera", "style"].includes(target.kind)) {
      const group = target.kind === "rendererGroup" || target.isGroup;
      await pasteWorkspaceRenderers(target.objectUid, group ? target.rendererUid : null);
    }
    return;
  }
  const targets = workspaceSelectionKind === "multiple" ? [...workspaceMultipleSelection] : target ? [target] : [];
  if (!targets.length) return;
  let copied = false;
  if (targets.length > 1) {
    if (targets.every(item => item.rendererUid !== null && workspaceRendererTargetClipboardKind(item))) copied = await copyWorkspaceRenderers(targets);
  } else if (target.kind === "camera") {
    const camera = snapshot.cameras.find(item => item.sceneUid === target.objectUid && item.name === target.rendererUid);
    if (camera) copied = await copyWorkspaceCamera(camera.sceneUid, camera.name);
  } else if (target.kind === "style") {
    const style = snapshot.styles.find(item => item.ownerSceneUid === target.objectUid && item.localIndex === target.rendererUid);
    if (style) copied = await copyWorkspaceStyle(target.objectUid, style);
  } else if (target.rendererUid !== null) {
    copied = await copyWorkspaceRenderer(target.objectUid, target.rendererUid);
  } else if (target.kind !== "scene") copied = await copyWorkspaceObject(target.objectUid);
  if (copied) {
    if (action === "cut" && snapshot === before) await deleteWorkspaceMultipleItems(targets);
  }
}

async function paintClipboardAction(action) {
  const before = snapshot;
  const target = selectedColoringTarget();
  if (!target) return;
  const state = coloringState(target);
  const index = coloringPaintSelectedIndex;
  if (state.type !== "PaintColoring") return;
  if (action === "paste") {
    const text = await invoke("clipboard_text");
    const [magic, meta, ...body] = text.replace(/^\uFEFF/, "").split("\n").map(line => line.trim());
    if (magic !== "CueMolClipboard/1" || JSON.parse(meta).kind !== "paint") throw new Error("The clipboard does not contain paint rows");
    const rows = JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(body.join("").replace(/\s/g, "")), char => char.charCodeAt(0))));
    if (!Array.isArray(rows)) throw new Error("Invalid paint rows");
    const valid = rows.filter(row => row && typeof row.sel === "string" && typeof row.col === "string");
    if (!valid.length) throw new Error("The clipboard contains no valid paint rows");
    if (snapshot !== before || selectedColoringTarget()?.key !== target.key) return;
    state.paintEntries.splice(index < 0 ? state.paintEntries.length : index, 0, ...valid.map(row => ({selection:row.sel,color:row.col})));
  } else {
    const row = state.paintEntries[index];
    if (!row) return;
    const bytes = new TextEncoder().encode(JSON.stringify([{sel:row.selection,col:row.color}]));
    const data = btoa(Array.from(bytes, byte => String.fromCharCode(byte)).join(""));
    await invoke("set_clipboard_text", {text:`CueMolClipboard/1\n{"kind":"paint","form":"single"}\n${data}\n`});
    if (action !== "cut" || snapshot !== before || selectedColoringTarget()?.key !== target.key) return;
    state.paintEntries.splice(index, 1);
  }
  await commitColoring(target, "PaintColoring", state);
}

async function dispatchClipboard(action) {
  try {
    const field = clipboardTextContext();
    if (field || document.querySelector("dialog[open]")) {
      await textClipboardAction(action, field);
      return true;
    }
    if (action === "undo" || action === "redo") return false;
    if (action === "select-all") { document.execCommand("selectAll"); return true; }
    if (action === "copy" && window.getSelection()?.toString()) {
      await textClipboardAction(action, null);
    } else if (clipboardScope === "tree") await treeClipboardAction(action);
    else if (clipboardScope === "paint") await paintClipboardAction(action);
    else if (clipboardScope === "actions" && action === "copy") {
      const ids = selectedActionIds();
      if (ids.length) await invoke("set_clipboard_text", {text:(await actionsRequest("commands", {ids})).text});
    }
    return true;
  } catch (error) { showError(error); return true; }
}

function createViewNumeric({ label, valueField, wheelAction, setAction, decimals, relative, send, value, current }) {
  const input = document.createElement("input");
  input.type = "text";
  input.inputMode = "decimal";
  input.className = "view-numeric";
  input.setAttribute("aria-label", label.replace(/:$/, ""));
  input.readOnly = true;
  if (valueField) input.dataset.viewField = valueField;
  let editing = false;
  let pointer = null;
  let accumulator = 0;
  const scalar = ["zoom", "slab", "distance"].includes(valueField);
  const refresh = () => {
    if (editing || pointer?.dragging) return;
    const number = relative ? 0 : value();
    input.value = Number.isFinite(number) ? Number(number).toFixed(decimals) : "";
  };
  const valid = () => input.isConnected && current();
  const adjust = delta => {
    if (!valid() || !Number.isFinite(delta) || !delta) return;
    accumulator += delta;
    if (relative) input.value = String(accumulator);
    void send(wheelAction, delta).then(refresh);
  };
  const finishEdit = commit => {
    if (!editing) return;
    editing = false;
    delete input.dataset.editing;
    input.readOnly = true;
    let number = Number(input.value);
    if (commit && input.value.trim() && Number.isFinite(number) && valid()) {
      if (valueField === "zoom") number = Math.max(0.01, number);
      else if (["slab", "distance"].includes(valueField)) number = Math.max(0, number);
      void send(setAction, number).then(refresh);
    }
    accumulator = 0;
    refresh();
  };
  const beginEdit = () => {
    if (pointer) return;
    editing = true;
    input.dataset.editing = "true";
    input.readOnly = false;
    input.select();
  };
  input.addEventListener("focus", beginEdit);
  input.addEventListener("blur", () => finishEdit(true));
  input.addEventListener("keydown", event => {
    if (event.key === "Escape") {
      event.preventDefault();
      finishEdit(false);
      input.blur();
    } else if (event.key === "Enter") {
      event.preventDefault();
      finishEdit(true);
      input.blur();
      if (["center-0", "center-1"].includes(valueField)) {
        input.closest(".view-dial-row").nextElementSibling?.querySelector("input")?.focus();
      }
    } else if (["ArrowUp", "ArrowDown"].includes(event.key)) {
      event.preventDefault();
      editing = false;
      input.readOnly = true;
      delete input.dataset.editing;
      adjust(event.key === "ArrowUp" ? 1 : -1);
      accumulator = 0;
      void Promise.resolve().then(refresh);
    } else if (!editing && event.key.length === 1 && !event.ctrlKey && !event.metaKey) {
      beginEdit();
    }
  });
  input.addEventListener("pointerdown", event => {
    if (event.button !== 0 || editing) return;
    const initial = relative ? 0 : value();
    pointer = { id: event.pointerId, start: event.clientX, previous: event.clientX, dragging: false, raw: initial, sent: initial };
    accumulator = 0;
    input.setPointerCapture(event.pointerId);
    event.preventDefault();
  });
  input.addEventListener("pointermove", event => {
    if (!pointer || event.pointerId !== pointer.id) return;
    if (!pointer.dragging && Math.abs(event.clientX - pointer.start) <= 4) return;
    pointer.dragging = true;
    input.dataset.dragging = "true";
    pointer.raw += (event.clientX - pointer.previous) * (event.shiftKey ? 0.1 : 1);
    pointer.previous = event.clientX;
    const minimum = valueField === "zoom" ? 0.01 : scalar ? 0 : -Infinity;
    pointer.raw = Math.max(minimum, pointer.raw);
    const snap = event.shiftKey ? valueField?.startsWith("center-") ? 0.01 : 0.1 : event.ctrlKey || event.metaKey ? 10 : 1;
    const next = Math.max(minimum, Math.round(pointer.raw / snap) * snap);
    const delta = next - pointer.sent;
    pointer.sent = next;
    if (scalar) { if (valid()) void send(setAction, next).then(refresh); }
    else adjust(delta);
  });
  const release = (event, cancelled = false) => {
    if (!pointer || event.pointerId !== pointer.id) return;
    const dragged = pointer.dragging;
    pointer = null;
    delete input.dataset.dragging;
    if (input.hasPointerCapture(event.pointerId)) input.releasePointerCapture(event.pointerId);
    accumulator = 0;
    refresh();
    if (!dragged && !cancelled) { input.focus(); beginEdit(); }
  };
  input.addEventListener("pointerup", event => release(event));
  input.addEventListener("pointercancel", event => release(event, true));
  input.addEventListener("lostpointercapture", event => release(event, true));
  input.addEventListener("click", event => event.preventDefault());
  refresh();
  return input;
}

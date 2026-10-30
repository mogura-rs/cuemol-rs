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

const cueMolPropertyInspector = {key: '', target: null, rows: [], sections: [], source: null, serial: 0, tab: 'Properties', filter: '', error: '', localToggles: {}, optionalValues: {}};
let cueMolViewInspectorTimer = null;

function refreshCueMolViewInspector() {
  if (workspaceSelectionKind !== 'view' || cueMolInspectorClosed || cueMolViewInspectorTimer !== null) return;
  cueMolViewInspectorTimer = setTimeout(() => {
    cueMolViewInspectorTimer = null;
    if (workspaceSelectionKind !== 'view' || cueMolInspectorClosed) return;
    cueMolPropertyInspector.source = null;
    renderTritiumInspector(null, null, null);
  }, 80);
}

function renderTritiumInspector(owner, camera, style) {
  if (cueMolActivity === "actions" && renderActionInspector()) return;
  const rendererUid = workspaceSelectionKind === "densityRenderer" ? selectedDensityRendererUid
    : workspaceSelectionKind === "surfaceRenderer" ? selectedSurfaceRendererUid
    : workspaceSelectionKind === "lwRenderer" ? selectedLwRendererUid
    : ["renderer", "rendererGroup"].includes(workspaceSelectionKind) ? selectedRendererUid : null;
  const renderer = owner?.renderers?.find(item => item.uid === rendererUid);
  const context = workspaceSelectionKind === 'scene' ? snapshot?.scenes.find(scene => scene.active)
    : workspaceSelectionKind === 'view' ? snapshot?.views.find(view => view.active) : null;
  const target = context || renderer || owner || camera || style;
  const key = target ? `${snapshot?.activeSceneUid}:${workspaceSelectionKind}:${target.uid ?? target.name}` : "";
  if (key !== cueMolInspectorTarget) {
    cueMolInspectorTarget = key;
    cueMolInspectorClosed = false;
    cueMolPropertyInspector.tab = workspaceSelectionKind === 'view' ? 'Generic' : 'Properties';
    cueMolPropertyInspector.filter = '';
  }
  contentHolder.classList.toggle("has-inspector", Boolean(target) && !cueMolInspectorClosed);
  document.getElementById("inspector-title").textContent = target?.name || "No object";
  const fields = document.getElementById("inspector-fields");
  if (!target || (!owner && !context)) {
    cueMolPropertyInspector.key = '';
    cueMolPropertyInspector.target = null;
    cueMolPropertyInspector.serial += 1;
    fields.replaceChildren();
    return;
  }
  const node = {nodeType: context ? workspaceSelectionKind : renderer ? 'renderer' : 'object', nodeId: target.uid};
  const inspector = cueMolPropertyInspector;
  if (inspector.key === key && inspector.source === snapshot) return;
  const selectedChanged = inspector.key !== key;
  inspector.key = key;
  inspector.target = node;
  inspector.source = snapshot;
  inspector.error = '';
  if (selectedChanged) { inspector.rows = []; inspector.localToggles = {}; inspector.optionalValues = {}; fields.textContent = 'Loading properties…'; }
  const serial = ++inspector.serial;
  void invoke('inspector_request', {tool:'list_node_props', args:node}).then(result => {
    if (inspector.serial !== serial || inspector.key !== key) return;
    inspector.rows = result.properties || [];
    inspector.sections = result.sections || [];
    paintCueMolProperties();
  }).catch(error => {
    if (inspector.serial !== serial || inspector.key !== key) return;
    inspector.error = String(error);
    paintCueMolProperties();
  });
}

async function editCueMolProperty(tool, args) {
  const inspector = cueMolPropertyInspector;
  const key = inspector.key;
  const target = {...inspector.target};
  setBusy(true);
  try {
    const result = await invoke('inspector_request', {tool, args:{...target, ...args}});
    if (result.snapshot) {
      snapshot = result.snapshot;
      syncEmptyMoleculeDetails();
      renderShell();
      void renderViewport();
      scheduleMoleculeDetailsRefresh();
    }
    if (key === inspector.key && tool === 'reset_node_props' && result.result?.properties) {
      inspector.rows = result.result.properties;
      paintCueMolProperties();
    }
  } catch (error) {
    if (key === inspector.key) { inspector.error = String(error); paintCueMolProperties(); }
  } finally { setBusy(false); }
}

function paintCueMolProperties() {
  const inspector = cueMolPropertyInspector;
  const fields = document.getElementById('inspector-fields');
  const active = fields.contains(document.activeElement) ? document.activeElement : null;
  const focus = active?.id ? {id:active.id, dirty:active.dataset.initialValue !== undefined && active.value !== active.dataset.initialValue, initial:active.dataset.initialValue, value:active.value, start:active.selectionStart, end:active.selectionEnd} : null;
  fields.replaceChildren();
  const tabs = document.createElement('div');
  tabs.className = 'inspector-tabs';
  tabs.setAttribute('role', 'tablist');
  for (const name of ['Properties', 'Generic']) {
    const tab = document.createElement('button');
    tab.type = 'button'; tab.textContent = name;
    tab.setAttribute('role', 'tab'); tab.setAttribute('aria-selected', String(name === inspector.tab));
    tab.addEventListener('click', () => { inspector.tab = name; paintCueMolProperties(); });
    tabs.append(tab);
  }
  fields.append(tabs);
  const filter = document.createElement('input');
  filter.id = 'inspector-filter'; filter.type = 'search'; filter.placeholder = 'Filter properties'; filter.setAttribute('aria-label', 'Filter properties');
  filter.value = inspector.filter;
  filter.addEventListener('input', () => {
    inspector.filter = filter.value;
    paintCueMolPropertyRows();
  });
  fields.append(filter);
  const reset = document.createElement('button');
  reset.type = 'button'; reset.textContent = 'Reset all to default';
  reset.disabled = !inspector.rows.some(row => row.hasdefault && !row.isdefault && !row.readonly);
  reset.addEventListener('click', () => void editCueMolProperty('reset_node_props', {}));
  fields.append(reset);
  if (inspector.error) {
    const error = document.createElement('p'); error.className = 'inspector-error'; error.setAttribute('role','alert');
    error.textContent = inspector.error; fields.append(error);
  }
  const rows = document.createElement('div'); rows.className = 'inspector-property-rows';
  fields.append(rows);
  paintCueMolPropertyRows();
  if (focus) {
    const input = document.getElementById(focus.id);
    if (input && !input.disabled) {
      if (focus.dirty && (input.type === 'number' || input.dataset.initialValue === focus.initial)) input.value=focus.value;
      input.focus({preventScroll:true});
      if (focus.start !== null && ['text','search'].includes(input.type)) input.setSelectionRange(focus.start,focus.end);
    }
  }
}

function inspectorDashValue(value) {
  const number=Number(value);
  return value.trim()==='' || !Number.isFinite(number) || number<0 ? '-1' : String(number);
}

function paintCueMolPropertyRows() {
  const inspector = cueMolPropertyInspector;
  const host = document.querySelector('.inspector-property-rows');
  if (!host) return;
  host.replaceChildren();
  const filter = inspector.filter.toLowerCase();
  const structured = inspector.tab === 'Properties' && inspector.sections.length > 0;
  let rows = inspector.rows;
  if (structured) {
    rows = inspector.sections.flatMap(section => section.rows.map(definition => {
      const property = inspector.rows.find(row => row.key === definition.key);
      if (definition.kind === 'toggle') {
        return {key:definition.key, value:definition.local && inspector.localToggles[definition.key] ? 'true' : definition.display, type:'toggle', category:section.title, schema:definition};
      }
      if (definition.options) {
        const matches = definition.options.find(option => Object.entries(option.patch).every(([key, value]) => {
          const current = inspector.rows.find(row => row.key === key)?.value;
          return current === value || current !== '' && value !== '' && Math.abs(Number(current) - Number(value)) < 1e-6;
        }));
        const reset = (definition.reset || []).map(key=>inspector.rows.find(row=>row.key===key)).filter(Boolean);
        return {key:definition.key, value:definition.display ?? matches?.label ?? 'Custom', type:'preset', allowed:definition.options.map(option=>option.label), category:section.title, schema:definition, hasdefault:reset.some(row=>row.hasdefault), isdefault:reset.every(row=>row.isdefault)};
      }
      return property ? {...property, value:definition.display ?? property.value, category:section.title, schema:definition, readonly:definition.editable ? false : property.readonly} : null;
    }).filter(Boolean));
  }
  const values = new Map(rows.map(row=>[row.key,row.value]));
  rows = rows.filter(row => !filter || row.key.toLowerCase().includes(filter));
  const groups = new Map();
  for (const row of rows) {
    const group = inspector.tab === 'Properties' ? row.category || 'Properties' : '';
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group).push(row);
  }
  const order = ['Common', 'Appearance', 'Geometry'];
  const rank = group => structured ? inspector.sections.findIndex(section=>section.title===group) : group === 'Identity' ? 4 : order.includes(group) ? order.indexOf(group) : 3;
  for (const [group, entries] of [...groups].sort(([a], [b]) => rank(a)-rank(b) || a.localeCompare(b))) {
    let container = host;
    if (group && !filter) {
      const details = document.createElement('details'); details.className = 'inspector-property-group';
      details.open = structured ? inspector.sections.find(section=>section.title===group)?.expanded : order.includes(group);
      const summary = document.createElement('summary'); summary.textContent = group;
      details.append(summary); host.append(details); container = details;
    }
    for (const row of entries) {
      const line = document.createElement('div'); line.className = 'inspector-property';
      const label = document.createElement('label'); label.textContent = row.schema?.label || (inspector.tab === 'Properties' ? row.key.split('.').at(-1) : row.key);
      const id = `inspector-property-${row.key}`; label.htmlFor = id; label.title = `${row.key} (${row.type})`;
      line.append(label);
      const controls = document.createElement('div'); controls.className = 'inspector-property-input';
      const choices = row.schema?.choices || row.allowed;
      const input = document.createElement(choices?.length ? 'select' : 'input');
      input.id = id; input.disabled = row.readonly || row.schema?.disabled === true || Boolean(row.schema?.enabledWhen && (values.get(row.schema.enabledWhen) ?? inspector.rows.find(property=>property.key===row.schema.enabledWhen)?.value) !== 'true');
      input.setAttribute('aria-label', row.key === 'name' ? 'Inspected name' : `Inspected ${row.key}`);
      if (choices?.length) {
        if (row.type === 'preset' && row.value === 'Custom') { const option = document.createElement('option'); option.value='Custom'; option.textContent='Custom'; option.disabled=true; input.append(option); }
        for (const value of choices) { const option = document.createElement('option'); option.value=value; option.textContent=row.schema?.labels?.[value] || value; if (row.schema?.font) option.style.fontFamily=value; input.append(option); }
        if (row.schema?.font) input.style.fontFamily=row.value;
      } else if (row.type === 'boolean' || row.type === 'toggle') { input.type='checkbox'; input.checked=row.value === 'true'; }
      else if (row.type === 'integer' || row.type === 'real') {
        input.type='number'; input.step=row.type === 'integer' ? '1' : row.schema?.fineStep ?? (row.schema?.step ? row.schema.step/10 : 'any');
        if (row.schema?.min !== undefined) input.min=row.schema.min;
        if (row.schema?.max !== undefined) input.max=row.schema.max;
      }
      else { input.type='text'; input.placeholder=row.schema?.placeholder || ''; }
      input.value = row.value;
      if (row.schema?.blankNegative && Number(row.value)<0) input.value='';
      input.dataset.initialValue = row.value;
      const commit = value => {
        if (row.schema?.blankNegative) {
          value=inspectorDashValue(value);
          if (value===inspectorDashValue(row.value)) return;
        }
        if (value === row.value) return;
        let updates;
        if (row.type === 'toggle') {
          if (row.schema.local) inspector.localToggles[row.key] = value === 'true';
          updates = Object.entries(row.schema.patches[value]).map(([prop,value])=>({prop,value}));
          if (!updates.length) { paintCueMolPropertyRows(); return; }
        } else if (row.type === 'preset') {
          updates = Object.entries(row.schema.options.find(option=>option.label===value).patch).map(([prop,value])=>({prop,value}));
        } else {
          if (row.schema?.writes) {
            updates=row.schema.writes.map(write=>({prop:write.prop,value:String((write.inverse === undefined ? Number(value)*(write.scale ?? 1) : write.inverse/Number(value))+(write.offset ?? 0))}));
            if (updates.some(update=>!Number.isFinite(Number(update.value)))) return;
          } else updates=(row.schema?.targets || [row.key]).map(prop=>({prop,value}));
          if (value === 'true') {
            for (const [prop, value] of Object.entries(row.schema?.seedWhenEnabled || {})) {
              if (!inspector.rows.find(property=>property.key===prop)?.value) updates.push({prop,value});
            }
          }
        }
        void editCueMolProperty(updates.length === 1 ? 'set_node_prop' : 'set_node_props', {
          ...(updates.length === 1 ? updates[0] : {updates}), cascadeGroupVisibility:inspector.tab === 'Properties',
        });
      };
      if (row.schema?.optional) {
        const optional = row.schema.optional;
        const on = Number(row.value)>=0;
        if (on) inspector.optionalValues[row.key] = Number(row.value);
        const remembered = inspector.optionalValues[row.key] ?? optional.on;
        const gate = document.createElement('input'); gate.type='checkbox'; gate.checked=on; gate.disabled=input.disabled;
        gate.setAttribute('aria-label',optional.label);
        gate.addEventListener('change',()=>commit(String(gate.checked ? remembered : optional.off)));
        controls.append(gate);
        if (!on) { input.value=String(remembered); input.disabled=true; }
      }
      input.addEventListener('change', () => {
        if (input.type === 'number' && row.schema?.min !== undefined && row.schema?.max !== undefined && input.value !== '' && Number.isFinite(Number(input.value))) {
          const fine=Number(input.step);
          const value=Math.max(row.schema.min,Math.min(row.schema.max,Number(input.value)));
          input.value=String(Number.isFinite(fine) && fine>0 ? Number(value.toFixed(Math.max(0,-Math.floor(Math.log10(fine))))) : value);
        }
        if (input.validity && !input.validity.valid) { input.reportValidity(); return; }
        commit(input.type === 'checkbox' ? String(input.checked) : input.value);
      });
      input.addEventListener('keydown', event => {
        if (event.isComposing) return;
        if (event.key === 'Escape') { event.preventDefault(); input.value=row.schema?.blankNegative && Number(row.value)<0 ? '' : row.value; input.blur(); }
        else if (event.key === 'Enter') { event.preventDefault(); input.blur(); }
      });
      if (row.type === 'color' && row.rgba) {
        const color = document.createElement('input'); color.type='color'; color.disabled=input.disabled;
        color.value='#'+row.rgba.slice(0,3).map(value=>value.toString(16).padStart(2,'0')).join('');
        color.setAttribute('aria-label',`Choose color ${row.key}`);
        color.addEventListener('change',()=>{
          const rgb=[1,3,5].map(index=>parseInt(color.value.slice(index,index+2),16));
          commit(`rgba(${rgb.join(',')},${row.rgba[3]/255})`);
        }); controls.append(color);
      }
      controls.append(input);
      if (row.schema?.unit) { const unit=document.createElement('span'); unit.className='inspector-unit'; unit.textContent=row.schema.unit; controls.append(unit); }
      line.append(controls);
      if (row.hasdefault && !row.readonly) {
        const reset = document.createElement('button'); reset.type='button'; reset.textContent='↺';
        reset.disabled=row.isdefault; reset.setAttribute('aria-label',`Reset ${row.key}`);
        reset.title=`Reset to default${row.defaultValue === undefined ? '' : ` (${row.defaultValue})`}`;
        reset.addEventListener('click',()=>{ delete inspector.optionalValues[row.key]; void editCueMolProperty('reset_node_props',{props:row.schema?.reset || [row.key]}); });
        line.append(reset);
      }
      container.append(line);
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

let agentState = null;
let agentLoaded = false;
let agentConfigDirty = false;
let agentRequestPending = false;
let agentRefreshPromise = null;
let agentRefreshDirty = false;
let agentTranscriptKey = '';
let agentRecallIndex = -1;
let agentRecallDraft = '';

function agentControl(name) { return document.getElementById(`agent-${name}`); }
function agentSettings() {
  return {
    model: agentControl('model').value.trim(), reasoning: agentControl('reasoning').value,
    enter_sends: agentControl('enter-sends').checked, endpoint: agentControl('endpoint').value.trim(),
    enabled: pluginEnabled('agent'),
  };
}
async function requestAgent(operation, args = {}) {
  const result = await invoke('agent_request', {operation, args});
  agentState = result.agent;
  agentLoaded = true;
  renderAgent();
  return result;
}
function refreshAgent() {
  agentRefreshDirty = true;
  if (agentRefreshPromise) return agentRefreshPromise;
  agentRefreshPromise = (async () => {
    while (agentRefreshDirty) {
      agentRefreshDirty = false;
      await requestAgent('status');
    }
  })().catch(error => { agentControl('status').textContent = String(error); })
    .finally(() => { agentRefreshPromise = null; });
  return agentRefreshPromise;
}
async function configureAgent() {
  if (!agentConfigDirty) return;
  const settings = agentSettings();
  await requestAgent('configure', {settings});
  agentConfigDirty = false;
  renderAgent();
}
async function setAgentKey(save = false) {
  const input = agentControl('key');
  if (!input.value && !save) return;
  const key = input.value;
  input.value = '';
  await requestAgent('key', {key, save});
}
async function runAgentAction(action) {
  if (agentRequestPending) return;
  agentRequestPending = true;
  renderAgent();
  try { await action(); }
  catch (error) { agentControl('status').textContent = String(error); }
  finally { agentRequestPending = false; renderAgent(false); }
}
function agentPromptHistory() {
  try {
    const values = JSON.parse(localStorage.getItem('cuemol.agent.promptHistory') || '[]');
    return Array.isArray(values) ? values.filter(value => typeof value === 'string').slice(0, 50) : [];
  } catch { return []; }
}
function rememberAgentPrompt(prompt) {
  try { localStorage.setItem('cuemol.agent.promptHistory', JSON.stringify([prompt, ...agentPromptHistory().filter(value => value !== prompt)].slice(0, 50))); }
  catch {}
}
function submitAgent() {
  const composer = agentControl('prompt');
  const prompt = composer.value.trim();
  if (!prompt || agentState?.running || agentRequestPending || !pluginEnabled('agent')) return;
  void runAgentAction(async () => {
    await configureAgent();
    await setAgentKey();
    rememberAgentPrompt(prompt);
    agentRecallIndex = -1;
    await requestAgent('start', {prompt});
    if (agentState?.running) composer.value = '';
  });
}
function renderAgent(updateStatus = true) {
  if (!agentControl('prompt')) return;
  const running = !!agentState?.running;
  const busy = running || agentRequestPending;
  const settings = agentState?.settings;
  if (settings && !agentConfigDirty) {
    agentControl('model').value = settings.model || '';
    agentControl('reasoning').value = settings.reasoning || 'default';
    agentControl('enter-sends').checked = !!settings.enter_sends;
    agentControl('endpoint').value = settings.endpoint || '';
  }
  document.querySelectorAll('[data-agent-setting]').forEach(input => { input.disabled = busy; });
  agentControl('prompt').disabled = running;
  agentControl('send').disabled = busy || !pluginEnabled('agent') || !agentControl('prompt').value.trim();
  agentControl('cancel').disabled = !running;
  agentControl('clear').disabled = busy || !(agentState?.messages?.length);
  agentControl('models').disabled = busy || !!agentState?.refreshingModels;
  agentControl('shortcut').textContent = agentControl('enter-sends').checked
    ? 'Enter to send; Shift+Enter for a new line'
    : `${/Mac/.test(navigator.platform) ? 'Cmd' : 'Ctrl'}+Enter to send`;
  const modelList = agentControl('model-list');
  const models = agentState?.models || [];
  if (modelList.dataset.models !== JSON.stringify(models)) {
    modelList.replaceChildren(...models.map(model => { const option = document.createElement('option'); option.value = model; return option; }));
    modelList.dataset.models = JSON.stringify(models);
  }
  const messages = agentState?.messages || [];
  const key = JSON.stringify(messages);
  if (key !== agentTranscriptKey) {
    const transcript = agentControl('transcript');
    const follow = transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 45;
    const expanded = new Set([...transcript.querySelectorAll('details[open]')].map(details => Number(details.dataset.index)));
    transcript.replaceChildren(...messages.map((message,index) => {
      const entry = document.createElement('article'); entry.className = 'agent-message';
      if (message.tool) {
        const tool = message.tool;
        const details = document.createElement('details'); details.className = 'agent-tool'; details.dataset.index = index; details.open = expanded.has(index);
        const summary = document.createElement('summary'); summary.textContent = `${tool.name} · ${tool.ok == null ? 'running...' : tool.ok ? 'ok' : 'failed'}`;
        details.append(summary);
        for (const [label,text] of [['Input',tool.input],['Result',tool.result]]) {
          if (text == null) continue;
          const title = document.createElement('strong'); title.textContent = label;
          const body = document.createElement('pre'); body.textContent = text;
          details.append(title,body);
        }
        entry.append(details);
      } else {
        const role = document.createElement('strong'); role.textContent = message.role;
        const body = document.createElement('div'); body.className = 'agent-message-text'; body.textContent = message.text;
        entry.append(role, body);
      }
      if (message.image) { const image = document.createElement('img'); image.alt = 'Agent render result'; image.src = `data:image/png;base64,${message.image}`; entry.append(image); }
      return entry;
    }));
    if (!messages.length) {
      const empty = document.createElement('p'); empty.className = 'agent-empty'; empty.textContent = 'Describe what you want to see. For example: load 1CRN and show it as a cartoon.';
      transcript.append(empty);
    }
    agentTranscriptKey = key;
    if (follow) transcript.scrollTop = transcript.scrollHeight;
  }
  if (updateStatus) agentControl('status').textContent = agentState?.message || '';
}
function setupAgent() {
  for (const control of document.querySelectorAll('[data-agent-setting]')) {
    if (control.id === 'agent-key') continue;
    control.addEventListener('input', () => { agentConfigDirty = true; renderAgent(); });
  }
  agentControl('model').addEventListener('change', () => { agentControl('key').value = ''; });
  agentControl('send').addEventListener('click', submitAgent);
  agentControl('cancel').addEventListener('click', () => { void runAgentAction(() => requestAgent('cancel')); });
  agentControl('clear').addEventListener('click', () => { void runAgentAction(() => requestAgent('clear')); });
  agentControl('apply').addEventListener('click', () => { void runAgentAction(configureAgent); });
  agentControl('models').addEventListener('click', () => { void runAgentAction(async () => { await configureAgent(); await setAgentKey(); await requestAgent('models'); }); });
  agentControl('save-key').addEventListener('click', () => { void runAgentAction(async () => { await configureAgent(); await setAgentKey(true); }); });
  agentControl('delete-key').addEventListener('click', () => { void runAgentAction(async () => { await configureAgent(); await requestAgent('delete_key'); }); });
  const composer = agentControl('prompt');
  composer.addEventListener('input', () => { agentRecallIndex = -1; renderAgent(); });
  composer.addEventListener('keydown', event => {
    if (event.isComposing || event.keyCode === 229) return;
    if (event.key === 'Enter' && !event.shiftKey && !event.altKey &&
        (agentControl('enter-sends').checked ? !event.ctrlKey && !event.metaKey : event.ctrlKey || event.metaKey)) {
      event.preventDefault(); submitAgent(); return;
    }
    if (!['ArrowUp', 'ArrowDown'].includes(event.key) || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (composer.selectionStart !== composer.selectionEnd) return;
    const first = !composer.value.slice(0, composer.selectionStart).includes('\n');
    const last = !composer.value.slice(composer.selectionEnd).includes('\n');
    const history = agentPromptHistory();
    if (event.key === 'ArrowUp' && first && agentRecallIndex + 1 < history.length) {
      if (agentRecallIndex < 0) agentRecallDraft = composer.value;
      composer.value = history[++agentRecallIndex];
    } else if (event.key === 'ArrowDown' && last && agentRecallIndex >= 0) {
      composer.value = --agentRecallIndex < 0 ? agentRecallDraft : history[agentRecallIndex];
    } else return;
    event.preventDefault();
    composer.setSelectionRange(composer.value.length, composer.value.length);
    renderAgent();
  });
  renderAgent();
}

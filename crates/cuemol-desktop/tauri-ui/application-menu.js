"use strict";

const CueMolApplicationMenu = (() => {
  const textActions = new Set(['cut', 'copy', 'paste', 'select-all', 'undo', 'redo']);
  let enabled = false, nativeDialogs = 0, stateProvider, actionHandler, mainWindow = false;
  let lastState = '', queued = false, pending = Promise.resolve();
  const request = args => window.__TAURI__.core.invoke('application_menu_request', args);
  const modalOpen = () => Boolean(document.querySelector('dialog:modal'));
  const blocked = () => nativeDialogs > 0 || modalOpen();

  function sync() {
    if (!enabled || queued) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      const args = mainWindow
        ? {operation: 'state', state: {...stateProvider(), blocked: modalOpen()}}
        : {operation: 'blocked', blocked: modalOpen()};
      const value = JSON.stringify(args);
      if (value === lastState) return;
      lastState = value;
      pending = pending.then(() => request(args)).catch(error => { lastState = ''; console.error(error); });
    });
  }

  async function nativeDialog(action) {
    if (!enabled) return action();
    ++nativeDialogs;
    try {
      await pending;
      await request({operation: 'native-dialog', blocked: true});
      return await action();
    } finally {
      --nativeDialogs;
      await request({operation: 'native-dialog', blocked: nativeDialogs > 0});
      sync();
    }
  }

  async function dispatch(id) {
    const text = textActions.has(id);
    const field = clipboardTextContext();
    if (blocked() && (!text || !field)) return;
    try {
      if (text && field) await textClipboardAction(id, field);
      else await actionHandler(id);
    } finally { sync(); }
  }

  async function start({main = false, state = () => ({}), action}) {
    if (!window.__TAURI_INTERNALS__ || !window.__TAURI__?.event?.listen) return false;
    if (!await request({operation: 'supported'})) return false;
    await request({operation: 'clear'});
    stateProvider = state;
    actionHandler = action;
    mainWindow = main;
    await window.__TAURI__.event.listen('cuemol-menu-action', event => {
      void dispatch(event.payload).catch(error => typeof showError === 'function' ? showError(error) : showWindowError(error));
    });
    enabled = true;
    const dialog = window.__TAURI__.dialog;
    for (const name of ['open', 'save', 'confirm', 'ask', 'message']) {
      const original = dialog?.[name];
      if (typeof original === 'function') dialog[name] = (...args) => nativeDialog(() => original(...args));
    }
    new MutationObserver(sync).observe(document.body, {subtree: true, attributes: true, attributeFilter: ['open']});
    if (main) document.body.classList.add('native-application-menu');
    sync();
    return true;
  }
  return {start, sync, nativeDialog};
})();

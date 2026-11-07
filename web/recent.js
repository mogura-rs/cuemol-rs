export function initRecentFiles({open, notify, error, fallback}) {
  const limit = 20;
  const handles = new WeakMap();
  const candidates = new WeakSet();
  let stored = new Set();
  let entries = [];
  let database;
  const publish = () => notify(JSON.stringify(entries.map(({id, name, handle}) => ({
    id, name, persistent: !!handle && stored.has(id),
  }))));
  const ready = new Promise(resolve => {
    try {
      const request = indexedDB.open('cuemol-recent-files', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('files');
      request.onerror = () => resolve();
      request.onblocked = () => { request.onsuccess = () => request.result.close(); resolve(); };
      request.onsuccess = () => {
        database = request.result;
        database.onversionchange = () => database.close();
        const read = database.transaction('files').objectStore('files').get('recent');
        read.onerror = () => resolve();
        read.onsuccess = () => {
          entries = (Array.isArray(read.result) ? read.result : [])
            .filter(entry => typeof entry.id === 'string' && typeof entry.name === 'string'
              && entry.handle?.kind === 'file')
            .slice(0, limit);
          stored = new Set(entries.map(entry => entry.id));
          publish();
          resolve();
        };
      };
    } catch { resolve(); }
  });
  let pending = ready;
  function serial(operation) {
    pending = pending.then(operation).catch(reason => error(reason?.message || String(reason)));
    return pending;
  }
  async function persist() {
    if (!database) { publish(); return; }
    try {
      await new Promise((resolve, reject) => {
        const transaction = database.transaction('files', 'readwrite');
        transaction.objectStore('files').put(entries.filter(entry => entry.handle)
          .map(({id, name, handle}) => ({id, name, handle})), 'recent');
        transaction.oncomplete = resolve;
        transaction.onerror = () => reject(transaction.error);
        transaction.onabort = () => reject(transaction.error || new Error('Recent file storage aborted'));
      });
      stored = new Set(entries.filter(entry => entry.handle).map(entry => entry.id));
    } finally { publish(); }
  }
  function report(reason) {
    if (reason?.name !== 'AbortError') error(reason?.message || String(reason));
  }
  const api = {
    register(file) { candidates.add(file); },
    choose() {
      if (!globalThis.showOpenFilePicker) { fallback(); return; }
      try {
        const selection = showOpenFilePicker({id: 'cuemol-files', multiple: true});
        Promise.resolve(selection).then(async selected => {
          const files = await Promise.all(selected.map(async handle => {
            const file = await handle.getFile();
            handles.set(file, handle);
            return file;
          }));
          open(files);
        }).catch(report);
      } catch (reason) { report(reason); }
    },
    reopen(id) {
      const entry = entries.find(entry => entry.id === id);
      if (!entry) return;
      (async () => {
        if (!entry.handle) { open([entry.file]); return; }
        const permission = {mode: 'read'};
        if (await entry.handle.queryPermission(permission) !== 'granted'
          && await entry.handle.requestPermission(permission) !== 'granted') {
          throw new Error(`Read permission was not granted for ${entry.name}.`);
        }
        const file = await entry.handle.getFile();
        handles.set(file, entry.handle);
        open([file]);
      })().catch(report);
    },
    remember(file) {
      if (!candidates.has(file)) return;
      return serial(async () => {
        const handle = handles.get(file);
        let index = entries.findIndex(entry => entry.file === file);
        if (index < 0 && handle) {
          for (let i = 0; i < entries.length; i++) {
            if (!entries[i].handle) continue;
            try {
              if (await handle.isSameEntry(entries[i].handle)) { index = i; break; }
            } catch { /* Removed handles must not prevent adding a valid file. */ }
          }
        }
        const id = index < 0 ? crypto.randomUUID() : entries.splice(index, 1)[0].id;
        entries.unshift({id, name: file.name, handle, file: handle ? undefined : file});
        entries.length = Math.min(entries.length, limit);
        await persist();
      });
    },
    clear() {
      return serial(async () => { entries = []; await persist(); });
    },
  };
  globalThis.cuemolFiles = api;
  return api;
}

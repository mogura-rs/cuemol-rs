export async function initLoader() {
  const response = await fetch(new URL('./pkg/cuemol_web_loader_bg.wasm', import.meta.url));
  if (!response.ok) throw new Error(`Cannot load the CueMol runtime: HTTP ${response.status}`);
  let module;
  if (typeof WebAssembly.compileStreaming === 'function'
      && response.headers.get('Content-Type')?.split(';')[0].trim() === 'application/wasm') {
    module = await WebAssembly.compileStreaming(response);
  } else {
    module = await WebAssembly.compile(await response.arrayBuffer());
  }
  let current;
  const cancelled = () => new DOMException('File loading cancelled.', 'AbortError');
  const retire = (state, error = cancelled()) => {
    clearTimeout(state.timer);
    state.worker.terminate();
    if (current === state) current = undefined;
    state.pending?.reject(error);
    state.pending = undefined;
  };
  const idle = state => {
    clearTimeout(state.timer);
    state.timer = setTimeout(() => retire(state), 2000);
  };
  const acquire = () => {
    if (current) return current;
    const state = {worker: new Worker(new URL('./loader-worker.js', import.meta.url), {type: 'module'})};
    current = state;
    state.worker.onmessage = ({data}) => {
      if (current !== state) return;
      if (data.error) { retire(state, new Error(data.error)); return; }
      if (data.ready) { if (!state.pending) idle(state); return; }
      if (!(data.bytes instanceof Uint8Array)) { retire(state, new Error('Invalid file-loader response.')); return; }
      loader.last = data.metrics;
      state.pending?.resolve(data.bytes);
      state.pending = undefined;
      idle(state);
    };
    state.worker.onerror = event => {
      event.preventDefault();
      retire(state, new Error(event.message || 'File loader failed.'));
    };
    state.worker.onmessageerror = () => retire(state, new Error('Cannot receive the parsed file.'));
    try { state.worker.postMessage({module}); } catch (error) { retire(state, error); throw error; }
    return state;
  };
  const loader = {
    last: undefined,
    parseFile(file) {
      if (current?.pending) return Promise.reject(new Error('A file is already being loaded.'));
      return new Promise((resolve, reject) => {
        const state = acquire();
        clearTimeout(state.timer);
        state.pending = {resolve, reject};
        try { state.worker.postMessage({file}); } catch (error) { retire(state, error); }
      });
    },
    cancel() { if (current) retire(current); },
  };
  globalThis.cuemolLoader = loader;
  acquire();
  return module;
}

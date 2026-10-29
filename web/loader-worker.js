import init, {prepare_file} from './pkg/cuemol_web_loader.js';

let initialized;
self.onmessage = async ({data: {module, file}}) => {
  try {
    if (module) {
      initialized = init({module_or_path: module});
      await initialized;
      self.postMessage({ready: true});
      return;
    }
    const runtime = await initialized;
    const input = new Uint8Array(await file.arrayBuffer());
    const started = performance.now();
    const bytes = prepare_file(file.name, input);
    const metrics = {inputBytes: input.byteLength, outputBytes: bytes.byteLength,
      parseMs: performance.now() - started, memoryBytes: runtime.memory.buffer.byteLength};
    self.postMessage({bytes, metrics}, [bytes.buffer]);
  } catch (error) {
    self.postMessage({error: error instanceof Error ? error.message : String(error)});
  }
};

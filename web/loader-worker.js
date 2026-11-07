import init, {prepare_file, prepare_pdb_file, prepare_file_with_options,
  decode_file_prefix, inspect_file, inspect_compressed_cif, inspect_compressed_mtz, mtz_header_offset} from './pkg/cuemol_web_loader.js';

let initialized;
async function inspect(file) {
  const plain = file.name.toLowerCase().replace(/\.(gz|bz2|xz)$/, '');
  const compressed = plain !== file.name.toLowerCase();
  const cif = /\.(cif|mmcif)$/.test(plain);
  const limit = cif ? 65536 : 1024;
  let inputBytes = 0;
  const read = async (start, end) => {
    const bytes = new Uint8Array(await file.slice(start, end).arrayBuffer());
    inputBytes += bytes.length;
    return bytes;
  };
  if (compressed && /\.mtz$/.test(plain)) {
    const info = inspect_compressed_mtz(file.name, await read(0, file.size));
    return {info, metrics: {inputBytes, metadata: true}};
  }
  if (compressed && cif) {
    for (let size = Math.min(16384, file.size);; size = Math.min(size * 2, file.size)) {
      try {
        const info = inspect_compressed_cif(file.name, await read(0, size));
        return {info, metrics: {inputBytes, metadata: true}};
      } catch (error) { if (size === file.size) throw error; }
    }
  }
  let prefix;
  if (compressed) {
    for (let size = Math.min(16384, file.size);; size = Math.min(size * 2, file.size)) {
      try { prefix = decode_file_prefix(await read(0, size), limit); break; }
      catch (error) { if (size === file.size) throw error; }
    }
  } else {
    prefix = await read(0, limit);
  }
  let header = new Uint8Array();
  let fileSize = file.size;
  if (/\.mtz$/.test(plain)) {
    header = await read(mtz_header_offset(prefix, fileSize), fileSize);
  } else if (cif && !compressed) {
    // Find the first structural/reflection table without retaining the file body.
    const decoder = new TextDecoder();
    for (let offset = 0;; offset += limit - 64) {
      const text = decoder.decode(prefix);
      if (/_atom_site\.|_refln\.pdbx_(?:FWT|DELFWT|PHWT|DELPHWT)/.test(text) || offset + prefix.length >= file.size) break;
      prefix = await read(offset + limit - 64, offset + limit * 2 - 64);
    }
  }
  return {info: inspect_file(file.name, prefix, header, fileSize), metrics: {inputBytes, metadata: true}};
}
self.onmessage = async ({data: {module, file, pdbKind, operation, options, companion}}) => {
  try {
    if (module) {
      initialized = init({module_or_path: module});
      await initialized;
      self.postMessage({ready: true});
      return;
    }
    const runtime = await initialized;
    if (operation === 'inspect') {
      const result = await inspect(file);
      result.metrics.memoryBytes = runtime.memory.buffer.byteLength;
      self.postMessage(result);
      return;
    }
    const input = new Uint8Array(await file.arrayBuffer());
    const extra = companion ? new Uint8Array(await companion.arrayBuffer()) : new Uint8Array();
    const started = performance.now();
    const bytes = options
      ? prepare_file_with_options(file.name, input, options, extra, !!companion)
      : pdbKind ? prepare_pdb_file(file.name, input, pdbKind) : prepare_file(file.name, input);
    const metrics = {inputBytes: input.byteLength + extra.byteLength, outputBytes: bytes.byteLength,
      parseMs: performance.now() - started, memoryBytes: runtime.memory.buffer.byteLength};
    self.postMessage({bytes, metrics}, [bytes.buffer]);
  } catch (error) {
    self.postMessage({error: error instanceof Error ? error.message : String(error)});
  }
};

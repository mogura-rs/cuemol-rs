const TEMPORARY = 'Browser temporary folder';
const MARKER = '.cuemol-session.json';
const DAY = 86400000;
const directories = new Map();
const jobs = new Map();
let initialized;
let database;
let temporary;
let muxer;

function failure(message) { return new Error(message); }
function cancelled() { return new DOMException('Movie output cancelled', 'AbortError'); }
function baseName(value) {
  const name = value.trim() || 'movie';
  if (name === '.' || name === '..' || /[/\\:*?"<>|%\x00-\x1f\x7f-\x9f]/.test(name)) {
    throw failure('Base name must be a filename without separators or percent signs');
  }
  return name;
}
function frameName(base, frame) { return `${base}_frm_${String(frame).padStart(4, '0')}.png`; }
function isFrame(name, base) {
  const prefix = `${base}_frm_`;
  return name.startsWith(prefix) && /^\d{4,}\.png$/.test(name.slice(prefix.length));
}
function isMovie(name, base) { return ['mov', 'mp4', 'wmv', 'gif'].some(ext => name === `${base}.${ext}`); }
function ownedFile(name) { return /_frm_\d{4,}\.png$/.test(name) || /\.(mov|mp4|wmv|gif)$/.test(name); }
function validate(options, size) {
  baseName(options.baseName);
  if (!Number.isFinite(options.fps) || options.fps <= 0) throw failure('Frame rate must be positive');
  if (!Number.isInteger(options.bitrateKbps) || options.bitrateKbps < 1) throw failure('Bitrate must be positive');
  if (!Array.isArray(size) || size.length !== 2 || size.some(n => !Number.isInteger(n) || n < 1 || n > 8192)
      || size[0] * size[1] > 32000000) throw failure('Invalid movie image size');
}
async function optionalFile(directory, name) {
  try { return await directory.getFileHandle(name); }
  catch (error) { if (error.name === 'NotFoundError') return null; throw error; }
}
async function writeAtomic(handle, data) {
  const writer = await handle.createWritable();
  try { await writer.write(data); await writer.close(); }
  catch (error) { await writer.abort().catch(() => {}); throw error; }
}
async function lease(name) {
  let ready, reject, release;
  const started = new Promise((resolve, fail) => { ready = resolve; reject = fail; });
  const held = new Promise(resolve => { release = resolve; });
  const task = navigator.locks.request(name, {ifAvailable: true}, async lock => {
    if (!lock) { reject(failure('These movie outputs are in use in another window')); return; }
    ready(); await held;
  }).catch(reject);
  await started;
  return async () => { release(); await task; };
}
function registry() {
  if (!database) database = new Promise((resolve, reject) => {
    const request = indexedDB.open('cuemol-movie-folders', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('folders', {keyPath: 'id'});
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  return database;
}
async function folderRecords() {
  const db = await registry();
  return new Promise((resolve, reject) => {
    const request = db.transaction('folders').objectStore('folders').getAll();
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
async function storeFolder(record) {
  const db = await registry();
  return new Promise((resolve, reject) => {
    const tx = db.transaction('folders', 'readwrite');
    tx.objectStore('folders').put(record);
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error || failure('Could not remember the movie folder'));
  });
}
async function sweep(root) {
  const old = [];
  for await (const [name, directory] of root.entries()) {
    if (directory.kind !== 'directory' || !/^session-[a-z0-9-]+$/.test(name)) continue;
    await navigator.locks.request(`cuemol-movie-session:${name}`, {ifAvailable: true}, async lock => {
      if (!lock) return;
      try {
        const marker = await directory.getFileHandle(MARKER);
        const metadata = JSON.parse(await (await marker.getFile()).text());
        if (metadata.version !== 1 || metadata.application !== 'cuemol' || metadata.session !== name) return;
        const files = [];
        for await (const [filename, handle] of directory.entries()) {
          if (filename === MARKER) continue;
          if (handle.kind !== 'file' || !ownedFile(filename)) return;
          const file = await handle.getFile();
          files.push({name: filename, modified: file.lastModified, movie: /\.(mov|mp4|wmv|gif)$/.test(filename)});
        }
        const now = Date.now();
        const remaining = [];
        for (const file of files) {
          if (now - file.modified > (file.movie ? 30 : 1) * DAY) await directory.removeEntry(file.name);
          else remaining.push(file);
        }
        if (!remaining.length && now - metadata.created > DAY) await root.removeEntry(name, {recursive: true});
        else if (remaining.some(file => file.movie)) old.push({name, modified: remaining.reduce((last, file) => Math.max(last, file.modified), 0)});
      } catch (error) { console.debug('Movie session cleanup skipped', error.name); }
    });
  }
  old.sort((a, b) => b.modified - a.modified);
  for (const {name} of old.slice(10)) {
    await navigator.locks.request(`cuemol-movie-session:${name}`, {ifAvailable: true}, async lock => {
      if (!lock) return;
      try {
        const directory = await root.getDirectoryHandle(name);
        for await (const [filename, handle] of directory.entries()) {
          if (filename !== MARKER && (handle.kind !== 'file' || !ownedFile(filename))) return;
        }
        await root.removeEntry(name, {recursive: true});
      } catch (error) { console.debug('Movie session retention skipped', error.name); }
    });
  }
}
async function initialize() {
  if (!initialized) initialized = (async () => {
    if (!navigator.storage?.getDirectory || !navigator.locks) {
      throw failure('Movie output requires browser file storage in a secure context');
    }
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle('cuemol-movies', {create: true});
    const name = `session-${crypto.randomUUID()}`;
    const release = await lease(`cuemol-movie-session:${name}`);
    try {
      const handle = await root.getDirectoryHandle(name, {create: true});
      await writeAtomic(await handle.getFileHandle(MARKER, {create: true}), JSON.stringify({
        version: 1, application: 'cuemol', session: name, created: Date.now(),
      }));
      temporary = {id: name, label: TEMPORARY, handle, release};
      directories.set(name, temporary);
      for (const record of await folderRecords()) directories.set(record.id, record);
      await sweep(root);
      return {temporary: TEMPORARY, directory: name, canPickFolder: typeof showDirectoryPicker === 'function'};
    } catch (error) { await release(); throw error; }
  })().catch(error => { initialized = null; throw error; });
  return initialized;
}
async function pickDirectory() {
  if (typeof showDirectoryPicker !== 'function') throw failure('This browser does not support choosing an output folder');
  let handle;
  try { handle = await showDirectoryPicker({id: 'cuemol-movies', mode: 'readwrite'}); }
  catch (error) { if (error.name === 'AbortError') return null; throw error; }
  await initialize();
  return navigator.locks.request('cuemol-movie-folder-registry', async () => {
    const records = await folderRecords();
    for (const record of records) {
      if (await handle.isSameEntry(record.handle)) {
        directories.set(record.id, {...record, handle});
        return {label: record.label, directory: record.id};
      }
    }
    let label = handle.name;
    for (let suffix = 2; label === TEMPORARY || records.some(record => record.label === label); suffix++) {
      label = `${handle.name} (${suffix})`;
    }
    const record = {id: `folder-${crypto.randomUUID()}`, label, handle};
    await storeFolder(record);
    directories.set(record.id, record);
    return {label, directory: record.id};
  });
}
async function directoryFor(options, requestPermission = false) {
  await initialize();
  if (options.useTempDir) return temporary;
  const directory = [...directories.values()].find(record => record.label === options.outputDir);
  if (!directory) throw failure('Choose a movie output folder');
  if (directory.handle.queryPermission) {
    let permission = await directory.handle.queryPermission({mode: 'readwrite'});
    if (permission !== 'granted' && requestPermission) permission = await directory.handle.requestPermission({mode: 'readwrite'});
    if (permission !== 'granted') throw failure('Choose the output folder again to grant access');
  }
  return directory;
}
async function inspectDirectory(directory, base) {
  let frameCount = 0, hasOutputs = false;
  while (await optionalFile(directory.handle, frameName(base, frameCount))) frameCount++;
  for await (const [name, handle] of directory.handle.entries()) {
    if (handle.kind === 'file' && (isFrame(name, base) || isMovie(name, base))) { hasOutputs = true; break; }
  }
  return {directory: directory.id, label: directory.label, baseName: base, frameCount, hasOutputs};
}
async function inspect(options) { return inspectDirectory(await directoryFor(options), baseName(options.baseName)); }
async function removeOutputs(directory, base) {
  for await (const [name, handle] of directory.handle.entries()) {
    if (handle.kind === 'file' && (isFrame(name, base) || isMovie(name, base))) await directory.handle.removeEntry(name);
  }
}
async function cleanup(options) {
  const directory = await directoryFor(options, true), base = baseName(options.baseName);
  const release = await lease(`cuemol-movie-output:${directory.id}:${base}`);
  try { await removeOutputs(directory, base); return {directory: directory.id, baseName: base}; }
  finally { await release(); }
}
function library() { return muxer ||= import('./pkg/movie/mediabunny.js'); }
async function encodingConfig(options, size) {
  const match = /^(mp4|mov)_h(264|265)$/.exec(options.movieFormat);
  if (!match) throw failure('This movie format requires the native application; disable Encode movie to save PNG frames');
  const lib = await library();
  const codec = match[2] === '264' ? 'avc' : 'hevc';
  const quality = new lib.Quality({bitrate: options.bitrateKbps * 1000});
  if (!await lib.canEncodeVideo(codec, {width: size[0], height: size[1], frameRate: options.fps, quality})) {
    throw failure(`H.${match[2]} encoding is unavailable for this size in this browser; choose another format or disable Encode movie`);
  }
  return {lib, codec, quality, extension: match[1]};
}
async function begin(options, size, overwrite, reencode = false, signal = null) {
  signal?.throwIfAborted();
  validate(options, size);
  const directory = await directoryFor(options, true), base = baseName(options.baseName);
  const release = await lease(`cuemol-movie-output:${directory.id}:${base}`);
  try {
    const info = await inspectDirectory(directory, base);
    if (reencode && !info.frameCount) throw failure('No contiguous movie frames are available');
    if (!reencode && info.hasOutputs && !overwrite) throw failure('Confirm replacement of existing movie outputs');
    if (reencode) {
      const first = await directory.handle.getFileHandle(frameName(base, 0));
      const bitmap = await createImageBitmap(await first.getFile());
      size = [bitmap.width, bitmap.height];
      bitmap.close();
      validate(options, size);
    }
    const encoding = (options.makeMovie || reencode) ? await encodingConfig(options, size) : null;
    signal?.throwIfAborted();
    if (!reencode) await removeOutputs(directory, base);
    const job = {id: crypto.randomUUID(), directory, base, options: structuredClone(options), size,
      encoding, release, frameCount: reencode ? info.frameCount : 0, encoded: 0, moviePath: null,
      cancelled: false, encodingOutput: null, started: performance.now(), phase: reencode ? 'encoding' : 'rendering'};
    jobs.set(job.id, job);
    job.signal = signal;
    job.onAbort = () => { if (jobs.has(job.id)) cancel(job.id); };
    signal?.addEventListener('abort', job.onAbort, {once: true});
    if (signal?.aborted) job.onAbort();
    return {job: job.id, ...info, frameCount: job.frameCount, size};
  } catch (error) { await release(); throw error; }
}
function getJob(id) { const job = jobs.get(id); if (!job) throw failure('Movie job is no longer available'); return job; }
async function writeFrame(id, index, bytes) {
  const job = getJob(id);
  if (job.cancelled) throw cancelled();
  if (index !== job.frameCount) throw failure('Movie frames must be written sequentially');
  const name = frameName(job.base, index);
  const previous = await optionalFile(job.directory.handle, name);
  const handle = previous || await job.directory.handle.getFileHandle(name, {create: true});
  try { await writeAtomic(handle, bytes); }
  catch (error) { if (!previous) await job.directory.handle.removeEntry(name).catch(() => {}); throw error; }
  job.frameCount++;
  return job.frameCount;
}
async function readFrame(directoryId, base, frame) {
  await initialize();
  const directory = directories.get(directoryId);
  if (!directory || !Number.isInteger(frame) || frame < 0) throw failure('Movie frame is no longer available');
  const handle = await directory.handle.getFileHandle(frameName(baseName(base), frame));
  return new Uint8Array(await (await handle.getFile()).arrayBuffer());
}
async function encode(id) {
  const job = getJob(id);
  if (job.cancelled) throw cancelled();
  if (!job.frameCount) throw failure('No movie frames are available');
  const {lib, codec, quality, extension} = job.encoding || await encodingConfig(job.options, job.size);
  const name = `${job.base}.${extension}`;
  const previous = await optionalFile(job.directory.handle, name);
  const handle = previous || await job.directory.handle.getFileHandle(name, {create: true});
  const writer = await handle.createWritable();
  let output;
  try {
    const target = new lib.StreamTarget(new WritableStream({write: ({position, data}) => writer.write({type: 'write', position, data})}),
      {chunked: true, chunkSize: 65536});
    output = new lib.Output({format: extension === 'mp4' ? new lib.Mp4OutputFormat({fastStart: false}) : new lib.MovOutputFormat({fastStart: false}), target});
    const canvas = new OffscreenCanvas(...job.size), context = canvas.getContext('2d', {alpha: false});
    const source = new lib.CanvasSource(canvas, {codec, quality});
    output.addVideoTrack(source, {frameRate: job.options.fps});
    job.phase = 'encoding'; job.encodingOutput = output;
    await output.start();
    for (let index = 0; index < job.frameCount; index++) {
      if (job.cancelled) throw cancelled();
      const frame = await job.directory.handle.getFileHandle(frameName(job.base, index));
      const bitmap = await createImageBitmap(await frame.getFile());
      try {
        if (bitmap.width !== job.size[0] || bitmap.height !== job.size[1]) throw failure('Movie frame dimensions do not match');
        context.drawImage(bitmap, 0, 0);
      } finally { bitmap.close(); }
      await source.add(index / job.options.fps, 1 / job.options.fps);
      job.encoded = index + 1;
    }
    source.close();
    if (job.cancelled) throw cancelled();
    await output.finalize();
    if (job.cancelled) throw cancelled();
    await writer.close();
    job.moviePath = `${job.directory.id}/${name}`;
    return job.moviePath;
  } catch (error) {
    if (output) await output.cancel().catch(() => {});
    await writer.abort().catch(() => {});
    if (!previous) await job.directory.handle.removeEntry(name).catch(() => {});
    throw job.cancelled ? cancelled() : error;
  } finally { job.encodingOutput = null; }
}
function cancel(id) {
  const job = getJob(id);
  job.cancelled = true;
  job.encodingOutput?.cancel().catch(() => {});
}
function status(id) {
  const job = getJob(id);
  return {frameCount: job.frameCount, encoded: job.encoded, phase: job.phase, cancelled: job.cancelled};
}
async function finish(id) {
  const job = getJob(id);
  if (job.encodingOutput) throw failure('Wait for encoding to stop before releasing movie outputs');
  job.signal?.removeEventListener('abort', job.onAbort);
  jobs.delete(id);
  await job.release();
  return {outputDir: job.directory.id, baseName: job.base, frameCount: job.frameCount,
    framesAvailable: true, fps: job.options.fps, moviePath: job.moviePath, options: job.options};
}
async function movieFile(path) {
  await initialize();
  const split = path.indexOf('/'), directory = directories.get(path.slice(0, split)), name = path.slice(split + 1);
  if (!directory || !name || name.includes('/') || name.includes('\\')) throw failure('Movie is no longer available');
  const file = await (await directory.handle.getFileHandle(name)).getFile();
  const type = name.endsWith('.mov') ? 'video/quicktime' : name.endsWith('.mp4') ? 'video/mp4'
    : name.endsWith('.gif') ? 'image/gif' : 'video/x-ms-wmv';
  return new File([file], name, {type, lastModified: file.lastModified});
}
function downloadFile(file) {
  const url = URL.createObjectURL(file), anchor = document.createElement('a');
  anchor.href = url; anchor.download = file.name; anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 60000);
}
async function saveMovie(path) {
  let destination;
  if (typeof showSaveFilePicker === 'function') {
    try { destination = await showSaveFilePicker({suggestedName: path.split('/').at(-1)}); }
    catch (error) { if (error.name === 'AbortError') return; throw error; }
  }
  const file = await movieFile(path);
  if (!destination) { downloadFile(file); return; }
  const writer = await destination.createWritable();
  try { await file.stream().pipeTo(writer); }
  catch (error) { await writer.abort().catch(() => {}); throw error; }
}
async function openMovie(path) {
  const viewer = window.open('about:blank', '_blank');
  if (!viewer) throw failure('Allow popups to open the movie');
  try {
    viewer.opener = null;
    const url = URL.createObjectURL(await movieFile(path));
    viewer.location.href = url;
    const timer = setInterval(() => { if (viewer.closed) { URL.revokeObjectURL(url); clearInterval(timer); } }, 1000);
  } catch (error) { viewer.close(); throw error; }
}

globalThis.cuemolMovies = {initialize, pickDirectory, inspect, cleanup, begin, writeFrame,
  readFrame, encode, cancel, status, finish, saveMovie, openMovie};

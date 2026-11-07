import {test, expect} from '@playwright/test';

const url = process.env.CUEMOL_WEB_TEST_URL || 'http://127.0.0.1:8097';
async function ready(page) {
  await page.route('**/movie-output-test', route => route.fulfill({contentType: 'text/html', body: '<!doctype html><title>Movie output</title>'}));
  await page.goto(`${url}/movie-output-test`);
  await page.evaluate(async () => {
    await import('/movie.js');
    globalThis.movieOptions = {useTempDir: true, outputDir: '', baseName: 'movie', fps: 29.97,
      makeMovie: true, movieFormat: 'mp4_h264', dupLastFrame: true, bitrateKbps: 1024};
    globalThis.frameBytes = async color => {
      const canvas = new OffscreenCanvas(64, 48), context = canvas.getContext('2d');
      context.fillStyle = color; context.fillRect(0, 0, 64, 48);
      return new Uint8Array(await (await canvas.convertToBlob({type: 'image/png'})).arrayBuffer());
    };
    globalThis.outputFile = async (directory, name) => {
      const root = await navigator.storage.getDirectory();
      const folder = directory.startsWith('session-')
        ? await (await root.getDirectoryHandle('cuemol-movies')).getDirectoryHandle(directory)
        : await root.getDirectoryHandle('custom-test');
      return (await folder.getFileHandle(name)).getFile();
    };
    await cuemolMovies.initialize();
  });
}

test('browser movie files preserve frames, fractional rates and previous encodings on cancellation', async ({page}) => {
  test.setTimeout(60000);
  await ready(page);
  const result = await page.evaluate(async () => {
    const outputs = [];
    for (const format of ['mp4_h264', 'mov_h264']) {
      const options = {...movieOptions, baseName: format, movieFormat: format};
      const {job, directory} = await cuemolMovies.begin(options, [64, 48], false);
      const bytes = await Promise.all(['red', 'green', 'blue'].map(frameBytes));
      for (let frame = 0; frame < bytes.length; frame++) await cuemolMovies.writeFrame(job, frame, bytes[frame]);
      const encoded = await cuemolMovies.encode(job);
      const movie = await cuemolMovies.finish(job);
      const file = await outputFile(directory, encoded.split('/').at(-1));
      const previous = Array.from(new Uint8Array(await file.arrayBuffer()));
      const frames = [];
      for (let frame = 0; frame < 3; frame++) frames.push(Array.from(await cuemolMovies.readFrame(directory, options.baseName, frame)));
      const reencode = await cuemolMovies.begin({...options, fps: 12.5}, [16, 16], false, true);
      const encoding = cuemolMovies.encode(reencode.job).then(() => null, error => error.name);
      cuemolMovies.cancel(reencode.job);
      const error = await encoding;
      await cuemolMovies.finish(reencode.job);
      const after = Array.from(new Uint8Array(await (await outputFile(directory, file.name)).arrayBuffer()));
      const video = document.createElement('video');
      const loaded = new Promise((resolve, reject) => { video.onloadedmetadata = resolve; video.onerror = reject; });
      const objectUrl = URL.createObjectURL(file); video.src = objectUrl; await loaded;
      const duration = video.duration, size = [video.videoWidth, video.videoHeight];
      video.removeAttribute('src'); video.load(); URL.revokeObjectURL(objectUrl);
      outputs.push({movie, error, reencodeSize: reencode.size, previous, after,
        framesEqual: frames.every((frame, index) => JSON.stringify(frame) === JSON.stringify(Array.from(bytes[index]))), duration, size});
    }
    return outputs;
  });
  for (const output of result) {
    expect(output.movie).toMatchObject({frameCount: 3, fps: 29.97, framesAvailable: true});
    expect(output.previous.length).toBeGreaterThan(100);
    expect(output.after).toEqual(output.previous);
    expect(output.error).toBe('AbortError');
    expect(output.reencodeSize).toEqual([64, 48]);
    expect(output.framesEqual).toBe(true);
    expect(output.size).toEqual([64, 48]);
    expect(output.duration).toBeCloseTo(3 / 29.97, 3);
  }
});

test('movie output locks span windows and cleanup preserves unrelated files', async ({page, context}) => {
  await ready(page);
  const folder = await page.evaluate(async () => {
    const root = await navigator.storage.getDirectory();
    const handle = await root.getDirectoryHandle('custom-test', {create: true});
    globalThis.showDirectoryPicker = async () => handle;
    const selected = await cuemolMovies.pickDirectory();
    const keep = await (await handle.getFileHandle('keep.txt', {create: true})).createWritable();
    await keep.write('preserve'); await keep.close();
    const options = {...movieOptions, useTempDir: false, outputDir: selected.label, makeMovie: false};
    const {job} = await cuemolMovies.begin(options, [64, 48], false);
    await cuemolMovies.writeFrame(job, 0, await frameBytes('red'));
    globalThis.activeMovie = job;
    return options;
  });
  const second = await context.newPage();
  await ready(second);
  const conflicts = await second.evaluate(async options => {
    const errors = [];
    for (const action of [() => cuemolMovies.begin(options, [64, 48], true), () => cuemolMovies.cleanup(options)]) {
      try { await action(); } catch (error) { errors.push(error.message); }
    }
    return errors;
  }, folder);
  expect(conflicts).toHaveLength(2);
  expect(conflicts.every(error => error.includes('in use'))).toBe(true);
  await page.evaluate(() => cuemolMovies.finish(activeMovie));
  const cleaned = await second.evaluate(async options => {
    await cuemolMovies.cleanup(options);
    return {info: await cuemolMovies.inspect(options), keep: await (await outputFile('custom', 'keep.txt')).text()};
  }, folder);
  expect(cleaned.info).toMatchObject({frameCount: 0, hasOutputs: false});
  expect(cleaned.keep).toBe('preserve');
  await second.close();
});

test('unsupported encoders and unconfirmed replacement preserve existing frames', async ({page}) => {
  await ready(page);
  const result = await page.evaluate(async () => {
    const options = {...movieOptions, makeMovie: false};
    const {job, directory} = await cuemolMovies.begin(options, [64, 48], false);
    const png = await frameBytes('blue');
    await cuemolMovies.writeFrame(job, 0, png); await cuemolMovies.finish(job);
    const errors = [];
    for (const [settings, overwrite] of [[options, false], [{...options, makeMovie: true, movieFormat: 'wmv2'}, true]]) {
      try { await cuemolMovies.begin(settings, [64, 48], overwrite); }
      catch (error) { errors.push(error.message); }
    }
    const abort = new AbortController();
    const stopped = cuemolMovies.begin(options, [64,48], true, false, abort.signal).catch(error => error.name);
    abort.abort(); errors.push(await stopped);
    return {errors, original: Array.from(png), after: Array.from(await cuemolMovies.readFrame(directory, 'movie', 0))};
  });
  expect(result.errors[0]).toContain('Confirm replacement');
  expect(result.errors[1]).toContain('native application');
  expect(result.errors[2]).toBe('AbortError');
  expect(result.after).toEqual(result.original);
});

test('temporary movie retention skips active and unknown sessions', async ({page, context}) => {
  await ready(page);
  const active = await page.evaluate(async () => {
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle('cuemol-movies');
    async function put(directory, name, data) {
      const writer = await (await directory.getFileHandle(name, {create:true})).createWritable();
      await writer.write(data); await writer.close();
    }
    async function session(name, files) {
      const directory = await root.getDirectoryHandle(name, {create:true});
      await put(directory, '.cuemol-session.json', JSON.stringify({version:1, application:'cuemol', session:name, created:Date.now()-4*86400000}));
      for (const name of files) await put(directory, name, 'fixture');
    }
    await session('session-old-frames', ['movie_frm_0000.png']);
    await session('session-unknown', ['movie_frm_0000.png','keep.txt']);
    for (let i=0;i<12;i++) await session(`session-movies-${i}`, ['movie.mp4']);
    const {directory} = await cuemolMovies.initialize();
    await put(await root.getDirectoryHandle(directory), 'movie_frm_0000.png', 'active');
    return directory;
  });
  const second = await context.newPage();
  await second.addInitScript(() => { const current=Date.now(); Date.now=()=>current+2*86400000; });
  await ready(second);
  const inspect = async page => page.evaluate(async () => {
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle('cuemol-movies'), result={};
    for await (const [name,directory] of root.entries()) {
      result[name]=[]; for await (const [file] of directory.entries()) result[name].push(file);
    }
    return result;
  });
  let remaining = await inspect(second);
  expect(remaining['session-old-frames']).toBeUndefined();
  expect(remaining['session-unknown']).toContain('movie_frm_0000.png');
  expect(remaining[active]).toContain('movie_frm_0000.png');
  expect(Object.keys(remaining).filter(name=>name.startsWith('session-movies-'))).toHaveLength(10);
  const third = await context.newPage();
  await third.addInitScript(() => { const current=Date.now(); Date.now=()=>current+31*86400000; });
  await ready(third);
  remaining = await inspect(third);
  expect(Object.keys(remaining).filter(name=>name.startsWith('session-movies-'))).toHaveLength(0);
  expect(remaining['session-unknown']).toContain('keep.txt');
  expect(remaining[active]).toContain('movie_frm_0000.png');
  await third.close(); await second.close();
});

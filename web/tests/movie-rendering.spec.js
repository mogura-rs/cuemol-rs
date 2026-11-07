import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {writeFile} from 'node:fs/promises';
import {call, load} from './support.js';

const molecule = 'HETATM    1  C1  LIG A   1       1.000   0.000   0.000  1.00 10.00           C  \nHETATM    2  O1  LIG A   1       0.000   2.000   0.000  1.00 10.00           O  \nEND\n';
const status = page => page.evaluate(() => window.cuemol.status());
async function settle(page) { await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); }
async function rect(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]?.length).toBe(4);
  await settle(page);
  return (await status(page)).ui.rects[name];
}
async function click(page, name, options = {}) {
  const [x, y, w, h] = await rect(page, name);
  await page.mouse.click(x + w / 2, y + h / 2, options); await settle(page);
}
async function field(page, name, text) {
  await click(page, name, {clickCount: 2});
  await page.keyboard.press('ControlOrMeta+a'); await settle(page);
  await page.keyboard.insertText(String(text)); await settle(page);
  await page.keyboard.press('Tab'); await settle(page);
}
async function ready(page, backend, duration = 90) {
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
  await load(page, 'movie.pdb', Buffer.from(molecule));
  await call(page, 'set_render_settings', {settings:'backend=umbreon; width=64; height=48; unit=px; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
  await call(page, 'add_anim', {type:'spin', name:'spin'});
  await call(page, 'set_anim_time', {element:'spin', startMs:0, endMs:duration});
  await click(page, 'Render'); await click(page, 'Render mode Movie');
  await field(page, 'Render Width', 64); await field(page, 'Render Height', 48);
  await field(page, 'Movie frame rate', 29.97);
}
async function saved(page, label) {
  const pending = page.waitForEvent('download'); await click(page, label);
  const download = await pending, chunks = [];
  for await (const chunk of await download.createReadStream()) chunks.push(chunk);
  return {bytes:Buffer.concat(chunks), name:download.suggestedFilename()};
}
async function frames(page, movie) {
  return page.evaluate(async movie => {
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle('cuemol-movies');
    const directory = await root.getDirectoryHandle(movie.outputDir);
    const result = [];
    for (let i = 0; i < movie.frameCount; i++) {
      const file = await (await directory.getFileHandle(`${movie.baseName}_frm_${String(i).padStart(4,'0')}.png`)).getFile();
      result.push({bytes:Array.from(new Uint8Array(await file.arrayBuffer())), modified:file.lastModified});
    }
    return result;
  }, movie);
}

for (const backend of ['webgl', 'webgpu']) {
  test(`Movie Rendering ${backend} saves, browses, re-encodes and cleans up without changing the scene`, async ({page}, testInfo) => {
    test.setTimeout(180000);
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await ready(page, backend);
    const before = await status(page);
    await click(page, 'Start Render');
    await expect.poll(async () => (await status(page)).rendering.historyCount, {timeout:90000}).toBe(1);
    let current = (await status(page)).rendering.current;
    expect(current.movie).toMatchObject({frameCount:3, fps:29.97, framesAvailable:true});
    expect(current.size).toEqual([64,48]);
    expect(current.movie.moviePath).toMatch(/\.mp4$/);
    expect((await status(page)).view).toEqual(before.view);
    expect((await status(page)).scene).toEqual(before.scene);
    const original = await frames(page, current.movie);
    expect(new Set(original.map(frame=>Buffer.from(frame.bytes).toString('base64'))).size).toBeGreaterThan(1);
    expect(original.every(frame => { const png = PNG.sync.read(Buffer.from(frame.bytes)); return png.width === 64 && png.height === 48; })).toBe(true);
    const [x, y, , h] = await rect(page, 'Movie frame');
    await page.mouse.click(x + 5, y + h / 2); await settle(page);
    await expect.poll(async () => (await status(page)).rendering.frame).toBe(0);
    expect((await saved(page, 'Save render')).bytes.equals(Buffer.from(original[0].bytes))).toBe(true);
    await page.evaluate(() => {
      const read = cuemolMovies.readFrame;
      globalThis.frameReads = {active:0, maximum:0, calls:0, read};
      cuemolMovies.readFrame = async (...args) => {
        frameReads.active++; frameReads.calls++; frameReads.maximum=Math.max(frameReads.maximum,frameReads.active);
        try { await new Promise(resolve=>setTimeout(resolve,250)); return await read(...args); }
        finally { frameReads.active--; }
      };
    });
    await page.mouse.click(x + 80, y + h / 2);
    await expect.poll(() => page.evaluate(() => frameReads.calls)).toBe(1);
    for (const offset of [5,60,5]) { await page.mouse.click(x + offset,y + h / 2); await settle(page); }
    await expect.poll(() => page.evaluate(() => frameReads.calls >= 2 && frameReads.active === 0)).toBe(true);
    expect(await page.evaluate(() => frameReads.maximum)).toBe(1);
    await expect.poll(async () => (await status(page)).rendering.frame).toBe(0);
    await page.evaluate(() => { cuemolMovies.readFrame=frameReads.read; });
    expect((await saved(page, 'Save render')).bytes.equals(Buffer.from(original[0].bytes))).toBe(true);
    await page.evaluate(() => { globalThis.showSaveFilePicker = undefined; });
    const movieDownload = await saved(page, 'Save movie as');
    expect(movieDownload.name).toBe('movie.mp4'); expect(movieDownload.bytes.length).toBeGreaterThan(100);
    await writeFile(testInfo.outputPath('movie.mp4'), movieDownload.bytes);
    const opened = page.waitForEvent('popup'); await click(page, 'Open movie');
    const popup = await opened; await popup.waitForURL(/^blob:/); await popup.close();
    await call(page, 'remove_anim', {element:'spin'});
    await field(page, 'Movie frame rate', 12.5); await click(page, 'Re-encode movie');
    await expect.poll(async () => (await status(page)).rendering.historyCount, {timeout:60000}).toBe(2);
    current = (await status(page)).rendering.current;
    expect(current.movie.fps).toBe(12.5);
    expect(await frames(page, current.movie)).toEqual(original);
    expect((await status(page)).view).toEqual(before.view);
    await click(page, 'Previous render');
    expect((await status(page)).rendering.current.movie.moviePath).toBeNull();
    expect((await saved(page, 'Save render')).bytes.equals(Buffer.from(original[0].bytes))).toBe(true);
    await click(page, 'Next render');
    const preview = (await saved(page, 'Save render')).bytes;
    await click(page, 'Cleanup movie'); await click(page, 'Cancel movie output deletion');
    expect(await frames(page, current.movie)).toEqual(original);
    await click(page, 'Cleanup movie'); await click(page, 'Confirm cleanup movie outputs');
    await expect.poll(async () => (await status(page)).rendering.current.movie.framesAvailable).toBe(false);
    expect((await status(page)).rendering.current.movie.moviePath).toBeNull();
    expect((await saved(page, 'Save render')).bytes.equals(preview)).toBe(true);
    const remaining = await page.evaluate(async options => cuemolMovies.inspect(options), current.movie.options);
    expect(remaining).toMatchObject({frameCount:0, hasOutputs:false});
    await page.screenshot({path:testInfo.outputPath('movie-cleanup-history.png')});
    expect(errors).toEqual([]);
  });
}

test('Movie output continues after closing Rendering and Stop preserves concurrent edits', async ({page}, testInfo) => {
  test.setTimeout(180000);
  await ready(page, 'webgl', 100000);
  await field(page, 'Render Width', 128); await field(page, 'Render Height', 96);
  await click(page, 'Start Render');
  await expect.poll(async () => (await status(page)).rendering.progress.frameCount, {timeout:60000}).toBeGreaterThan(0);
  await call(page, 'create_scene', {name:'Edited during movie'});
  await call(page, 'set_view', {zoom:42, center:[1,2,3]});
  const edited = await status(page);
  const [x, y, w] = await rect(page, 'Rendering window');
  await page.mouse.click(x + w - 14, y + 12); await settle(page);
  await expect.poll(async () => (await status(page)).ui.rects['Start Render']).toBeUndefined();
  const completed = (await status(page)).rendering.progress.frameCount;
  await expect.poll(async () => (await status(page)).rendering.progress.frameCount).toBeGreaterThan(completed);
  await click(page, 'Cancel render');
  await expect.poll(async () => (await status(page)).rendering.running, {timeout:30000}).toBe(false);
  const state = await status(page), movie = state.rendering.current.movie;
  expect(movie.frameCount).toBeGreaterThan(0); expect(movie.frameCount).toBeLessThan(2998);
  expect(movie.moviePath).toBeNull(); expect(state.rendering.message).toMatch(/cancelled/i);
  expect(state.scene).toEqual(edited.scene); expect(state.view).toEqual(edited.view);
  expect((await frames(page, movie)).length).toBe(movie.frameCount);
  await click(page, 'Render');
  await page.screenshot({path:testInfo.outputPath('movie-stopped-with-live-edits.png')});
});

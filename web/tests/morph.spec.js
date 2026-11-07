import {test, expect} from '@playwright/test';
import {gzipSync} from 'node:zlib';
import {load, call} from './support.js';

const pdb = Buffer.from('ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  ALA A   2       3.000   0.000   0.000  1.00 10.00           C\nEND\n');
const status = page => page.evaluate(() => window.cuemol?.status());
const frame = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function click(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]).toBeTruthy();
  await frame(page);
  const r = (await status(page)).ui.rects[name];
  await page.mouse.click(r[0]+r[2]/2, r[1]+r[3]/2);
  await frame(page);
}
async function open(page) {
  await click(page, 'Tools menu');
  await click(page, 'Mol morphing animation…');
  await expect.poll(async () => (await status(page)).ui.rects['Morph window']).toBeTruthy();
}
async function frames(page, id) {
  return JSON.parse((await call(page, 'list_morph_frames', {objId:id})).message).frames;
}
async function chooseFiles(page) {
  await click(page, 'Morph Add frame');
  const chosen = page.waitForEvent('filechooser');
  await click(page, 'Morph Add PDB file');
  return chosen;
}
async function addSource(page) {
  await click(page, 'Morph Add frame');
  await click(page, 'Morph Add mol');
  await click(page, 'Morph Confirm molecule frame');
}
for (const backend of ['webgl', 'webgpu']) {
  test(`morph conversion and frame edits preserve base, objects and undo (${backend})`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async () => (await status(page))?.scene?.sceneId, {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    await load(page, 'target.pdb', pdb);
    await load(page, 'source.pdb', pdb);
    const before = await call(page, 'clear_undo');
    const id = before.scene.objects[0].id;
    await open(page);
    expect(await frames(page, id)).toEqual([]);
    await click(page, 'Morph Close morph');
    expect((await status(page)).scene.objects).toEqual(before.scene.objects);
    await open(page);
    await click(page, 'Morph Convert to MorphMol');
    expect((await frames(page, id)).map(f=>f.name)).toEqual(['(this)']);
    expect((await status(page)).scene.objects[0].className).toBe('MorphMol');
    await click(page, 'Morph Frame 0');
    await click(page, 'Morph Delete frame');
    expect(await frames(page, id)).toHaveLength(1);
    await addSource(page);
    expect((await frames(page, id)).map(f=>f.name)).toEqual(['(this)', 'source.pdb']);
    await click(page, 'Morph Frame 0');
    const picker = await chooseFiles(page);
    await picker.setFiles([
      {name:'one.pdb',mimeType:'application/octet-stream',buffer:pdb},
      {name:'two.ent.gz',mimeType:'application/gzip',buffer:gzipSync(pdb)},
    ]);
    await expect.poll(async () => (await status(page)).ui.rects['Morph Frame 3']).toBeTruthy();
    await expect.poll(async () => (await status(page)).loading).toBe(false);
    expect((await frames(page, id)).map(f=>f.name)).toEqual(['(this)', 'one.pdb', 'two.ent.gz', 'source.pdb']);
    expect((await status(page)).scene.objects).toHaveLength(2);
    expect((await status(page)).view).toEqual(before.view);
    await page.screenshot({path:testInfo.outputPath('morph-frames.png')});
    await click(page, 'Morph Delete frame');
    expect((await frames(page, id)).map(f=>f.name)).toEqual(['(this)', 'one.pdb', 'source.pdb']);
    await call(page, 'undo');
    expect(await frames(page, id)).toHaveLength(4);
    await call(page, 'redo');
    expect(await frames(page, id)).toHaveLength(3);
    await click(page, 'Morph Close morph');
    for (let i=0;i<5;i++) await call(page, 'undo');
    expect(await frames(page, id)).toEqual([]);
    expect((await status(page)).scene.objects).toEqual(before.scene.objects);
    await call(page, 'redo');
    expect(await frames(page, id)).toHaveLength(1);
    await open(page);
    await click(page, 'Morph Target molecule');
    await click(page, 'Morph Target source.pdb');
    await expect.poll(async () => (await status(page)).ui.rects['Morph Convert to MorphMol']).toBeTruthy();
    await click(page, 'Morph Convert to MorphMol');
    await click(page, 'Morph Add frame');
    await click(page, 'Morph Add mol');
    await click(page, 'Morph Confirm molecule frame');
    expect(await frames(page, before.scene.objects[1].id)).toHaveLength(1);
    await call(page, 'create_scene', {name:'Other'});
    await expect.poll(async () => (await status(page)).ui.rects['Morph window']).toBeUndefined();
  });
}

test('morph imports stop on failure and discard late picker and parser results', async ({page}) => {
  await page.goto('./?renderer=webgl');
  await expect.poll(async () => (await status(page))?.scene?.sceneId, {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'target.pdb', pdb);
  const id = (await call(page, 'clear_undo')).scene.objects[0].id;
  await open(page); await click(page, 'Morph Convert to MorphMol');
  let picker = await chooseFiles(page);
  await picker.setFiles([
    {name:'good.pdb',mimeType:'application/octet-stream',buffer:pdb},
    {name:'bad.ent.gz',mimeType:'application/gzip',buffer:Buffer.from('invalid gzip')},
    {name:'skipped.pdb',mimeType:'application/octet-stream',buffer:pdb},
  ]);
  await expect.poll(async () => (await status(page)).message).toMatch(/syntax error|gzip|header|format|magic|compression|reading/i);
  await expect.poll(async () => (await status(page)).loading).toBe(false);
  expect((await frames(page, id)).map(f=>f.name)).toEqual(['(this)', 'good.pdb']);
  await call(page, 'undo');
  expect(await frames(page, id)).toHaveLength(1);
  picker = await chooseFiles(page);
  await click(page, 'Morph Close morph'); await open(page);
  await picker.setFiles({name:'late.pdb',mimeType:'application/octet-stream',buffer:pdb});
  await frame(page); await frame(page);
  expect(await frames(page, id)).toHaveLength(1);
  await page.evaluate(() => {
    const parse = globalThis.cuemolLoader.parseFileWithOptions;
    globalThis.cuemolLoader.parseFileWithOptions = async (...args) => {
      const data = await parse(...args);
      await new Promise(resolve => { window.releaseMorphParse = resolve; });
      return data;
    };
  });
  picker = await chooseFiles(page);
  await picker.setFiles({name:'cancelled.pdb',mimeType:'application/octet-stream',buffer:pdb});
  await expect.poll(() => page.evaluate(() => typeof window.releaseMorphParse)).toBe('function');
  await click(page, 'Morph Close morph'); await open(page);
  await page.evaluate(() => window.releaseMorphParse());
  await frame(page); await frame(page);
  expect(await frames(page, id)).toHaveLength(1);
  expect((await status(page)).scene.objects).toHaveLength(1);
  await page.evaluate(() => { delete window.releaseMorphParse; });
  picker = await chooseFiles(page);
  await picker.setFiles({name:'superseded.pdb',mimeType:'application/octet-stream',buffer:pdb});
  await expect.poll(() => page.evaluate(() => typeof window.releaseMorphParse)).toBe('function');
  await page.evaluate(() => window.cuemol.command('undo'));
  await expect.poll(async () => (await status(page)).scene.objects[0].className).toBe('MolCoord');
  await page.evaluate(() => window.releaseMorphParse());
  await expect.poll(async () => (await status(page)).loading).toBe(false);
  expect(await frames(page, id)).toHaveLength(0);
});

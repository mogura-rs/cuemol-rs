import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = Buffer.from('ATOM      1  N   GLY A   1       0.000   0.000   0.000  1.00 10.00           N  \nATOM      2  CA  GLY A   2       3.000   0.000   0.000  1.00 10.00           C  \nEND\n');
async function click(page, name) {
  await expect.poll(() => page.evaluate(name => window.cuemol.status().ui.rects[name], name)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const r = await page.evaluate(name => window.cuemol.status().ui.rects[name], name);
  await page.mouse.click(r[0] + r[2] / 2, r[1] + r[3] / 2);
}
async function fill(page, name, value) {
  await click(page, `Molecule edit ${name}`);
  await page.keyboard.press('ControlOrMeta+A');
  if (value) await page.keyboard.type(value); else await page.keyboard.press('Backspace');
}
const state = page => page.evaluate(() => window.cuemol.status());
const closed = page => expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Molecule edit window'])).toBeFalsy();
async function open(page, name) {
  await click(page, 'Edit menu'); await click(page, `${name}…`);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Molecule edit window'])).toBeTruthy();
}
async function ready(page, backend, second = false) {
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
  await load(page, 'first.pdb', pdb);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
  if (second) {
    await load(page, 'second.pdb', Buffer.from(pdb.toString().replaceAll('GLY A', 'GLY B')));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(2);
  }
  return call(page, 'clear_undo');
}
async function query(page, name, args) { return JSON.parse((await call(page, name, args)).message); }
for (const backend of ['webgl', 'webgpu']) {
  test(`molecule deletion requires explicit selection and preserves failed edits (${backend})`, async ({page}) => {
    const before = await ready(page, backend);
    await open(page, 'Delete mol atoms');
    await click(page, 'Molecule edit OK');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await fill(page, 'Selection', 'none');
    await click(page, 'Molecule edit OK');
    await expect.poll(() => state(page).then(s => s.message)).toBe('No atoms matched the selection');
    await fill(page, 'Selection', 'invalid (((');
    await click(page, 'Molecule edit OK');
    await expect.poll(() => state(page).then(s => s.message)).not.toBe('No atoms matched the selection');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await call(page, 'undo');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await fill(page, 'Selection', '*');
    await click(page, 'Molecule edit OK'); await closed(page);
    expect((await state(page)).scene.objects[0].atoms).toBe(0);
    expect((await call(page, 'undo')).scene.objects).toEqual(before.scene.objects);
    expect((await call(page, 'redo')).scene.objects[0].atoms).toBe(0);
    await open(page, 'Delete mol atoms');
    await click(page, 'Molecule edit Choose selection');
    await click(page, 'Selection tab History');
    await click(page, 'Molecule edit Cancel');
    await closed(page);
  });

  test(`chain and residue forms confirm unusual values and reset fields (${backend})`, async ({page}, testInfo) => {
    test.setTimeout(120000);
    const before = await ready(page, backend);
    const id = before.scene.objects[0].id;
    await open(page, 'Change chain ID');
    await fill(page, 'New chain ID', 'A');
    await click(page, 'Molecule edit OK');
    await expect.poll(() => state(page).then(s => s.message)).toContain('destination residue');
    await fill(page, 'Selection', 'none');
    await click(page, 'Molecule edit OK'); await closed(page);
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await call(page, 'undo');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await open(page, 'Change chain ID');
    await fill(page, 'New chain ID', ' ');
    await click(page, 'Molecule edit OK');
    await click(page, 'Molecule edit No');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await fill(page, 'New chain ID', ' AB ');
    await click(page, 'Molecule edit OK');
    await page.screenshot({path: testInfo.outputPath('molecule-chain-confirm.png')});
    await click(page, 'Molecule edit Yes'); await closed(page);
    expect((await query(page, 'list_chains', {objId: id})).chains).toEqual(['AB']);
    await call(page, 'undo');
    expect((await query(page, 'list_chains', {objId: id})).chains).toEqual(['A']);
    await call(page, 'redo');
    expect((await query(page, 'list_chains', {objId: id})).chains).toEqual(['AB']);
    await call(page, 'undo');
    await open(page, 'Change chain ID');
    await click(page, 'Molecule edit OK');
    expect((await query(page, 'list_chains', {objId: id})).chains).toEqual(['A']);
    await click(page, 'Molecule edit Cancel');
    await open(page, 'Change residue number');
    await click(page, 'Molecule edit Start from');
    await click(page, 'Molecule edit OK');
    await expect.poll(() => state(page).then(s => s.message)).toBe('Residue index change was rejected');
    await click(page, 'Molecule edit Shift by');
    await fill(page, 'Residue value', '0');
    await click(page, 'Molecule edit OK');
    await fill(page, 'Residue value', '2147483647');
    await click(page, 'Molecule edit OK');
    await expect.poll(() => state(page).then(s => s.message)).toContain('integer range');
    expect((await query(page, 'list_residues', {objId: id, chain: 'A'})).residues.map(r => r.index)).toEqual([1, 2]);
    await fill(page, 'Residue value', '1.5');
    await click(page, 'Molecule edit OK'); await closed(page);
    expect((await query(page, 'list_residues', {objId: id, chain: 'A'})).residues.map(r => r.index)).toEqual([2, 3]);
    await call(page, 'undo');
    await open(page, 'Change residue number');
    await click(page, 'Molecule edit Start from');
    await fill(page, 'Residue value', '10000');
    await click(page, 'Molecule edit OK');
    await click(page, 'Molecule edit No');
    expect((await query(page, 'list_residues', {objId: id, chain: 'A'})).residues.map(r => r.index)).toEqual([1, 2]);
    await click(page, 'Molecule edit OK');
    await click(page, 'Molecule edit Yes'); await closed(page);
    expect((await query(page, 'list_residues', {objId: id, chain: 'A'})).residues.map(r => r.index)).toEqual([10000, 10001]);
    await call(page, 'undo');
    expect((await query(page, 'list_residues', {objId: id, chain: 'A'})).residues.map(r => r.index)).toEqual([1, 2]);
  });

  test(`merge dialog copies or moves with one undo and rejects stale scenes (${backend})`, async ({page}, testInfo) => {
    test.setTimeout(120000);
    const before = await ready(page, backend, true);
    await open(page, 'Merge molecule');
    await click(page, 'Molecule edit From molecule');
    await click(page, `Molecule edit From molecule ${before.scene.objects[0].name}`);
    await click(page, 'Molecule edit To molecule');
    await click(page, `Molecule edit To molecule ${before.scene.objects[0].name}`);
    await click(page, 'Molecule edit OK');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await click(page, 'Molecule edit To molecule');
    await click(page, `Molecule edit To molecule ${before.scene.objects[1].name}`);
    await fill(page, 'Selection', 'none');
    await click(page, 'Molecule edit OK');
    await expect.poll(() => state(page).then(s => s.message)).toContain('no atoms');
    expect((await state(page)).scene.objects).toEqual(before.scene.objects);
    await fill(page, 'Selection', '');
    await click(page, 'Molecule edit OK'); await closed(page);
    expect((await state(page)).scene.objects.map(o => o.atoms)).toEqual([2, 4]);
    expect((await call(page, 'undo')).scene.objects).toEqual(before.scene.objects);
    await open(page, 'Merge molecule');
    await click(page, 'Molecule edit Copy');
    await page.screenshot({path: testInfo.outputPath('molecule-merge.png')});
    await click(page, 'Molecule edit OK'); await closed(page);
    expect((await state(page)).scene.objects.map(o => o.atoms)).toEqual([0, 4]);
    expect((await call(page, 'undo')).scene.objects).toEqual(before.scene.objects);
    expect((await call(page, 'redo')).scene.objects.map(o => o.atoms)).toEqual([0, 4]);
    await open(page, 'Merge molecule');
    await call(page, 'create_scene', {name: 'Other scene'});
    await closed(page);
  });
}

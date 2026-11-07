import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = Buffer.from('ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \nATOM      2  CA  GLY A   2       3.000   0.000   0.000  1.00 10.00           C  \nEND\n');
async function click(page, name) {
  await expect.poll(() => page.evaluate(name => window.cuemol.status().ui.rects[name], name)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const r = await page.evaluate(name => window.cuemol.status().ui.rects[name], name);
  await page.mouse.click(r[0] + r[2] / 2, r[1] + r[3] / 2);
}
async function open(page) {
  await click(page, 'Tools menu');
  await click(page, 'Reassign secondary str…');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Secondary window'])).toBeTruthy();
}
async function fill(page, value) {
  await click(page, 'Secondary Selection');
  await page.keyboard.press('ControlOrMeta+A');
  if (value) await page.keyboard.type(value); else await page.keyboard.press('Backspace');
}
const closed = page => expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Secondary window'])).toBeFalsy();
async function count(page, id, selection) {
  return JSON.parse((await call(page, 'count_selection', {objId: id, selection})).message).count;
}
for (const backend of ['webgl', 'webgpu']) {
  test(`secondary dialog assigns types, preserves failures and resets options (${backend})`, async ({page}, testInfo) => {
    test.setTimeout(180000);
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    await load(page, 'probe.pdb', pdb);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
    const id = (await call(page, 'clear_undo')).scene.objects[0].id;
    await open(page);
    await click(page, 'Secondary Assign');
    await click(page, 'Secondary Choose selection');
    await click(page, 'Selection tab History');
    await click(page, 'Secondary Choose selection');
    await fill(page, 'invalid (((');
    await click(page, 'Secondary OK');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('syntax error');
    expect(await count(page, id, 'rprop secondary=helix')).toBe(0);
    await fill(page, 'resi 1');
    await click(page, 'Secondary Type'); await click(page, 'Secondary Type α helix');
    await page.screenshot({path: testInfo.outputPath('secondary-assign.png')});
    await click(page, 'Secondary OK'); await closed(page);
    expect(await count(page, id, 'rprop secondary2=Hs')).toBe(1);
    await open(page);
    await click(page, 'Secondary Assign'); await fill(page, 'none');
    await click(page, 'Secondary OK'); await closed(page);
    await call(page, 'undo');
    expect(await count(page, id, 'rprop secondary=helix')).toBe(0);
    await call(page, 'redo');
    expect(await count(page, id, 'rprop secondary=helix')).toBe(1);
    for (const [label, selector] of [['β strand', 'rprop secondary=sheet'], ['3-10 helix', 'rprop secondary=helix'], ['π helix', 'rprop secondary=helix'], ['α helix', 'rprop secondary=helix']]) {
      await open(page); await click(page, 'Secondary Assign');
      await click(page, 'Secondary Type'); await click(page, `Secondary Type ${label}`);
      await click(page, 'Secondary OK'); await closed(page);
      expect(await count(page, id, selector)).toBe(2);
    }
    await open(page); await click(page, 'Secondary Assign');
    await click(page, 'Secondary OK'); await closed(page);
    expect(await count(page, id, 'rprop secondary=helix')).toBe(0);
    await call(page, 'undo');
    expect(await count(page, id, 'rprop secondary=helix')).toBe(2);
    await open(page); await click(page, 'Secondary Gap fill');
    await page.screenshot({path: testInfo.outputPath('secondary-recalculate.png')});
    await click(page, 'Secondary OK'); await closed(page);
    expect(await count(page, id, 'rprop secondary=helix')).toBe(0);
    await call(page, 'undo');
    expect(await count(page, id, 'rprop secondary=helix')).toBe(2);
    await open(page); await click(page, 'Secondary Cancel'); await closed(page);
    await open(page); await call(page, 'create_scene', {name: 'Other scene'}); await closed(page);
  });
}

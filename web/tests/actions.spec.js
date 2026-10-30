import {test, expect} from '@playwright/test';
import {load, call} from './support.js';
import {readFile} from 'node:fs/promises';

const pdb = 'ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       2.000   0.000   0.000  1.00 10.00           C\nEND\n';
const status = page => page.evaluate(() => window.cuemol.status());
const present = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function choose(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]?.length).toBe(4);
  const [x, y, w, h] = (await status(page)).ui.rects[name];
  await page.mouse.click(x + w / 2, y + h / 2);
  await present(page);
}
async function setup(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'probe.pdb', Buffer.from(pdb));
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  await choose(page, 'Actions');
}
const hasCartoon = state => state.scene.objects[0].renderers.some(renderer => renderer.name === 'p_cartoon');

test('Actions presets share one history step and export replayable operations', async ({page}, testInfo) => {
  await setup(page);
  await choose(page, 'Actions preset cartoon-ligand');
  await choose(page, 'Actions Run');
  await expect.poll(async () => hasCartoon(await status(page))).toBe(true);
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  const steps = Object.keys((await status(page)).ui.rects).filter(key => /^History step \d+$/.test(key));
  const last = Math.max(...steps.map(key => Number(key.split(' ').at(-1))));
  await choose(page, `History step ${last}`);
  const downloaded = page.waitForEvent('download');
  await choose(page, 'History Save QAX…');
  const artifact = await downloaded;
  const xml = await readFile(await artifact.path(), 'utf8');
  expect(xml).toContain('cuemol-actions');
  expect(xml.match(/<op /g)).toHaveLength(3);
  expect(xml).toContain('p_cartoon');
  await choose(page, 'History Undo');
  expect(hasCartoon(await status(page))).toBe(false);
  await choose(page, 'History Redo');
  expect(hasCartoon(await status(page))).toBe(true);
  await page.screenshot({path:testInfo.outputPath('actions-history-presets.png')});
  await choose(page, 'History Undo');
  await load(page, 'replay.qax', Buffer.from(xml));
  await expect.poll(() => page.evaluate(() => localStorage.getItem('cuemol.actions'))).toContain('replay');
  await choose(page, 'Actions file replay');
  await choose(page, 'Actions Run');
  await expect.poll(async () => hasCartoon(await status(page))).toBe(true);
});

test('Actions saves a reusable macro and applies it to a different molecule', async ({page}) => {
  await setup(page);
  await choose(page, 'Actions preset cartoon-ligand');
  await choose(page, 'Actions Run');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  const steps = Object.keys((await status(page)).ui.rects).filter(key => /^History step \d+$/.test(key));
  const last = Math.max(...steps.map(key => Number(key.split(' ').at(-1))));
  await choose(page, `History step ${last}`);
  await choose(page, 'History Save to Actions…');
  await choose(page, 'Actions name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Reusable view');
  await choose(page, 'Actions Save');
  await expect.poll(() => page.evaluate(() => localStorage.getItem('cuemol.actions'))).toContain('Reusable view');
  await choose(page, 'History Undo');
  await call(page, 'rename_node', {nodeType:'object', nodeId:(await status(page)).scene.objects[0].id, name:'other molecule'});
  await choose(page, 'Actions preset Reusable view');
  await choose(page, 'Actions Run');
  await expect.poll(async () => hasCartoon(await status(page))).toBe(true);
  expect((await status(page)).scene.objects[0].name).toBe('other molecule');
  await choose(page, 'History step 0');
  await choose(page, 'History Go to step');
  expect((await status(page)).scene.objects.length).toBe(0);
});

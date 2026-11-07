import {test, expect} from '@playwright/test';
import {load, acceptFileOptions} from './support.js';

const atom = 'ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \n';
const pdb = `${atom}END\n`;
async function ready(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
}
async function click(page, key) {
  await expect.poll(() => page.evaluate(key => window.cuemol.status().ui.rects[key], key)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const rect = await page.evaluate(key => window.cuemol.status().ui.rects[key], key);
  await page.mouse.click(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
}
const recent = page => page.evaluate(() => window.cuemol.status().recentFiles);
async function recentMenu(page) { await click(page, 'File menu'); await click(page, 'Open Recent'); }
async function reopen(page, id) { await recentMenu(page); await click(page, `Recent file ${id}`); }

test('Recent files reopen through options, exclude failed or cancelled loads and clear without changing the scene', async ({page}) => {
  await ready(page);
  await load(page, 'original.pdb', Buffer.from(pdb));
  await expect.poll(() => recent(page)).toHaveLength(1);
  const original = (await recent(page))[0];
  expect(original.persistent).toBe(false);
  await load(page, 'cancelled.pdb', Buffer.from(pdb), {accept:false});
  await click(page, 'File options Cancel');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
  expect(await recent(page)).toEqual([original]);
  await load(page, 'invalid.pdb', Buffer.from(pdb), {accept:false});
  await click(page, 'File selection enabled');
  await click(page, 'File selection');
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.type('invalid (((');
  await click(page, 'File options Open');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
  expect(await recent(page)).toEqual([original]);
  await reopen(page, original.id);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Choose options for original.pdb');
  await acceptFileOptions(page);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(2);
  expect(await recent(page)).toEqual([original]);
  const scene = await page.evaluate(() => window.cuemol.status().scene);
  await recentMenu(page);
  await click(page, 'Clear Recent');
  await expect.poll(() => recent(page)).toEqual([]);
  expect(await page.evaluate(() => window.cuemol.status().scene)).toEqual(scene);
  await page.reload();
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.recentFiles)).toEqual([]);
});

test('File handles persist, distinguish same-named files and reopen current bytes after reload', async ({page}) => {
  await ready(page);
  for (const directory of ['first', 'second']) {
    await page.evaluate(async ({directory, pdb}) => {
      const root = await navigator.storage.getDirectory();
      const dir = await root.getDirectoryHandle(directory, {create:true});
      const handle = await dir.getFileHandle('same.pdb', {create:true});
      const writer = await handle.createWritable();
      await writer.write(pdb); await writer.close();
      window.showOpenFilePicker = async () => [handle];
    }, {directory, pdb});
    await click(page, 'File menu'); await click(page, 'Open File');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['File options Open'])).toBeTruthy();
    await acceptFileOptions(page);
  }
  await expect.poll(() => recent(page)).toHaveLength(2);
  const entries = await recent(page);
  expect(entries.every(entry => entry.persistent && entry.name === 'same.pdb')).toBe(true);
  expect(entries[0].id).not.toBe(entries[1].id);
  await page.evaluate(async text => {
    const root = await navigator.storage.getDirectory();
    const dir = await root.getDirectoryHandle('first');
    const handle = await dir.getFileHandle('same.pdb');
    const writer = await handle.createWritable();
    await writer.write(text); await writer.close();
  }, atom + atom.replace('      1', '      2').replace(' CA ', ' CB ') + 'END\n');
  await page.reload();
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await expect.poll(() => recent(page)).toEqual(entries);
  await reopen(page, entries[1].id);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['File options Open'])).toBeTruthy();
  await acceptFileOptions(page);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects[0]?.atoms)).toBe(2);
  await expect.poll(() => recent(page)).toEqual([entries[1], entries[0]]);
  await recentMenu(page); await click(page, 'Clear Recent');
  await expect.poll(() => recent(page)).toEqual([]);
  await page.reload();
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await expect.poll(() => recent(page)).toEqual([]);
});

test('Open picker cancellation preserves scene and history while permission errors remain visible', async ({page}) => {
  await ready(page);
  await load(page, 'original.pdb', Buffer.from(pdb));
  await expect.poll(() => recent(page)).toHaveLength(1);
  const before = await page.evaluate(() => window.cuemol.status());
  await page.evaluate(() => { window.showOpenFilePicker = async () => { throw new DOMException('Cancelled', 'AbortError'); }; });
  await click(page, 'File menu'); await click(page, 'Open File');
  const after = await page.evaluate(() => window.cuemol.status());
  expect(after.scene).toEqual(before.scene);
  expect(after.recentFiles).toEqual(before.recentFiles);
  expect(after.message).toBe(before.message);
  await page.evaluate(() => { window.showOpenFilePicker = async () => { throw new DOMException('Read access denied', 'SecurityError'); }; });
  await click(page, 'File menu'); await click(page, 'Open File');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Read access denied');
  expect(await recent(page)).toEqual(before.recentFiles);
});

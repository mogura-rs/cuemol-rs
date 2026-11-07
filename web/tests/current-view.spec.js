import {readFile} from 'node:fs/promises';
import {test, expect} from '@playwright/test';
import {call, load} from './support.js';

async function click(page, key) {
  await expect.poll(() => page.evaluate(key => window.cuemol.status().ui.rects[key], key)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const [x, y, w, h] = await page.evaluate(key => window.cuemol.status().ui.rects[key], key);
  await page.mouse.click(x + w / 2, y + h / 2);
}
for (const renderer of ['webgl', 'webgpu']) {
  test(`Save current view ${renderer} downloads a portable camera and leaves scene history intact`, async ({page}) => {
    await page.goto(`./?renderer=${renderer}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'probe.pdb', Buffer.from('ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n'));
    const original = await page.evaluate(() => window.cuemol.status());
    const object = original.scene.objects[0];
    await call(page, 'rename_node', {nodeType:'object', nodeId:object.id, name:'renamed'});
    const current = await call(page, 'set_view', {center:[1.123456789, -3.987654321, 7.], zoom:31.125, slab:16.375, distance:83.25, rotation:[0, 0, 0.6, 0.8]});
    const download = page.waitForEvent('download');
    await click(page, 'File menu'); await click(page, 'Save current view');
    const saved = await download;
    expect(saved.suggestedFilename()).toBe('view.cam');
    const xml = await readFile(await saved.path(), 'utf8');
    expect(xml).toContain('<camera');
    expect(xml).toContain('name="__current"');
    expect(xml).not.toContain('visibilities');
    expect(xml).not.toContain('src=');
    expect((await page.evaluate(() => window.cuemol.status())).view).toEqual(current.view);
    const undone = await call(page, 'undo');
    expect(undone.scene.objects[0].name).toBe(object.name);
    expect(undone.view).toEqual(current.view);
    const redone = await call(page, 'redo');
    expect(redone.scene.objects[0].name).toBe('renamed');
    await call(page, 'set_view', {center:[0,0,0], zoom:9, slab:99});
    await load(page, 'saved view.cam', Buffer.from(xml));
    const loaded = await page.evaluate(() => window.cuemol.status());
    expect(loaded.view).toEqual(current.view);
    expect(loaded.scene.objects[0].name).toBe('renamed');
  });
}

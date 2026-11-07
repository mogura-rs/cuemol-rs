import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = Buffer.from('ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n');
const status = page => page.evaluate(() => window.cuemol?.status());
const frame = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function click(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]).toBeTruthy();
  await frame(page);
  const r = (await status(page)).ui.rects[name];
  await page.mouse.click(r[0] + r[2]/2, r[1] + r[3]/2);
  await frame(page);
}
async function open(page) {
  await click(page, 'Tools menu');
  await click(page, 'APBS elepot calculation…');
  await expect.poll(async () => (await status(page)).ui.rects['APBS window']).toBeTruthy();
}

for (const backend of ['webgl', 'webgpu']) {
  test(`APBS form shows the native execution boundary and resets options (${backend})`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async () => (await status(page))?.scene?.sceneId, {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'source.pdb', pdb);
    const before = await call(page, 'clear_undo');
    await open(page);
    expect((await status(page)).ui.rects['APBS Force field']).toBeTruthy();
    expect((await status(page)).ui.rects['APBS Use hydrogen atoms']).toBeUndefined();
    await click(page, 'APBS Start');
    expect((await status(page)).scene.objects).toEqual(before.scene.objects);
    expect((await status(page)).view).toEqual(before.view);
    await click(page, 'APBS Method');
    await click(page, 'APBS Use internal method');
    await expect.poll(async () => (await status(page)).ui.rects['APBS Use hydrogen atoms']).toBeTruthy();
    expect((await status(page)).ui.rects['APBS Force field']).toBeUndefined();
    await page.screenshot({path:testInfo.outputPath('apbs-native-boundary.png')});
    await click(page, 'APBS Close'); await open(page);
    await expect.poll(async () => (await status(page)).ui.rects['APBS Force field']).toBeTruthy();
    await call(page, 'create_scene', {name:'Other'});
    await expect.poll(async () => (await status(page)).ui.rects['APBS window']).toBeUndefined();
  });
}

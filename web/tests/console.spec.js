import {test, expect} from '@playwright/test';
import {load, call} from './support.js';
import {readFile} from 'node:fs/promises';
import {PNG} from 'pngjs';

const pdb = 'ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       2.000   0.000   0.000  1.00 10.00           C\nEND\n';
const status = page => page.evaluate(() => window.cuemol.status());
const present = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function choose(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]?.length).toBe(4);
  const [x, y, w, h] = (await status(page)).ui.rects[name];
  await page.mouse.click(x + w / 2, y + h / 2);
  await present(page);
}
async function input(page, text) {
  await choose(page, 'Console input');
  await page.keyboard.press('ControlOrMeta+a');
  await present(page);
  await page.keyboard.insertText(text);
  await present(page);
}
async function setup(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'probe.pdb', Buffer.from(pdb));
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  await call(page, 'rename_node', {nodeType:'object', nodeId:(await status(page)).scene.objects[0].id, name:'probe'});
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('probe');
  await choose(page, 'Console');
}

test('CueMol console resolves names sequentially and a submission has one undo entry', async ({page}, testInfo) => {
  await setup(page);
  await input(page, 'create_renderer probe, simple, name=wire; set probe/wire.visible, false; select probe, resid 2; rename probe, renamed');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('renamed');
  expect((await status(page)).scene.objects[0].renderers.find(renderer => renderer.name === 'wire').visible).toBe(false);
  expect((await status(page)).selection).toBe('resid 2');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  expect((await status(page)).scene.objects[0].renderers.some(renderer => renderer.name === 'wire')).toBe(false);
  await call(page, 'redo');
  await input(page, 'rename renamed, changed; missing_command; hide changed');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console error');
  expect((await status(page)).scene.objects[0].name).toBe('changed');
  expect((await status(page)).scene.objects[0].visible).toBe(true);
  await call(page, 'undo');
  await input(page, 'get_scene_state');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'redo');
  expect((await status(page)).scene.objects[0].name).toBe('changed');
  await page.screenshot({path: testInfo.outputPath('console-native-dialect.png')});
});

test('Console completes molecule names, recalls history, and submits multiline input once', async ({page}) => {
  await setup(page);
  await input(page, 'select pro');
  await page.keyboard.press('Tab');
  await page.keyboard.insertText(', resid 1');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).selection).toBe('resid 1');
  await input(page, 'set_view zoom=12');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBe(12);
  await call(page, 'set_view', {zoom:21});
  await choose(page, 'Console input');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBe(12);
  await input(page, 'rename probe, first');
  await page.keyboard.press('Shift+Enter');
  await present(page);
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  await page.keyboard.insertText('rename first, second');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('second');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  const viewport = (await status(page)).ui.rects.Viewport;
  await input(page, 'rename probe, step0');
  for (let index = 1; index < 12; index++) {
    await page.keyboard.press('Shift+Enter');
    await present(page);
    await page.keyboard.insertText(`rename step${index - 1}, step${index}`);
    await present(page);
  }
  expect((await status(page)).ui.rects.Viewport).toEqual(viewport);
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('step11');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
});

test('Console preserves submissions and the next draft within one browser event batch', async ({page}) => {
  await setup(page);
  await input(page, 'rename probe, renamed');
  await page.evaluate(() => {
    const target = document.activeElement;
    if (!(target instanceof HTMLInputElement)) throw new Error('Console text input is not focused');
    const enter = () => {
      target.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', code:'Enter', bubbles:true}));
      target.dispatchEvent(new KeyboardEvent('keyup', {key:'Enter', code:'Enter', bubbles:true}));
    };
    const text = value => {
      target.value = value;
      target.dispatchEvent(new InputEvent('input', {data:value, inputType:'insertText', bubbles:true}));
    };
    enter();
    text('undo');
    enter();
    text('redo');
  });
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('cuemol.console.history'))))
    .toEqual(['rename probe, renamed', 'undo']);
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('probe');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('renamed');
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('cuemol.console.history'))))
    .toEqual(['rename probe, renamed', 'undo', 'redo']);
});

test('Console waits for an IME commit before running a command', async ({page, browserName}) => {
  test.skip(browserName !== 'chromium', 'Chromium exposes the browser IME protocol.');
  await setup(page);
  await input(page, 'rename probe, ');
  const session = await page.context().newCDPSession(page);
  await session.send('Input.imeSetComposition', {text:'にほん', selectionStart:3, selectionEnd:3});
  await present(page);
  await session.send('Input.dispatchKeyEvent', {type:'keyDown', key:'Enter', code:'Enter', windowsVirtualKeyCode:13});
  await session.send('Input.insertText', {text:'日本語'});
  await present(page);
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('日本語');
});

test('Console scene downloads and explicit history reset respect transaction boundaries', async ({page}) => {
  await setup(page);
  await input(page, 'rename probe, saved');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('saved');
  const download = page.waitForEvent('download');
  await input(page, 'save console-scene.qsc');
  await page.keyboard.press('Enter');
  const scene = await download;
  expect(await readFile(await scene.path(), 'utf8')).toContain('name="saved"');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('saved');
  await input(page, 'rename saved, partial; clear_undo');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console error');
  expect((await status(page)).scene.objects[0].name).toBe('partial');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('saved');
  await input(page, 'clear_undo');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'redo');
  expect((await status(page)).scene.objects[0].name).toBe('saved');
});

test('PyMOL console updates representations, labels, measurements and selected exports', async ({page}, testInfo) => {
  await setup(page);
  await choose(page, 'Console PyMOL');
  await input(page, 'show sticks, probe; color red, probe and resi 2; label probe and name CA, "%s%s" % (resn, resi); distance d, probe and resi 1, probe and resi 2; set_name probe, renamed');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  const current = await status(page);
  expect(current.scene.objects[0].name).toBe('renamed');
  expect(current.scene.objects[0].renderers.some(renderer => renderer.name === 'pym:sticks' && renderer.visible)).toBe(true);
  expect(current.ui.navigation.labels).toBe(2);
  expect(current.ui.navigation.measurementLabels).toContain('1.00');
  let download = page.waitForEvent('download');
  await input(page, 'save selected.pdb, renamed and resi 2');
  await page.keyboard.press('Enter');
  const selected = await download;
  const output = await readFile(await selected.path(), 'utf8');
  expect(output).toContain('GLY');
  expect(output).not.toContain('ALA');
  download = page.waitForEvent('download');
  await input(page, 'png console.png, 96, 64, ray=0');
  await page.keyboard.press('Enter');
  const captured = await download;
  const image = PNG.sync.read(await readFile(await captured.path()));
  expect([image.width, image.height]).toEqual([96, 64]);
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await page.screenshot({path:testInfo.outputPath('console-pymol.png')});
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  expect((await status(page)).scene.objects[0].renderers.some(renderer => renderer.name === 'pym:sticks')).toBe(false);
});

test('Console awaits downloads in order and stopping rejects late results while keeping one undo', async ({page}) => {
  await setup(page);
  await choose(page, 'Console PyMOL');
  const url = new URL('console-fixture.pdb', page.url()).href;
  await page.route(url, route => route.fulfill({body:pdb, contentType:'text/plain'}));
  await input(page, `load ${url}, fetched; set_name fetched, ready`);
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects.map(object => object.name)).toEqual(['probe', 'ready']);
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'undo');
  expect((await status(page)).scene.objects.map(object => object.name)).toEqual(['probe']);
  await page.unroute(url);
  let arrived;
  const requested = new Promise(resolve => { arrived = resolve; });
  await page.route(url, route => { arrived(route); });
  await input(page, `set_name probe, pending; load ${url}, late; set_name pending, unwanted`);
  await page.keyboard.press('Enter');
  const pending = await requested;
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('pending');
  await choose(page, 'Console Stop');
  await pending.fulfill({body:pdb, contentType:'text/plain'}).catch(() => {});
  await page.waitForTimeout(300);
  expect((await status(page)).scene.objects.map(object => object.name)).toEqual(['pending']);
  await call(page, 'undo');
  expect((await status(page)).scene.objects.map(object => object.name)).toEqual(['probe']);
});

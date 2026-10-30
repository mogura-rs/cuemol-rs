import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = 'ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C\nEND\n';
const status = page => page.evaluate(() => window.cuemol.status());
const present = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function click(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]?.length).toBe(4);
  const [x,y,w,h] = (await status(page)).ui.rects[name];
  await page.mouse.click(x+w/2,y+h/2);
  await present(page);
}
async function menu(page, action) {
  await click(page,'Edit menu');
  await click(page,`Edit ${action}`);
}
async function setup(page, context) {
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await load(page,'clipboard.pdb',Buffer.from(pdb));
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  const object = (await status(page)).scene.objects[0];
  await click(page,`object:${object.id}`);
  return object;
}

test('workspace clipboard survives source deletion and paste has one undo step', async ({page,context},testInfo) => {
  const object = await setup(page,context);
  await call(page,'clear_undo');
  await menu(page,'Copy');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain('CueMolClipboard/1');
  await menu(page,'Cut');
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(0);
  await menu(page,'Paste');
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  const pasted = (await status(page)).scene.objects[0];
  expect(pasted.name).toBe(object.name);
  expect(pasted.atoms).toBe(1);
  expect(pasted.id).not.toBe(object.id);
  await call(page,'undo');
  expect((await status(page)).scene.objects).toHaveLength(0);
  await call(page,'undo');
  expect((await status(page)).scene.objects[0].id).toBe(object.id);
  await page.screenshot({path:testInfo.outputPath('clipboard-undo.png')});
});

test('Edit clipboard commands preserve text focus across the menu', async ({page,context}) => {
  await setup(page,context);
  await click(page,'Console');
  await click(page,'Console input');
  await page.keyboard.insertText('set_view zoom=17');
  await page.keyboard.press('ControlOrMeta+a');
  await present(page);
  await menu(page,'Copy');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('set_view zoom=17');
  await menu(page,'Cut');
  await menu(page,'Paste');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBe(17);
  expect((await status(page)).scene.objects).toHaveLength(1);
});

test('denied or delayed clipboard writes cannot cut a changed scene', async ({page,context}) => {
  const object = await setup(page,context);
  await page.evaluate(() => Object.defineProperty(navigator.clipboard,'writeText',{configurable:true,value:async () => {throw new Error('denied by test');}}));
  await menu(page,'Cut');
  await expect.poll(async () => (await status(page)).message).toContain('denied by test');
  expect((await status(page)).scene.objects[0].id).toBe(object.id);
  await page.evaluate(() => Object.defineProperty(navigator.clipboard,'writeText',{configurable:true,value:() => new Promise(resolve => {window.__clipboardResolve=resolve;})}));
  await menu(page,'Cut');
  await expect.poll(() => page.evaluate(() => Boolean(window.__clipboardResolve))).toBe(true);
  await call(page,'rename_node',{nodeType:'object',nodeId:object.id,name:'changed during copy'});
  await page.evaluate(() => window.__clipboardResolve());
  await expect.poll(async () => (await status(page)).message).toContain('cut cancelled');
  expect((await status(page)).scene.objects[0].name).toBe('changed during copy');
});

import {test, expect} from '@playwright/test';
import {call, load} from './support.js';

async function modified(page) {
  const state = await call(page, 'list_node_props', {nodeType:'scene'});
  return JSON.parse(state.message).properties.find(row => row.key === 'modified').value;
}

test('Scene status follows edits, history and successful download', async ({page}) => {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  expect(await modified(page)).toBe('false');
  await call(page, 'set_node_prop', {nodeType:'scene', prop:'bgcolor', value:'red'});
  expect(await modified(page)).toBe('true');
  await call(page, 'undo');
  expect(await modified(page)).toBe('false');
  await call(page, 'redo');
  expect(await modified(page)).toBe('true');
  await call(page, 'clear_undo');
  expect(await modified(page)).toBe('false');
  await call(page, 'set_node_prop', {nodeType:'scene', prop:'bgcolor', value:'red'});
  expect(await modified(page)).toBe('false');
  await load(page, 'atom.pdb', Buffer.from('HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n'));
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.objects.length)).toBe(1);
  expect(await modified(page)).toBe('true');
  await call(page, 'create_style_set', {name:'local'});
  await call(page, 'define_color', {styleSet:'local', name:'accent', color:'#00ff00'});
  await call(page, 'clear_undo');
  expect(await modified(page)).toBe('true');
  const cameras = JSON.parse((await call(page, 'list_cameras')).message);
  await page.evaluate(() => {
    window.originalDownloadURL = URL.createObjectURL;
    URL.createObjectURL = () => { throw new Error('download failure probe'); };
  });
  await page.keyboard.press('ControlOrMeta+s');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('download failure probe');
  expect(await modified(page)).toBe('true');
  expect(JSON.parse((await call(page, 'list_cameras')).message)).toEqual(cameras);
  await page.evaluate(() => { URL.createObjectURL = window.originalDownloadURL; });
  const downloaded = page.waitForEvent('download');
  await page.keyboard.press('ControlOrMeta+s');
  expect((await downloaded).suggestedFilename()).toBe('scene.qsc');
  expect(await modified(page)).toBe('false');
  await page.keyboard.press('ControlOrMeta+Alt+w');
  await expect.poll(() => page.evaluate(() => Object.keys(window.cuemol.status().ui.rects).filter(key => key.startsWith('Tab ')).length)).toBe(0);
});

import {test, expect} from '@playwright/test';
import {call} from './support.js';

const status = page => page.evaluate(() => window.cuemol?.status());
const timeline = async page => JSON.parse((await call(page, 'list_anims')).message);
async function click(page, key) {
  await expect.poll(async () => (await status(page))?.ui.rects[key]?.length).toBe(4);
  const [x,y,w,h] = (await status(page)).ui.rects[key];
  await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}
async function savedZoom(page) {
  const result = await call(page,'execute_script',{source:'print(scene.getCamera("__current").zoom);'});
  return Number(JSON.parse(result.message).log.trim());
}

for (const backend of ['webgl','webgpu']) {
  test(`Animation authoring ${backend} keeps its initial camera through undo and console edits`,async ({page}) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async () => (await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
    await call(page,'set_view',{zoom:87});
    await click(page,'Animation');
    await click(page,'Animation Add');
    await click(page,'Animation add wait');
    expect((await timeline(page)).mgr.startcam).toBe('__current');
    expect(await savedZoom(page)).toBe(87);
    await call(page,'undo');
    expect((await timeline(page)).elements).toHaveLength(0);
    expect((await timeline(page)).mgr.startcam).toBe('__current');
    expect(await savedZoom(page)).toBe(87);
    await call(page,'redo');
    await call(page,'set_view',{zoom:23});
    await call(page,'animate',{action:'seek',timeMs:0});
    expect((await status(page)).view.zoom).toBe(87);
    await call(page,'animate',{action:'stop'});
    await call(page,'create_scene',{name:'console animation'});
    await call(page,'set_view',{zoom:42});
    await call(page,'clear_undo');
    await click(page,'Console');
    await click(page,'Console input');
    await page.keyboard.insertText('add_anim wait');
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await status(page)).message).toBe('Console executed');
    expect((await timeline(page)).elements).toHaveLength(1);
    await call(page,'undo');
    expect((await timeline(page)).elements).toHaveLength(0);
    expect(await savedZoom(page)).toBe(42);
    await call(page,'redo');
    const rejected=await call(page,'reset_anim_prop',{element:'1',props:', ,'});
    expect(rejected.message).toContain('no properties to reset');
    expect((await timeline(page)).elements).toHaveLength(1);
  });
}

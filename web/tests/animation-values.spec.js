import {test, expect} from '@playwright/test';
import {call} from './support.js';

async function properties(page) {
  const result = await call(page, 'execute_script', {
    source: 'var a=scene.getAnimMgr().getAt(0);print(JSON.stringify({angle:a.angle,axis:a.axis.toString(),disabled:a.disabled}));',
  });
  return JSON.parse(JSON.parse(result.message).log.trim());
}

for (const backend of ['webgl', 'webgpu']) {
  test(`Animation property values ${backend} preserve types directions and redo`, async ({page}) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status().scene.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await call(page, 'add_anim', {type:'spin', name:'rotation'});
    await call(page, 'set_anim_prop', {element:'rotation', prop:'angle', value:'0x80'});
    await call(page, 'set_anim_prop', {element:'rotation', prop:'axis', value:'1 0 0'});
    await call(page, 'set_anim_prop', {element:'rotation', prop:'axis', value:'0 0.00001 0'});
    expect(await properties(page)).toEqual({angle:128, axis:'(1,0,0)', disabled:false});
    await call(page, 'clear_undo');
    await call(page, 'set_anim_prop', {element:'rotation', prop:'disabled', value:' ON '});
    expect((await properties(page)).disabled).toBe(true);
    await call(page, 'undo');
    const rejected = await call(page, 'set_anim_generic_prop', {element:'rotation', prop:'disabled', value:'1'});
    expect(rejected.message).toContain('invalid boolean');
    expect((await properties(page)).disabled).toBe(false);
    await call(page, 'redo');
    expect((await properties(page)).disabled).toBe(true);
    await call(page, 'set_anim_generic_prop', {element:'rotation', prop:'axis', value:'(0,0,0.00001)'});
    expect((await properties(page)).axis).toBe('(1,0,0)');
    expect(errors).toEqual([]);
  });
}

import {test, expect} from '@playwright/test';
import {load, call} from './support.js';
const pdb = Buffer.from('ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n');
async function click(page, name, options = {}) {
  await expect.poll(() => page.evaluate(name => window.cuemol.status().ui.rects[name], name)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const r = await page.evaluate(name => window.cuemol.status().ui.rects[name], name);
  await page.mouse.click(r[0]+r[2]/2, r[1]+r[3]/2, options);
}
async function fill(page, label, value, numeric = false) {
  await click(page, `Surface ${label}`, numeric ? {clickCount:2} : {});
  await page.keyboard.press('ControlOrMeta+A');
  if (value) await page.keyboard.type(value); else await page.keyboard.press('Backspace');
  if (numeric) await page.keyboard.press('Enter');
}
async function open(page, tool) {
  await click(page, 'Tools menu'); await click(page, `Mol surface ${tool}…`);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Surface window'])).toBeTruthy();
}
const closed = page => expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Surface window'])).toBeFalsy();
async function properties(page, id) {
  const state = await call(page, 'execute_script', {source:`var s=cuemol.getObject(${id});print(s.orig_den);print(s.orig_prad);print(s.orig_sel.toString());print(s.sesbackend);`});
  expect(state.message).not.toMatch(/^Error:/);
  const values = JSON.parse(state.message).log.trim().split('\n');
  return [Number(values[0]), Number(values[1]), ...values.slice(2)];
}
for (const backend of ['webgl', 'webgpu']) {
  test(`surface dialogs generate, regenerate and cut with rollback and history (${backend})`, async ({page}, testInfo) => {
    test.setTimeout(180000);
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'source.pdb', pdb);
    await call(page, 'clear_undo');
    await open(page, 'generation');
    await click(page, 'Surface Use selection');
    await click(page, 'Surface Choose selection');
    await click(page, 'Selection tab History');
    await click(page, 'Surface Choose selection');
    await fill(page, 'Selection', 'invalid (((');
    await click(page, 'Surface OK');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('syntax error');
    expect((await call(page, 'undo')).scene.objects).toHaveLength(1);
    await fill(page, 'Selection', 'chain A');
    await click(page, 'Surface Algorithm'); await click(page, 'Surface Algorithm rust');
    await fill(page, 'Probe radius', '1.8', true);
    await fill(page, 'Name', '');
    await page.screenshot({path:testInfo.outputPath('surface-generation.png')});
    await click(page, 'Surface OK'); await closed(page);
    let state = await page.evaluate(() => window.cuemol.status());
    let surface = state.scene.objects.find(object => object.className === 'MolSurfObj');
    expect(surface.name).toBe('sf_source.pdb');
    expect(await properties(page, surface.id)).toEqual([2,expect.closeTo(1.8, 9),'chain A','rust']);
    await call(page, 'undo');
    expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(1);
    await call(page, 'redo');
    await open(page, 'generation');
    await click(page, 'Surface OK'); await closed(page);
    state = await page.evaluate(() => window.cuemol.status());
    const second = state.scene.objects.find(object => object.name === 'sf_source.pdb(1)');
    expect(second).toBeTruthy();
    expect(await properties(page, second.id)).toEqual([2,expect.closeTo(1.4, 9),'','rust']);
    await call(page, 'undo');
    await click(page, `object:${surface.id}`, {button:'right'});
    await click(page, 'Regenerate surface…');
    await fill(page, 'Density', '3', true);
    await page.screenshot({path:testInfo.outputPath('surface-regenerate.png')});
    await click(page, 'Surface OK'); await closed(page);
    expect(await properties(page, surface.id)).toEqual([3,expect.closeTo(1.8, 9),'chain A','rust']);
    await call(page, 'undo'); expect((await properties(page, surface.id))[0]).toBe(2);
    await call(page, 'redo'); expect((await properties(page, surface.id))[0]).toBe(3);
    await call(page, 'set_view', {center:[0,0,-120],slab:240});
    await open(page, 'cutter');
    await click(page, 'Surface Mode'); await click(page, 'Surface Mode separate');
    await page.screenshot({path:testInfo.outputPath('surface-cut.png')});
    await click(page, 'Surface OK'); await closed(page);
    state = await page.evaluate(() => window.cuemol.status());
    expect(state.scene.objects.some(object => object.name === `sect_${surface.name}`)).toBe(true);
    await call(page, 'undo');
    expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(2);
    await call(page, 'redo');
    expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(3);
    await open(page, 'cutter'); await click(page, 'Surface Cancel'); await closed(page);
    await open(page, 'generation'); await call(page, 'create_scene', {name:'Other'}); await closed(page);
  });
}

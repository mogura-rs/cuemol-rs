import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

function molecule(offset, pivot = 'CA') {
  return Buffer.from(Array.from({length:9}, (_, i) => {
    const t = i === 8 ? 1000 : i;
    const coordinates = [t + offset, Math.sin(t), Math.cos(t)].map(value => value.toFixed(3).padStart(8)).join('');
    return `ATOM  ${String(i + 1).padStart(5)} ${pivot.padStart(4)} ${(pivot === 'P' ? 'A' : 'ALA').padStart(3)} A${String(i + 1).padStart(4)}    ${coordinates}  1.00 10.00           ${pivot === 'P' ? 'P' : 'C'}  \n`;
  }).join('') + 'END\n');
}
async function click(page, name) {
  await expect.poll(() => page.evaluate(name => window.cuemol.status().ui.rects[name], name)).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const r = await page.evaluate(name => window.cuemol.status().ui.rects[name], name);
  await page.mouse.click(r[0] + r[2] / 2, r[1] + r[3] / 2);
}
async function open(page) {
  await click(page, 'Tools menu');
  await click(page, 'Molecular superposition…');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Superpose window'])).toBeTruthy();
}
async function fill(page, label, value) {
  await click(page, `Superpose ${label}`);
  await page.keyboard.press('ControlOrMeta+A');
  if (value) await page.keyboard.type(value); else await page.keyboard.press('Backspace');
}
const closed = page => expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Superpose window'])).toBeFalsy();
async function matrixX(page, id) {
  const result = await call(page, 'execute_script', {source:`print(cuemol.getObject(${id}).xformMat.getAt(1,4));`});
  expect(result.message).not.toMatch(/^Error:/);
  return Number(JSON.parse(result.message).log.trim());
}
async function history(page) {
  return page.evaluate(() => {
    const line = localStorage.getItem('cuemol.config').split('\n').find(line => line.startsWith('cuemol2.ui.histories.mol_superpose\t'));
    return JSON.parse(line.slice(line.indexOf('\t') + 1));
  });
}
for (const backend of ['webgl', 'webgpu']) {
  test(`superposition dialog keeps errors, pivots, selection framing and successful history (${backend})`, async ({page}, testInfo) => {
    test.setTimeout(180000);
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    await load(page, 'ref.pdb', molecule(0));
    await load(page, 'mov.pdb', molecule(20));
    const [reference, moving] = (await call(page, 'clear_undo')).scene.objects;
    await open(page);
    await click(page, 'Superpose Reference choose selection');
    await click(page, 'Selection tab History');
    await click(page, 'Superpose Reference choose selection');
    await fill(page, 'Reference selection', 'invalid (((');
    await click(page, 'Superpose OK');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('syntax error');
    expect(await matrixX(page, moving.id)).toBe(0);
    expect((await call(page, 'undo')).scene.objects).toHaveLength(2);
    expect(await matrixX(page, moving.id)).toBe(0);
    await fill(page, 'Reference selection', 'resi 1:8');
    await fill(page, 'Moving selection', 'resi 1:8');
    await click(page, 'Superpose Pivot atom'); await click(page, 'Superpose Pivot CA');
    await click(page, 'Superpose Use property');
    await page.screenshot({path:testInfo.outputPath('superpose-options.png')});
    await click(page, 'Superpose OK'); await closed(page);
    let state = await page.evaluate(() => window.cuemol.status());
    expect(state.view.center[0]).toBeCloseTo(3.5, 4);
    expect(state.view.zoom).toBeLessThan(100);
    expect(await matrixX(page, moving.id)).toBeCloseTo(-20, 5);
    expect(await history(page)).toEqual({refObjId:reference.id,movObjId:moving.id,algo:'USalign',pivot:'CA',autoRecenter:true,useprop:true});
    await call(page, 'undo'); expect(await matrixX(page, moving.id)).toBeCloseTo(0, 5);
    await call(page, 'redo'); expect(await matrixX(page, moving.id)).toBeCloseTo(-20, 5);
    await open(page);
    await click(page, 'Superpose OK'); await closed(page);
    state = await page.evaluate(() => window.cuemol.status());
    expect(state.view.center[0]).toBeCloseTo(500, 3);
    await open(page);
    await click(page, 'Superpose Method LSQ');
    await click(page, 'Superpose Reference molecule');
    await click(page, 'Superpose Reference molecule mov.pdb');
    await fill(page, 'Reference selection', 'resi 1:3');
    await fill(page, 'Moving selection', 'resi 4:6');
    await click(page, 'Superpose Auto recenter');
    await click(page, 'Superpose OK'); await closed(page);
    const saved = await history(page);
    expect(saved).toMatchObject({refObjId:moving.id,movObjId:moving.id,algo:'LSQ',autoRecenter:false});
    await open(page); await click(page, 'Superpose Method SSM'); await click(page, 'Superpose Cancel'); await closed(page);
    expect(await history(page)).toEqual(saved);
    await open(page); await call(page, 'create_scene', {name:'Other scene'}); await closed(page);
    await load(page, 'rna-ref.pdb', molecule(0, 'P'));
    await load(page, 'rna-mov.pdb', molecule(20, 'P'));
    const rnaMoving = (await call(page, 'clear_undo')).scene.objects[1].id;
    await open(page); await click(page, 'Superpose Method USalign');
    await click(page, 'Superpose Pivot atom'); await click(page, 'Superpose Pivot P');
    await click(page, 'Superpose OK'); await closed(page);
    expect(await matrixX(page, rnaMoving)).toBeCloseTo(-20, 4);
    expect((await history(page)).pivot).toBe('P');
    await page.screenshot({path:testInfo.outputPath('superpose-result.png')});
  });
}

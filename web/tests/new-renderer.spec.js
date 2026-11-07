import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = Buffer.from('ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  ALA A   2     200.000   0.000   0.000  1.00 10.00           C\nEND\n');
const dx = Buffer.from('object 1 class gridpositions counts 2 2 2\norigin 10 20 30\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 2 2 2\nobject 3 class array type double rank 0 items 8 data follows\n0 1 2 3 4 5 6 7\n');
const frame = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
const status = page => page.evaluate(() => window.cuemol?.status());
async function click(page, name, button = 'left') {
  await expect.poll(async () => (await status(page)).ui.rects[name]).toBeTruthy();
  await frame(page);
  const r = (await status(page)).ui.rects[name];
  await page.mouse.click(r[0]+r[2]/2, r[1]+r[3]/2, {button});
  await frame(page);
}
async function fill(page, field, value) {
  await click(page, `New renderer ${field}`);
  await page.keyboard.press('ControlOrMeta+A');
  if (value) await page.keyboard.type(value);
  else await page.keyboard.press('Backspace');
  await frame(page);
}
async function open(page, id, kind = 'object') {
  await click(page, `${kind}:${id}`, 'right');
  await click(page, 'Workspace New Renderer');
  await expect.poll(async () => (await status(page)).ui.rects['New renderer Create']).toBeTruthy();
  await frame(page);
}
async function choose(page, type) {
  await click(page, 'New renderer Type');
  const key = `New renderer Type ${type}`;
  await expect.poll(async () => (await status(page)).ui.rects[key]).toBeTruthy();
  let r = (await status(page)).ui.rects[key];
  const combo = (await status(page)).ui.rects['New renderer Type'];
  if (r[1] > combo[1] + 300) {
    await page.mouse.move(combo[0]+100,combo[1]+150);
    await page.mouse.wheel(0, r[1]-combo[1]-220);
    await frame(page);
  }
  await click(page, key);
}
async function create(page, name) {
  await click(page, 'New renderer Create');
  await expect.poll(async () => (await status(page)).message).toBe(`Created renderer ${name}`);
  await expect.poll(async () => (await status(page)).ui.rects['New renderer Create']).toBeUndefined();
  return status(page);
}

for (const backend of ['webgl','webgpu']) {
  test(`new renderer preserves input, group membership and one-step history (${backend})`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async () => (await status(page))?.scene?.sceneId, {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'molecule.pdb', pdb);
    const object = (await status(page)).scene.objects[0].id;
    await call(page, 'set_mol_selection', {objId:object, selection:'aid 1'});
    const before = await call(page, 'clear_undo');
    await open(page, object);
    await click(page, 'New renderer Choose selection');
    await click(page, 'Selection tab History');
    await click(page, 'New renderer Choose selection');
    await choose(page, 'ballstick');
    await fill(page, 'Name', 'chosen');
    await choose(page, 'cpk');
    await fill(page, 'Object name', 'ignored rename');
    await fill(page, 'Selection', 'invalid (((');
    await click(page, 'New renderer Create');
    await expect.poll(async () => (await status(page)).message).toMatch(/selection|invalid|unexpected/i);
    expect((await status(page)).scene.objects).toEqual(before.scene.objects);
    expect((await status(page)).view).toEqual(before.view);
    expect((await status(page)).ui.rects['New renderer Create']).toBeTruthy();
    await page.screenshot({path:testInfo.outputPath('new-renderer-error.png')});
    await fill(page, 'Selection', 'aid 1');
    let state = await create(page, 'chosen');
    let renderer = state.scene.objects[0].renderers.find(r=>r.name==='chosen');
    expect(renderer.type).toBe('cpk');
    expect(state.scene.objects[0].name).toBe('molecule.pdb');
    expect(state.view.center[0]).toBeCloseTo(0, 4);
    const detail = await call(page, 'execute_script', {source:`print(cuemol.getRenderer(${renderer.id}).sel.toString());`});
    expect(detail.message).toContain('aid 1');
    state = await call(page, 'undo');
    expect(state.scene.objects).toEqual(before.scene.objects);
    state = await call(page, 'redo');
    expect(state.scene.objects[0].renderers.some(r=>r.name==='chosen')).toBe(true);
    await open(page, object);
    await choose(page, 'ballstick');
    await click(page, 'New renderer Cancel');
    const history = await page.evaluate(() => localStorage.getItem('cuemol.config'));
    expect(history).toContain('cpk');
    await open(page, object);
    await choose(page, 'ballstick');
    await fill(page, 'Name', 'temporary');
    await fill(page, 'Name', '');
    await create(page, 'ballstick1');
    await call(page, 'create_group', {objId:object,name:'bundle'});
    state = await call(page, 'clear_undo');
    const group = state.scene.objects[0].renderers.find(r=>r.name==='bundle').id;
    await open(page, group, 'renderer');
    await click(page, 'New renderer Type');
    const types = Object.keys((await status(page)).ui.rects).filter(k=>k.startsWith('New renderer Type '));
    expect(types.some(k=>k.includes('Preset'))).toBe(false);
    await click(page, 'New renderer Type simple');
    await fill(page, 'Name', 'grouped');
    await page.screenshot({path:testInfo.outputPath('new-renderer-group.png')});
    const grouped = await create(page, 'grouped');
    expect(grouped.scene.objects[0].renderers.find(r=>r.name==='grouped').group).toBe('bundle');
    expect((await call(page, 'undo')).scene.objects).toEqual(state.scene.objects);
    await call(page, 'redo');
    await open(page, object);
    await call(page, 'create_scene');
    await expect.poll(async () => (await status(page)).ui.rects['New renderer Create']).toBeUndefined();
    expect((await status(page)).scene.objects).toHaveLength(0);
  });
}

test('new map and surface renderers use class options and map centering', async ({page}, testInfo) => {
  await page.goto('./?renderer=webgl');
  await expect.poll(async () => (await status(page))?.scene?.sceneId, {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'potential.dx', dx);
  let state = await status(page);
  const map = state.scene.objects[0].id;
  const view = state.view;
  await open(page, map);
  expect((await status(page)).ui.rects['New renderer Selection']).toBeUndefined();
  await choose(page, 'isosurf');
  await click(page, 'New renderer Set map center (keep the view)');
  state = await create(page, 'isosurf1');
  expect(state.view).toEqual(view);
  await open(page, map);
  await click(page, 'New renderer Move view center (fit the map)');
  await page.screenshot({path:testInfo.outputPath('new-renderer-map.png')});
  state = await create(page, 'isosurf2');
  expect(state.view.center).toEqual([11,21,31]);
  await call(page, 'create_group', {objId:map,name:'map group'});
  state = await status(page);
  const group = state.scene.objects[0].rendererGroups.find(g=>g.name==='map group').id;
  await open(page, group, 'renderer');
  await fill(page, 'Name', 'map grouped');
  state = await create(page, 'map grouped');
  expect(state.scene.objects[0].renderers.find(r=>r.name==='map grouped').group).toBe('map group');
  const ply = Buffer.from('ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n');
  await load(page, 'triangle.ply', ply);
  state = await status(page);
  const surface = state.scene.objects.find(o=>o.name==='triangle.ply').id;
  await open(page, surface);
  await fill(page, 'Name', 'surface second');
  state = await create(page, 'surface second');
  expect(state.scene.objects.find(o=>o.id===surface).renderers.find(r=>r.name==='surface second').type).toBe('molsurf');
});

test('new renderer presets ignore the disabled selector and undo as one group', async ({page}, testInfo) => {
  await page.goto('./?renderer=webgl');
  await expect.poll(async () => (await status(page))?.scene?.sceneId, {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'preset.pdb', pdb);
  const object = (await status(page)).scene.objects[0].id;
  await call(page, 'set_mol_selection', {objId:object,selection:'aid 1'});
  const before = await call(page, 'clear_undo');
  const types = JSON.parse((await call(page, 'list_renderer_types', {objId:object})).message);
  await open(page, object);
  await fill(page, 'Selection', 'invalid (((');
  await choose(page, types.presetTypes[0]);
  await fill(page, 'Name', 'preset group');
  await page.screenshot({path:testInfo.outputPath('new-renderer-preset.png')});
  let state = await create(page, 'preset group');
  expect(state.scene.objects[0].renderers.find(r=>r.name==='preset group').type).toBe('*group');
  expect(state.scene.objects[0].renderers.filter(r=>r.group==='preset group').length).toBeGreaterThan(0);
  state = await call(page, 'undo');
  expect(state.scene.objects).toEqual(before.scene.objects);
  await call(page, 'redo');
  await open(page, object);
  await fill(page, 'Name', 'remembered preset');
  state = await create(page, 'remembered preset');
  expect(state.scene.objects[0].renderers.find(r=>r.name==='remembered preset').type).toBe('*group');
});

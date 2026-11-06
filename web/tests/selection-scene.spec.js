import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';

const pdb = `HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00  0.00           C
HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00  0.00           O
HETATM    3  N1  LIG A   1       1.000   1.500   0.000  1.00  0.00           N
END
`;
const status = page => page.evaluate(() => window.cuemol.status());
async function command(page, name, args = {}) {
  const id = await page.evaluate(([name, args]) => window.cuemol.command(name, args), [name, args]);
  await expect.poll(async () => {
    const state = await status(page);
    return state.lastCommand === id && !state.busy;
  }).toBe(true);
  const message = (await status(page)).message;
  expect(message).not.toMatch(/^Error:/);
  try { return JSON.parse(message); } catch { return message; }
}

for (const backend of ['webgl', 'webgpu']) {
  test(`cross-molecule selection updates ${backend} geometry and picking after target movement`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    for (const [index, name] of ['Probe', 'Target'].entries()) {
      await page.locator('#files').setInputFiles({name:`${name}.pdb`, mimeType:'application/octet-stream', buffer:Buffer.from(pdb)});
      await expect.poll(async () => (await status(page)).scene.objects.length).toBe(index + 1);
    }
    const [probe, target] = (await status(page)).scene.objects;
    await command(page, 'rename_node', {nodeType:'object', nodeId:target.id, name:'Target'});
    await command(page, 'set_visible', {nodeType:'object', nodeId:target.id, visible:false});
    for (const renderer of probe.renderers) await command(page, 'delete_node', {nodeType:'renderer', nodeId:renderer.id});
    const expression = 'name C1 around 2.1 [Target]';
    const renderer = await command(page, 'create_renderer', {objId:probe.id, rendererType:'cpk', name:'cross', selection:expression});
    await command(page, 'set_projection', {perspective:false});
    await command(page, 'set_view', {center:[-1,0,0], zoom:12, distance:200, slab:50});
    const count = async () => (await command(page, 'count_selection', {objId:probe.id, selection:expression})).count;
    expect(await count()).toBe(2);
    const [x,y,w,h] = (await status(page)).ui.rects.Viewport;
    const point = [x + w / 2 + h / 12, y + h / 2];
    await page.mouse.move(...point);
    await expect.poll(async () => (await status(page)).ui.navigation.hover?.index).toBe(1);
    await page.mouse.move(5, 5);
    const clip = {x:Math.floor(x + w / 2 - 140), y:Math.floor(y + h / 2 - 100), width:280, height:200};
    const capture = async name => PNG.sync.read(await page.screenshot({clip, path:testInfo.outputPath(name)})).data;
    const before = await capture('cross-selected.png');
    const shift = async value => command(page, 'execute_script', {source:`var offset = cuemol.createObj('Vector'); offset.x = ${value}; var matrix = cuemol.createObj('Matrix'); matrix.setTranslate(offset); cuemol.getObject(${target.id}).xformMat = matrix;`});
    await shift(20);
    expect(await count()).toBe(0);
    await page.mouse.move(...point);
    await expect.poll(async () => (await status(page)).ui.navigation.hover).toBeNull();
    await page.mouse.move(5, 5);
    const empty = await capture('cross-target-moved.png');
    expect(empty.equals(before)).toBe(false);
    await shift(0);
    expect(await count()).toBe(2);
    const restored = await capture('cross-restored.png');
    expect(restored.equals(before)).toBe(true);

    const definition = await command(page, 'define_selection', {styleSet:'selection-check', name:'temporarySelection', selection:'name C1'});
    expect(definition.name).toBe('temporarySelection');
    const changed = await command(page, 'set_node_prop', {nodeType:'renderer', nodeId:renderer.rendererId, prop:'sel', value:'temporarySelection'});
    expect(changed.value).toBe('temporarySelection');
    const removed = await command(page, 'remove_selection', {styleSet:'selection-check', name:'temporarySelection'});
    expect(removed.name).toBe('temporarySelection');
    const properties = await command(page, 'list_node_props', {nodeType:'renderer', nodeId:renderer.rendererId});
    expect(properties.properties.find(row => row.key === 'sel').value).toBe('temporarySelection');
    await command(page, 'set_visible', {nodeType:'object', nodeId:target.id, visible:true});
    await page.mouse.move(...point);
    await expect.poll(async () => (await status(page)).ui.navigation.hover).toEqual({object:target.id, index:1});
    await page.mouse.move(5, 5);
    await command(page, 'set_visible', {nodeType:'object', nodeId:target.id, visible:false});
    const missing = await capture('cross-missing-selection.png');
    expect(missing.equals(empty)).toBe(true);
  });
}

for (const backend of ['webgl', 'webgpu']) {
  test(`selection rewriting and fit-box APIs work in ${backend}`, async ({page}) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await page.locator('#files').setInputFiles({name:'Probe.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
    await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
    const object=(await status(page)).scene.objects[0];
    const result=await command(page,'execute_script',{source:`
      var mol=cuemol.getObject(${object.id});
      var view=scene.getView(scene.activeViewID);
      var selection=cuemol.makeSel('aid 1 or aid 2 or aid 1');
      print(selection.simplify());
      print(selection.toString());
      selection.compile('not chain A or not chain B',scene.uid);
      print(selection.invertSimplified());
      selection.compile('byres resn ALA around 3',scene.uid);
      print(selection.rewriteAround(5,false));
      view.rotQuat=cuemol.Quat();
      var bounds=JSON.parse(mol.getFitBoxJSON(view,cuemol.makeSel('aid 1')));
      print(JSON.stringify(bounds));
      view.fitToBox(cuemol.Vector('('+bounds.min.join(',')+')'),cuemol.Vector('('+bounds.max.join(',')+')'));
      print([view.center.x,view.center.y,view.center.z,view.slab].join(','));
    `});
    const lines=result.log.trim().split('\n');
    expect(lines.slice(0,4)).toEqual(['aid 1:2','aid 1 or aid 2 or aid 1','not (not chain A or not chain B)','(resn ALA) around 5.0']);
    expect(JSON.parse(lines[4])).toEqual({min:[-2,0,0],max:[-2,0,0]});
    expect(lines[5]).toBe('-2,0,0,4');
    expect((await command(page,'count_selection',{objId:object.id,selection:lines[0]})).count).toBe(2);
  });
}


for (const backend of ['webgl', 'webgpu']) {
  test(`Workspace Focus frames a renderer and another object in ${backend}`, async ({page}) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    for (const name of ['Probe','Target']) {
      await page.locator('#files').setInputFiles({name:`${name}.pdb`,mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
      await expect.poll(async () => (await status(page)).scene.objects.length).toBe(name === 'Probe' ? 1 : 2);
    }
    const [probe, target] = (await status(page)).scene.objects;
    const renderer = await command(page, 'create_renderer', {objId:probe.id,rendererType:'cpk',name:'focus_first',selection:'name C1'});
    await command(page, 'execute_script', {source:`var offset=cuemol.Vector('(20,0,0)'); var matrix=cuemol.createObj('Matrix'); matrix.setTranslate(offset); cuemol.getObject(${target.id}).xformMat=matrix;`});
    await command(page, 'set_view', {rotation:[1,0,0,0],center:[100,100,100],zoom:200,slab:200});
    const click = async (name, modifiers=[]) => {
      await expect.poll(async () => Boolean((await status(page)).ui.rects[name])).toBe(true);
      const [x,y,w,h] = (await status(page)).ui.rects[name];
      for (const key of modifiers) await page.keyboard.down(key);
      await page.mouse.click(x+w/2,y+h/2);
      for (const key of modifiers.toReversed()) await page.keyboard.up(key);
    };
    await click(`renderer:${renderer.rendererId}`);
    await click(`object:${target.id}`, ['ControlOrMeta']);
    await click('Workspace Focus');
    await expect.poll(async () => (await status(page)).view.center[0]).toBeCloseTo(9.8, 5);
    const state = await status(page);
    const [, , width, height] = state.ui.rects.Viewport;
    expect(state.view.center[1]).toBeCloseTo(0.75,5);
    expect(state.view.zoom).toBeCloseTo(Math.max(23.6/(Math.round(width)/Math.round(height)),4),4);
    expect(state.view.slab).toBeCloseTo(4,5);
  });
}

for (const backend of ['webgl', 'webgpu']) {
  test(`camera rotation and slab fitting preserve scene redo in ${backend}`, async ({page}) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await page.locator('#files').setInputFiles({name:'Probe.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
    await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
    const object = (await status(page)).scene.objects[0];
    await command(page, 'rename_node', {nodeType:'object',nodeId:object.id,name:'renamed'});
    await command(page, 'undo');
    await command(page, 'set_view', {rotation:[1,0,0,0],center:[0,0,30],distance:200,zoom:37});
    await command(page, 'set_view', {fitSlab:true,objId:object.id});
    const fitted = (await status(page)).view;
    expect(fitted.center).toEqual([0,0,30]);
    expect(fitted.zoom).toBe(37);
    expect(fitted.slab).toBeCloseTo(66,5);
    const q = Math.SQRT1_2;
    await command(page, 'set_view', {rotation:[q,0,q,0],center:[0,0,0],fitSlab:true});
    expect((await status(page)).view.slab).toBeCloseTo(66,5);
    await command(page, 'set_view', {fitSlab:true});
    expect((await status(page)).view.slab).toBeCloseTo(4.4,5);
    await command(page, 'redo');
    expect((await status(page)).scene.objects[0].name).toBe('renamed');
  });
}

import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';

const pdb = `HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00  0.00           C
HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00  0.00           O
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
  try { return JSON.parse(message); } catch (cause) { throw new Error(`${name}: ${message}`, {cause}); }
}

for (const backend of ['webgl', 'webgpu']) {
  test(`script and GUI styles share ${backend} selection, colors and geometry`, async ({page}, testInfo) => {
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    expect(await page.evaluate(() => window.cuemol.backend)).toBe(backend);
    await page.locator('#files').setInputFiles({name:'styles.pdb', mimeType:'application/octet-stream', buffer:Buffer.from(pdb)});
    await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
    const object = (await status(page)).scene.objects[0];
    for (const renderer of object.renderers) await command(page, 'delete_node', {nodeType:'renderer', nodeId:renderer.id});
    const created = await command(page, 'create_renderer', {objId:object.id, rendererType:'ballstick', name:'styled'});
    expect(created.rendererId).toBeGreaterThan(0);
    const renderer = created.rendererId;
    await command(page, 'set_projection', {perspective:false});
    await command(page, 'set_view', {center:[-1,0,0], zoom:12, distance:200, slab:50});
    await command(page, 'set_visible', {nodeType:'renderer', nodeId:renderer, visible:false});
    const [x,y,w,h] = (await status(page)).ui.rects.Viewport;
    const clip = {x:Math.floor(x + w / 2 - 140), y:Math.floor(y + h / 2 - 100), width:280, height:200};
    const capture = async name => {
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      return PNG.sync.read(await page.screenshot({clip, path:testInfo.outputPath(name)})).data;
    };
    const empty = await capture('styles-empty.png');
    const result = await command(page, 'execute_script', {source:"var styles=cuemol.getService('StyleManager'), scope=scene.uid; var id=styles.createStyleSet('user',scope); styles.setStrData('sel','focus','name C1',scope,id); styles.setColor('accent',cuemol.makeColor('#1234ee'),scope,id); styles.setStyleValue(scope,'user','Custom.sphr','0.9'); styles.setStyleValue(scope,'user','Custom.defaultcolor','accent'); print(id);"});
    const styleId = Number(result.log.trim());
    expect(styleId).toBeGreaterThan(0);
    const applied = await command(page, 'execute_script', {source:`var renderer=cuemol.getObject(${object.id}).getRendererByName('styled'); renderer.applyStyles('Custom'); print(renderer.defaultcolor.r());`});
    expect(applied.log).toBe('18\n');
    expect((await command(page, 'set_node_prop', {nodeType:'renderer', nodeId:renderer, prop:'sel', value:'focus'})).value).toBe('focus');
    await command(page, 'set_visible', {nodeType:'renderer', nodeId:renderer, visible:true});
    expect((await command(page, 'count_selection', {objId:object.id, selection:'focus'})).count).toBe(1);
    const blue = await capture('styles-blue.png');
    expect(blue.equals(empty)).toBe(false);
    expect((await command(page, 'define_color', {styleSet:'user', name:'accent', color:'#22cc44'})).name).toBe('accent');
    const green = await capture('styles-green.png');
    expect(green.equals(blue)).toBe(false);
    expect((await command(page, 'define_selection', {styleSet:'user', name:'focus', selection:'name O1'})).name).toBe('focus');
    const read = await command(page, 'execute_script', {source:"var styles=cuemol.getService('StyleManager'); print(styles.hasStyleSet('user',scene.uid)); print(styles.getStrData('sel','focus',scene.uid));"});
    expect(read.log).toBe(`${styleId}\nname O1\n`);
    await page.mouse.move(x + w / 2 + h / 12, y + h / 2);
    await expect.poll(async () => (await status(page)).ui.navigation.hover).toEqual({object:object.id, index:1});
    await page.mouse.move(5, 5);
    const oxygen = await capture('styles-oxygen.png');
    expect(oxygen.equals(green)).toBe(false);
    const removed = await command(page, 'execute_script', {source:`print(cuemol.getService('StyleManager').removeStrData('sel','focus',scene.uid,${styleId}));`});
    expect(removed.log).toBe('true\n');
    const after = await capture('styles-removed.png');
    expect(after.equals(empty)).toBe(true);
  });
}

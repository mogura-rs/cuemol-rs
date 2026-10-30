import {test, expect} from '@playwright/test';

const pdb = `HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C
HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O
END
`;
const status = page => page.evaluate(() => window.cuemol.status());
async function command(page, name, args = {}) {
  const id = await page.evaluate(([name,args]) => window.cuemol.command(name,args), [name,args]);
  await expect.poll(async () => (await status(page)).lastCommand === id && !(await status(page)).busy).toBe(true);
  const message = (await status(page)).message;
  try { return JSON.parse(message); } catch { return message; }
}
async function click(page, name, inspector = true, double = false) {
  await expect.poll(async () => (await status(page)).ui[inspector ? 'inspector' : 'rects'][name]?.length).toBe(4);
  const [x,y,w,h] = (await status(page)).ui[inspector ? 'inspector' : 'rects'][name];
  if (double) await page.mouse.dblclick(x+w/2,y+h/2); else await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}
async function filter(page, text) {
  await click(page, 'Filter properties');
  await expect(page.locator('input[type="text"]')).toBeFocused();
  await page.keyboard.press('ControlOrMeta+a');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  if (text) await page.keyboard.insertText(text); else await page.keyboard.press('Backspace');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.keyboard.press('Tab');
}
async function setup(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await page.locator('#files').setInputFiles({name:'inspector.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  const object = (await status(page)).scene.objects[0];
  const created = await command(page,'create_renderer',{objId:object.id,rendererType:'ribbon',name:'Properties'});
  await click(page, `object:${object.id}`, false);
  await click(page, `renderer:${created.rendererId}`, false);
  await click(page,'Generic tab');
  await command(page,'clear_undo');
  return {object, id:created.rendererId};
}
async function prop(page,id,key) {
  return (await command(page,'get_prop',{path:`#${id}.${key}`})).value;
}

test('Inspector edits typed values and resets them in one undo step', async ({page},testInfo) => {
  const {id} = await setup(page);
  await filter(page,'alpha');
  await click(page,'alpha',true,true);
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.type('0.35'); await page.keyboard.press('Enter');
  await expect.poll(async () => prop(page,id,'alpha')).toBe('0.35');
  await filter(page,'egtype');
  await click(page,'egtype');
  await click(page,'egtype option silhouette');
  await expect.poll(async () => prop(page,id,'egtype')).toBe('silhouette');
  await click(page,'Reset all to default');
  await expect.poll(async () => prop(page,id,'alpha')).toBe('1');
  expect(await prop(page,id,'egtype')).toBe('none');
  expect(await prop(page,id,'name')).toBe('Properties');
  await command(page,'undo');
  expect(await prop(page,id,'alpha')).toBe('0.35');
  expect(await prop(page,id,'egtype')).toBe('silhouette');
  await page.screenshot({path:testInfo.outputPath('inspector-typed-reset.png')});
  await command(page,'set_node_prop',{nodeType:'view',prop:'zoom',value:'25'});
  await command(page,'reset_node_props',{nodeType:'scene'});
  await command(page,'redo');
  expect(await prop(page,id,'alpha')).toBe('1');
});

test('Inspector exposes nested enum defaults and keeps identity read-only', async ({page},testInfo) => {
  const {id} = await setup(page);
  await filter(page,'helixhead.type');
  await click(page,'helixhead.type');
  await click(page,'helixhead.type option flat');
  await expect.poll(async () => prop(page,id,'helixhead.type')).toBe('flat');
  await click(page,'Reset helixhead.type');
  const rows = (await command(page,'list_node_props',{nodeType:'renderer',nodeId:id})).properties;
  expect(rows.find(row=>row.key==='helixhead.type').isdefault).toBe(true);
  await filter(page,'uid');
  await click(page,'uid',true,true);
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.type('999999'); await page.keyboard.press('Enter');
  expect(await prop(page,id,'uid')).toBe(String(id));
  await filter(page,'helix');
  await page.screenshot({path:testInfo.outputPath('inspector-nested.png')});
});

test('Scene presets use one undo and View fields follow viewport navigation', async ({page,context},testInfo) => {
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await setup(page);
  await click(page,'Scene menu',false);
  await click(page,'Scene Properties',false);
  await click(page,'name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Inspector scene');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await command(page,'get_prop',{path:'name'})).value).toBe('Inspector scene');
  await click(page,'aoEnabled');
  await command(page,'clear_undo');
  await click(page,'ao-preset');
  await click(page,'ao-preset option High');
  await expect.poll(async () => (await command(page,'get_prop',{path:'aoRadius'})).value).toBe('12');
  expect((await command(page,'get_prop',{path:'aoSteps'})).value).toBe('5');
  await command(page,'undo');
  expect((await command(page,'get_prop',{path:'aoRadius'})).value).toBe('4');
  expect((await command(page,'get_prop',{path:'aoSteps'})).value).toBe('3');
  await command(page,'redo');
  await page.screenshot({path:testInfo.outputPath('inspector-scene-presets.png')});
  await click(page,'Generic tab');
  await filter(page,'name');
  await click(page,'name');
  await page.keyboard.insertText('forbidden');
  await page.keyboard.press('Enter');
  expect((await command(page,'get_prop',{path:'name'})).value).toBe('Inspector scene');
  await click(page,'View menu',false);
  await click(page,'View Properties',false);
  await filter(page,'zoom');
  const oldZoom=(await status(page)).view.zoom;
  const viewport=(await status(page)).ui.rects.Viewport;
  expect(viewport).toHaveLength(4);
  await page.mouse.move(viewport[0]+viewport[2]/2,viewport[1]+viewport[3]/2);
  await page.mouse.wheel(0,-180);
  await expect.poll(async () => (await status(page)).view.zoom).not.toBe(oldZoom);
  await click(page,'zoom',true,true);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toMatch(/^\d/);
  const value=await page.evaluate(()=>navigator.clipboard.readText());
  expect(Number(value)).toBeCloseTo((await status(page)).view.zoom,1);
  await page.keyboard.type('37.25');
  const beforeDraftNavigation=(await status(page)).view.zoom;
  await page.mouse.move(viewport[0]+viewport[2]/2,viewport[1]+viewport[3]/2);
  await page.mouse.wheel(0,180);
  await expect.poll(async () => (await status(page)).view.zoom).not.toBe(beforeDraftNavigation);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('37.25');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBeCloseTo(37.25,2);
  await page.keyboard.press('Escape');
  await filter(page,'name');
  await click(page,'name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Orbit view');
  const beforeNameNavigation=(await status(page)).view.zoom;
  await page.mouse.move(viewport[0]+viewport[2]/2,viewport[1]+viewport[3]/2);
  await page.mouse.wheel(0,-180);
  await expect.poll(async () => (await status(page)).view.zoom).not.toBe(beforeNameNavigation);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('Orbit view');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await command(page,'get_prop',{path:'view.name'})).value).toBe('Orbit view');
  await click(page,'name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Obsolete draft');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('Obsolete draft');
  const renamed=await command(page,'set_node_prop',{nodeType:'view',prop:'name',value:'External rename'});
  expect(renamed.value).toBe('External rename');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('External rename');
  await page.screenshot({path:testInfo.outputPath('inspector-scene-view.png')});
});

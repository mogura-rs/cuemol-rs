import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = 'ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n';
const status = page => page.evaluate(() => window.cuemol?.status());
const settle = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function click(page, key) {
  await expect.poll(async () => Boolean((await status(page)).ui.rects[key])).toBe(true);
  await settle(page);
  const [x,y,w,h] = (await status(page)).ui.rects[key];
  await page.mouse.click(x+w/2,y+h/2);
  await settle(page);
}
async function enter(page, key, value) {
  await click(page,key);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(value);
  await page.keyboard.press('Enter');
  await settle(page);
}
async function crystal(page, id) {
  const result = await call(page,'execute_script',{source:`var c=cuemol.getObject(${id}).getExtData('CrystalInfo');print(JSON.stringify(c ? [c.a,c.b,c.c,c.alpha,c.beta,c.gamma,c.nsg] : null));`});
  return JSON.parse(JSON.parse(result.message).log.trim());
}
function mapBytes() {
  const bytes = Buffer.alloc(1024+64*4);
  const word = (index,value) => bytes.writeInt32LE(value,index*4);
  const real = (index,value) => bytes.writeFloatLE(value,index*4);
  for(let axis=0;axis<3;axis++) {word(axis,4);word(7+axis,4);real(10+axis,8);real(13+axis,90);word(16+axis,axis+1);}
  word(3,2);word(22,1);real(19,0);real(20,63);real(21,31.5);real(54,18.5);
  bytes.write('MAP ',208);
  for(let index=0;index<64;index++)bytes.writeFloatLE(index,1024+index*4);
  return bytes;
}

for(const backend of ['webgl','webgpu']) {
  test(`Symmetry pane ${backend} creates constrained cells and keeps one undo step`, async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async()=>Boolean((await status(page))?.scene.sceneId),{timeout:60000}).toBe(true);
    await load(page,'probe.pdb',Buffer.from(pdb));
    await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
    const id=(await status(page)).scene.objects[0].id;
    await click(page,'Crystal');
    await click(page,'Symmetry unit cell');
    expect((await status(page)).scene.objects[0].renderers.some(item=>item.type==='*unitcell')).toBe(false);
    await click(page,'Symmetry change');
    await click(page,'Symmetry create Cancel');
    expect(await crystal(page,id)).toBe(null);
    await click(page,'Symmetry change');
    await click(page,'Symmetry create OK');
    await enter(page,'Symmetry edit a','42');
    await enter(page,'Symmetry edit b','25');
    await click(page,'Symmetry lattice');
    await click(page,'Symmetry lattice Cubic');
    await click(page,'Symmetry restrict');
    await click(page,'Symmetry restrict');
    await click(page,'Symmetry restrict');
    await click(page,'Symmetry edit OK');
    expect(await crystal(page,id)).toEqual([42,42,42,90,90,90,195]);
    await call(page,'undo');expect(await crystal(page,id)).toBe(null);
    await call(page,'redo');expect(await crystal(page,id)).toEqual([42,42,42,90,90,90,195]);
    const rejected=await call(page,'set_symmetry',{objId:id,a:42,b:42,c:42,alpha:0,beta:90,gamma:90,spaceGroup:195});
    expect(rejected.message).toContain('invalid');
    expect(await crystal(page,id)).toEqual([42,42,42,90,90,90,195]);
    await call(page,'set_view',{center:[21,21,21],zoom:90,distance:200,slab:100});
    const view=(await status(page)).view;
    await call(page,'clear_undo');
    await click(page,'Symmetry unit cell');await click(page,'Symmetry unit cell');
    expect((await status(page)).scene.objects[0].renderers.filter(item=>item.type==='*unitcell')).toHaveLength(1);
    await call(page,'undo');
    expect((await status(page)).scene.objects[0].renderers.filter(item=>item.type==='*unitcell')).toHaveLength(0);
    await click(page,'Symmetry change');await click(page,'Symmetry edit OK');
    await call(page,'redo');
    expect((await status(page)).scene.objects[0].renderers.filter(item=>item.type==='*unitcell')).toHaveLength(1);
    await click(page,'Symmetry mates');await click(page,'Symmetry extent 20 Å');
    let renderer=(await status(page)).scene.objects[0].renderers.find(item=>item.type==='*symm');
    const prop=async key=>JSON.parse((await call(page,'get_prop',{path:`#${renderer.id}.${key}`})).message).value;
    expect(await prop('extent')).toBe('20');expect(await prop('autoupdate')).toBe('true');
    await click(page,'Symmetry mates');await click(page,'Symmetry extent Unit cell');
    expect(await prop('unitcell')).toBe('true');expect(await prop('autoupdate')).toBe('false');
    expect((await status(page)).scene.objects[0].renderers.filter(item=>item.type==='*symm')).toHaveLength(1);
    expect((await status(page)).view).toEqual(view);
    await page.screenshot({path:testInfo.outputPath('symmetry-pane.png')});
    await click(page,'Symmetry change');await enter(page,'Symmetry edit a','77');
    await click(page,'Symmetry edit Cancel');expect((await crystal(page,id))[0]).toBe(42);
    await click(page,'Symmetry change');
    await call(page,'create_scene',{name:'other'});await settle(page);
    expect((await status(page)).ui.rects['Symmetry edit OK']).toBeUndefined();
  });

  test(`Symmetry pane ${backend} exposes map cells without molecular actions`,async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async()=>Boolean((await status(page))?.scene.sceneId),{timeout:60000}).toBe(true);
    await load(page,'probe.pdb',Buffer.from(pdb));
    await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
    await load(page,'crystal.map',mapBytes());
    await expect.poll(async()=>(await status(page)).scene.maps.length).toBe(1);
    const map=(await status(page)).scene.maps[0];
    const name=(await status(page)).scene.objects.find(item=>item.id===map.id).name;
    await click(page,'Crystal');await click(page,'Symmetry object');await click(page,`Symmetry target ${name}`);
    await click(page,'Symmetry change');await click(page,'Symmetry mates');
    expect((await status(page)).ui.rects['Symmetry edit OK']).toBeUndefined();
    expect((await status(page)).ui.rects['Symmetry create OK']).toBeUndefined();
    const view=(await status(page)).view;
    await call(page,'clear_undo');
    await click(page,'Symmetry unit cell');await click(page,'Symmetry unit cell');
    const renderers=async()=>(await status(page)).scene.objects.find(item=>item.id===map.id).renderers.filter(item=>item.type==='*unitcell');
    expect(await renderers()).toHaveLength(1);
    await call(page,'undo');expect(await renderers()).toHaveLength(0);
    await call(page,'redo');expect(await renderers()).toHaveLength(1);
    expect((await status(page)).view).toEqual(view);
    await page.screenshot({path:testInfo.outputPath('symmetry-map.png')});
  });
}

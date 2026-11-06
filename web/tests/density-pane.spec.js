import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const dx = () => {
  const values=[];
  for(let z=0;z<9;z++)for(let y=0;y<9;y++)for(let x=0;x<9;x++)values.push(Math.exp(-((x-4)**2+(y-4)**2+(z-4)**2)/6));
  return Buffer.from(`object 1 class gridpositions counts 9 9 9\norigin -4 -4 -4\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\nobject 2 class gridconnections counts 9 9 9\nobject 3 class array type double rank 0 items ${values.length} data follows\n${values.join('\n')}\n`);
};
const status=page=>page.evaluate(()=>window.cuemol?.status());
async function click(page,name) {
  await expect.poll(async()=>Boolean((await status(page)).ui.rects[name])).toBe(true);
  const [x,y,w,h]=(await status(page)).ui.rects[name];
  await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function property(page,id,key) {
  const result=await call(page,'get_prop',{path:`#${id}.${key}`});
  return JSON.parse(result.message).value;
}
for(const backend of ['webgl','webgpu']) {
  test(`Density pane ${backend} edits the selected map and previews opacity`, async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async()=>Boolean((await status(page))?.scene.sceneId),{timeout:60000}).toBe(true);
    await load(page,'density.dx',dx());
    await expect.poll(async()=>(await status(page)).scene.maps.length).toBe(1);
    const state=await status(page);
    const map=state.scene.maps[0];
    const target=state.scene.objects.find(object=>object.id===map.id);
    const id=target.renderers.find(renderer=>['contour','isosurf','gpu_mapmesh','gpu_mapvol'].includes(renderer.type)).id;
    await call(page,'set_view',{zoom:16,slab:30});
    await click(page,'Crystal');
    await expect.poll(async()=>Boolean((await status(page)).ui.rects['Density level'])).toBe(true);
    await click(page,'Density level');
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.insertText('2');
    await page.keyboard.press('Enter');
    await expect.poll(async()=>Number(await property(page,id,'siglevel'))).toBe(2);
    await call(page,'undo');
    expect(Number(await property(page,id,'siglevel'))).not.toBe(2);
    await click(page,'Density options');
    await click(page,'Use absolute contour level');
    expect(await property(page,id,'use_abslevel')).toBe('true');
    await click(page,'Density options');
    await click(page,'Use sigma contour level');
    await call(page,'clear_undo');
    await click(page,'Density redraw');
    const [x,y,w,h]=(await status(page)).ui.rects['Density opacity'];
    await page.mouse.move(x+w/2,y+h/2);
    await page.mouse.down();
    await page.mouse.move(x+w/2-8,y+h/2,{steps:4});
    await expect.poll(async()=>Number(await property(page,id,'alpha'))).toBeLessThan(1);
    await page.keyboard.press('Escape');
    await page.mouse.up();
    await expect.poll(async()=>Number(await property(page,id,'alpha'))).toBe(1);
    await click(page,'Density redraw');
    await page.mouse.move(x+w/2,y+h/2);
    await page.mouse.down();
    await page.mouse.move(x+w/2-8,y+h/2,{steps:4});
    await expect.poll(async()=>Number(await property(page,id,'alpha'))).toBeLessThan(1);
    const committed=Number(await property(page,id,'alpha'));
    await page.mouse.up();
    await call(page,'undo');
    expect(Number(await property(page,id,'alpha'))).toBe(1);
    await call(page,'redo');
    expect(Number(await property(page,id,'alpha'))).toBe(committed);
    const beforeCell=(await status(page)).view;
    await page.screenshot({path:testInfo.outputPath('density-pane.png')});
    await click(page,'Density cell');
    await expect.poll(async()=>(await status(page)).scene.objects.find(object=>object.id===map.id).renderers.filter(renderer=>renderer.type==='*unitcell').length).toBe(1);
    expect((await status(page)).view).toEqual(beforeCell);
    await call(page,'undo');
    expect((await status(page)).scene.objects.find(object=>object.id===map.id).renderers.filter(renderer=>renderer.type==='*unitcell')).toHaveLength(0);
  });
}

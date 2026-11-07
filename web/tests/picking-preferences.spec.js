import {test,expect} from '@playwright/test';
import {load,call} from './support.js';
const pdb='HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n';
const status=page=>page.evaluate(()=>window.cuemol?.status());
async function click(page,key){
  await expect.poll(async()=>(await status(page))?.ui.rects[key]?.length).toBe(4);
  const [x,y,w,h]=(await status(page)).ui.rects[key];
  await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function settings(page){
  await click(page,'Edit menu');await click(page,'Settings');await click(page,'Settings Mouse & Navigation');
  const [x,y,w,h]=(await status(page)).ui.rects['Settings window'];
  await page.mouse.move(x+w-30,y+h/2);await page.mouse.wheel(0,-1600);
  await expect.poll(async()=>(await status(page)).ui.rects['GPU picking'][1]).toBeGreaterThan(y+25);
}
async function closeSettings(page){
  const [x,y,w]=(await status(page)).ui.rects['Settings window'];
  await page.mouse.click(x+w-14,y+13);
  await expect.poll(async()=>(await status(page)).ui.rects['Settings window']).toBeUndefined();
}
async function sphere(page){
  await load(page,'sphere.pdb',Buffer.from(pdb));
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  const object=(await status(page)).scene.objects[0];
  for(const renderer of object.renderers)await call(page,'delete_node',{nodeId:renderer.id,nodeType:'renderer'});
  await call(page,'create_renderer',{objId:object.id,rendererType:'cpk'});
  await call(page,'set_projection',{perspective:false});
  await call(page,'set_view',{center:[0,0,0],zoom:10,distance:40,slab:20});
}
for(const backend of ['webgl','webgpu'])test(`Picking preferences ${backend} control geometry and hover without scene undo`,async({page},testInfo)=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  await sphere(page);
  const point=async(offset=0)=>{const [x,y,w,h]=(await status(page)).ui.rects.Viewport;return [x+w/2+offset,y+h/2];};
  await page.mouse.move(...await point(35));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
  await call(page,'set_node_prop',{nodeType:'scene',prop:'name',value:'renamed'});
  await settings(page);await click(page,'GPU picking');await click(page,'Hover highlight');
  await page.screenshot({path:testInfo.outputPath('picking-settings.png')});
  const window=(await status(page)).ui.rects['Settings window'];
  expect(window[1]+window[3]).toBeLessThanOrEqual(page.viewportSize().height);
  await click(page,'Settings Configuration & Startup');
  await page.mouse.move(window[0]+window[2]-30,window[1]+window[3]-40);
  await page.mouse.wheel(0,1200);
  await expect.poll(async()=>{const [x,y,w,h]=(await status(page)).ui.rects['Settings Save'];return y+h;}).toBeLessThan(window[1]+window[3]);
  await click(page,'Settings Save');
  await expect.poll(async()=>(await status(page)).message).toBe('Settings saved.');
  await closeSettings(page);
  await page.mouse.move(...await point(35));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
  await page.mouse.move(...await point());
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
  await call(page,'undo');
  let reply=await call(page,'execute_script',{source:"print(getService('ViewInputConfig').gpu_pick)"});
  expect(JSON.parse(reply.message).log).toBe('false\n');
  await call(page,'redo');
  reply=await call(page,'execute_script',{source:"print(getService('ViewInputConfig').gpu_pick)"});
  expect(JSON.parse(reply.message).log).toBe('false\n');
  await settings(page);await click(page,'Hover info');await closeSettings(page);
  await page.mouse.move(...await point());
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
  await page.mouse.click(...await point());
  await expect.poll(async()=>(await status(page)).ui.navigation.labels).toBe(1);
  const saved=await page.evaluate(()=>localStorage.getItem('cuemol.config'));
  for(const key of ['gpu-pick','hover-info','hover-highlight'])expect(saved).toContain(`view_input:${key}\tfalse`);
  await page.reload();
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  reply=await call(page,'execute_script',{source:"print(getService('ViewInputConfig').gpu_pick)"});
  expect(JSON.parse(reply.message).log).toBe('false\n');
  await sphere(page);
  await page.mouse.move(...await point());
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
  await settings(page);await click(page,'GPU picking');await click(page,'Hover info');await closeSettings(page);
  await page.mouse.move(...await point(35));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
  expect(errors).toEqual([]);
});

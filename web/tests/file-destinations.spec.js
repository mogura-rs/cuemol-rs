import {test,expect} from '@playwright/test';
import {load,call} from './support.js';
const pdb='HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n';
const status=page=>page.evaluate(()=>window.cuemol?.status());
async function click(page,key,left=false){
  await expect.poll(async()=>(await status(page))?.ui.rects[key]?.length).toBe(4);
  const [x,y,w,h]=(await status(page)).ui.rects[key];
  await page.mouse.click(left ? x+15 : x+w/2,y+h/2);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function drop(page,files){
  await page.evaluate(files=>{
    const data=new DataTransfer();
    for(const [name,text] of files)data.items.add(new File([text],name));
    document.dispatchEvent(new DragEvent('drop',{bubbles:true,dataTransfer:data}));
  },files);
}
async function loaded(page,name){
  await expect.poll(async()=>(await status(page)).message,{timeout:60000}).toBe(`Loaded ${name}`);
  await expect.poll(async()=>(await status(page)).busy).toBe(false);
}
for(const backend of ['webgl','webgpu'])test(`File destinations ${backend} preserve batches and undo`,async({page},testInfo)=>{
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  await click(page,'Tools menu');await click(page,'Settings');
  await click(page,'Drag and drop opens into',true);await click(page,'Drop target New scene');
  expect(await page.evaluate(()=>localStorage.getItem('cuemol.config'))).toContain('files:drop-target\tnew');
  await page.screenshot({path:testInfo.outputPath('file-destinations.png')});
  await page.reload();
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  const empty=(await status(page)).scene.sceneId;
  await drop(page,[['first.pdb',pdb],['second.pdb',pdb]]);
  await loaded(page,'second.pdb');
  expect((await status(page)).scene.sceneId).toBe(empty);
  expect((await status(page)).scene.objects).toHaveLength(2);
  await drop(page,[['invalid.gro','invalid'],['third.pdb',pdb],['fourth.pdb',pdb]]);
  await loaded(page,'fourth.pdb');
  const destination=(await status(page)).scene.sceneId;
  expect(destination).not.toBe(empty);
  expect((await status(page)).scene.objects).toHaveLength(2);
  await load(page,'menu.pdb',Buffer.from(pdb));await loaded(page,'menu.pdb');
  expect((await status(page)).scene.sceneId).toBe(destination);
  expect((await status(page)).scene.objects).toHaveLength(3);
  await call(page,'undo');
  await drop(page,[['broken.gro','invalid']]);
  await expect.poll(async()=>(await status(page)).busy).toBe(false);
  await expect.poll(async()=>(await status(page)).message).not.toContain('Loading');
  await call(page,'redo');
  expect((await status(page)).scene.objects).toHaveLength(3);
  await call(page,'undo');await call(page,'undo');await call(page,'undo');
  expect((await status(page)).scene.sceneId).toBe(empty);
  expect((await status(page)).scene.objects).toHaveLength(2);
});

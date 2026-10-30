import {test, expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';

const pdb = `HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C
HETATM    2  C2  LIG A   1      10.000   0.000   0.000  1.00 10.00           C
END
`;
const status = page => page.evaluate(() => window.cuemol.status());
async function command(page, name, args={}) {
  const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await expect.poll(async()=>{const value=await status(page);return value.lastCommand===id&&!value.busy;}).toBe(true);
  return status(page);
}
async function click(page, name) {
  await expect.poll(async()=>(await status(page)).ui.rects[name]?.length).toBe(4);
  const [x,y,w,h]=(await status(page)).ui.rects[name];
  await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function setup(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await page.locator('#files').setInputFiles({name:'shell.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  const [x,y,w,h]=(await status(page)).ui.rects.Viewport;
  await page.mouse.click(x+w*.9,y+h*.8);
  return (await status(page)).scene.objects[0];
}

test('browser shell shortcuts preserve undo, modifier keys, palette and IME',async({page})=>{
  const object=await setup(page);
  await command(page,'rename_node',{nodeType:'object',nodeId:object.id,name:'Changed'});
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(async()=>(await status(page)).scene.objects[0].name).toBe(object.name);
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect.poll(async()=>(await status(page)).scene.objects[0].name).toBe('Changed');
  await command(page,'set_view',{center:[25,30,0],zoom:50});
  await page.keyboard.press('ControlOrMeta+c');
  await page.keyboard.press('ControlOrMeta+f');
  expect((await status(page)).view.center).toEqual([25,30,0]);
  await page.keyboard.press('Escape');
  await click(page,`object:${object.id}`);
  const [vx,vy,vw,vh]=(await status(page)).ui.rects.Viewport;
  await page.mouse.move(vx+vw/2,vy+vh/2);
  await page.keyboard.press('c');
  await expect.poll(async()=>(await status(page)).view.center).toEqual([5,0,0]);
  await expect.poll(async()=>(await status(page)).ui.rects['Rect Select']?.length).toBe(4);
  await page.keyboard.press('ControlOrMeta+Shift+p');
  await expect.poll(async()=>(await status(page)).ui.rects['Rect Select']).toBeUndefined();
  await page.keyboard.press('ControlOrMeta+Shift+p');
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await expect.poll(async()=>(await status(page)).ui.rects['Rect Select']?.length).toBe(4);
  await page.evaluate(()=>window.dispatchEvent(new KeyboardEvent('keydown',{key:'w',ctrlKey:true,metaKey:true,isComposing:true,bubbles:true})));
  expect((await status(page)).ui.modal).toBe(false);
});

test('browser tabs cancel closing, save the scene and restore it through undo',async({page},testInfo)=>{
  await setup(page);
  const original=(await status(page)).scene.sceneId;
  await page.keyboard.press('ControlOrMeta+Alt+t');
  await expect.poll(async()=>(await status(page)).scene.sceneId).not.toBe(original);
  await page.keyboard.press('ControlOrMeta+Alt+w');
  await expect.poll(async()=>(await status(page)).scene.sceneId).toBe(original);
  await page.keyboard.press('ControlOrMeta+Alt+w');
  await expect.poll(async()=>(await status(page)).ui.modal).toBe(true);
  await click(page,'Cancel closing tab');
  expect((await status(page)).scene.objects[0].atoms).toBe(2);
  await click(page,`Close tab ${original}`);
  await expect.poll(async()=>(await status(page)).ui.modal).toBe(true);
  const download=page.waitForEvent('download');
  await click(page,'Save and close tab');
  const file=await download;
  const path=testInfo.outputPath('closed-scene.qsc');
  await file.saveAs(path);
  expect((await readFile(path)).length).toBeGreaterThan(100);
  await expect.poll(async()=>Object.keys((await status(page)).ui.rects).filter(key=>key.startsWith('Tab ')).length).toBe(0);
  const [x,y]=(await status(page)).ui.rects.Viewport;
  await page.mouse.click(x+30,y+30);
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(async()=>(await status(page)).scene?.objects?.[0]?.atoms).toBe(2);
  await page.screenshot({path:testInfo.outputPath('restored-tab.png')});
});

test('browser Edit menu opens a working molecular edit form',async({page})=>{
  const object=await setup(page);
  await click(page,'Edit menu');
  await click(page,'Delete mol atoms…');
  await click(page,'Command Run');
  await expect.poll(async()=>(await status(page)).scene.objects[0].atoms).toBe(0);
  await command(page,'undo');
  await expect.poll(async()=>(await status(page)).scene.objects[0].atoms).toBe(object.atoms);
});

test('browser toolbar history jumps over several edits',async({page},testInfo)=>{
  const object=await setup(page);
  await command(page,'clear_undo');
  await command(page,'rename_node',{nodeType:'object',nodeId:object.id,name:'First edit'});
  await command(page,'rename_node',{nodeType:'object',nodeId:object.id,name:'Second edit'});
  await click(page,'Undo history');
  await page.screenshot({path:testInfo.outputPath('undo-history.png')});
  await click(page,'Undo step 2');
  await expect.poll(async()=>(await status(page)).scene.objects[0].name).toBe(object.name);
  await click(page,'Redo history');
  await click(page,'Redo step 2');
  await expect.poll(async()=>(await status(page)).scene.objects[0].name).toBe('Second edit');
});

test('browser Crystal action creates symmetry mates through its command form',async({page})=>{
  const object=await setup(page);
  await command(page,'set_symmetry',{objId:object.id,a:20,b:20,c:20,alpha:90,beta:90,gamma:90,spaceGroup:1});
  await click(page,'Crystal');
  await click(page,'Show symmetry mates…');
  await click(page,'Command Run');
  await expect.poll(async()=>(await status(page)).scene.objects[0].renderers.some(r=>r.type==='*symm')).toBe(true);
  await command(page,'undo');
  await expect.poll(async()=>(await status(page)).scene.objects[0].renderers.some(r=>r.type==='*symm')).toBe(false);
});

import {test,expect} from '@playwright/test';
import {load,call} from './support.js';
const status=page=>page.evaluate(()=>window.cuemol?.status());
const settle=page=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
async function rect(page,key){await expect.poll(async()=>Boolean((await status(page))?.ui.rects[key])).toBe(true);await settle(page);return(await status(page)).ui.rects[key];}
async function click(page,key){const[x,y,w,h]=await rect(page,key);await page.mouse.click(x+w/2,y+h/2);await settle(page);}
async function text(page,key,value){await click(page,key);await page.keyboard.press('ControlOrMeta+a');await page.keyboard.insertText(value);await page.keyboard.press('Enter');await settle(page);}
async function drag(page,key,dx,dy=0){const[x,y,w,h]=await rect(page,key);await page.mouse.move(x+w/2,y+h/2);await page.mouse.down();await page.mouse.move(x+w/2+dx,y+h/2+dy,{steps:10});await page.mouse.up();await settle(page);}
for(const backend of ['webgl','webgpu'])test(`View pane ${backend} handles relative transforms and absolute inputs without changing scene history`,async({page},testInfo)=>{
  await page.goto(`./?renderer=${backend}`);await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  await load(page,'view.pdb',Buffer.from('HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n'));
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  await call(page,'set_view',{zoom:50,distance:200,slab:300,center:[0,0,0],rotation:[1,0,0,0]});await call(page,'save_camera',{name:'front'});await call(page,'clear_undo');
  await call(page,'delete_camera',{name:'front'});await call(page,'undo');await click(page,'View');
  await text(page,'View RotX','30');await text(page,'View RotY','45');await text(page,'View RotZ','90');
  const rotation=(await status(page)).view.rotation;
  await drag(page,'View TraX',20,12);const center=(await status(page)).view.center;
  expect(Math.abs(center[0])).toBeLessThan(1e-8);expect(center[1]).toBeLessThan(0);expect(center[2]).toBeLessThan(0);expect(center[1]/center[2]).toBeCloseTo(Math.sqrt(3),8);
  await text(page,'View TraX','12.25');await page.keyboard.insertText('-8.5');await page.keyboard.press('Enter');await settle(page);await page.keyboard.insertText('3');await page.keyboard.press('Enter');await settle(page);
  expect((await status(page)).view.center).toEqual([12.25,-8.5,3]);expect((await status(page)).view.rotation).toEqual(rotation);
  await text(page,'View Dist','10');expect((await status(page)).view.distance).toBe(10);expect((await status(page)).view.slab).toBe(300);
  await text(page,'View Slab','500');expect((await status(page)).view.slab).toBe(20);
  await text(page,'View Zoom','-1');expect((await status(page)).view.zoom).toBe(0.01);
  await call(page,'redo');expect(JSON.parse((await call(page,'list_cameras')).message).cameras).toHaveLength(0);
  await call(page,'set_view',{zoom:40,distance:200,slab:60});await click(page,'View Perspective');expect((await status(page)).view.perspective).toBe(true);
  await click(page,'View Center mark');await click(page,'View Center mark Axis');
  await click(page,'View TraX');await page.keyboard.press('ControlOrMeta+a');await page.keyboard.insertText('999');await page.keyboard.press('Escape');await settle(page);expect((await status(page)).view.center[0]).toBe(12.25);
  await page.screenshot({path:testInfo.outputPath('view-pane.png')});
});

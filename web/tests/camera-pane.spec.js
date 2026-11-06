import {readFile} from 'node:fs/promises';
import {test,expect} from '@playwright/test';
import {load,call} from './support.js';
const status=page=>page.evaluate(()=>window.cuemol?.status());
const settle=page=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
async function rect(page,key){await expect.poll(async()=>Boolean((await status(page))?.ui.rects[key])).toBe(true);await settle(page);return (await status(page)).ui.rects[key];}
async function click(page,key,options={}){const[x,y,w,h]=await rect(page,key);await page.mouse.click(x+w/2,y+h/2,options);await settle(page);}
async function text(page,key,value,submit=true){await click(page,key);await page.keyboard.press('ControlOrMeta+a');await page.keyboard.insertText(value);if(submit)await page.keyboard.press('Enter');await settle(page);}
async function names(page){return JSON.parse((await call(page,'list_cameras')).message).cameras.map(camera=>camera.name);}
async function stored(page,name){const result=await call(page,'execute_script',{source:`var c=scene.getCamera(${JSON.stringify(name)});print(JSON.stringify(c ? {zoom:c.zoom,src:c.src,vis:c.vis_size}:null));`});return JSON.parse(JSON.parse(result.message).log.trim());}
const pdb='ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n';
for(const backend of ['webgl','webgpu']){
  test(`Camera pane ${backend} edits names and order without adding navigation history`,async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
    await call(page,'set_view',{zoom:80});await call(page,'save_camera',{name:'front'});
    await call(page,'save_camera',{name:'__current'});await call(page,'save_camera',{name:'back'});
    await call(page,'clear_undo');await click(page,'View');await click(page,'camera:front');
    await call(page,'delete_camera',{name:'back'});await call(page,'undo');
    await call(page,'set_view',{zoom:20});await click(page,'Camera apply');
    expect((await status(page)).view.zoom).toBe(80);await call(page,'redo');expect(await names(page)).toEqual(['__current','front']);
    await call(page,'undo');await click(page,'camera:front');await page.keyboard.press('ArrowDown');await page.keyboard.press('F2');
    await text(page,'Camera rename','renamed');expect(await names(page)).toEqual(['__current','front','renamed']);
    await call(page,'undo');expect(await names(page)).toEqual(['__current','front','back']);await call(page,'redo');
    const[a,b,c,d]=await rect(page,'camera:renamed'),[x,y,w]=await rect(page,'camera:__current');
    await page.mouse.move(a+c/2,b+d/2);await page.mouse.down();await page.mouse.move(x+w/2,y+2,{steps:12});await page.mouse.up();await settle(page);
    expect(await names(page)).toEqual(['__current','renamed','front']);await call(page,'undo');expect(await names(page)).toEqual(['__current','front','renamed']);
    await click(page,'camera:front',{button:'right'});await click(page,'Camera menu Properties…');
    await text(page,'Camera property Zoom','91');await click(page,'Camera properties OK');
    expect((await stored(page,'front')).zoom).toBe(91);await call(page,'undo');expect((await stored(page,'front')).zoom).toBe(80);
    await click(page,'Camera new');await text(page,'Camera new name','extra',false);await click(page,'Camera create');expect(await names(page)).toContain('extra');
    await page.screenshot({path:testInfo.outputPath('camera-pane.png')});
  });
  test(`Camera pane ${backend} restores show-hide and imports camera files`,async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
    await load(page,'probe.pdb',Buffer.from(pdb));await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
    const id=(await status(page)).scene.objects[0].id;
    await call(page,'save_camera',{name:'front'});await click(page,'View');await click(page,'camera:front');
    await call(page,'set_node_prop',{nodeType:'object',nodeId:id,prop:'visible',value:'false'});await click(page,'Camera save visibility');
    expect((await stored(page,'front')).vis).toBeGreaterThan(0);
    await call(page,'set_node_prop',{nodeType:'object',nodeId:id,prop:'visible',value:'true'});await click(page,'Camera apply');expect((await status(page)).scene.objects[0].visible).toBe(true);
    await click(page,'Camera apply visibility');expect((await status(page)).scene.objects[0].visible).toBe(false);await call(page,'undo');expect((await status(page)).scene.objects[0].visible).toBe(true);
    await click(page,'Camera save');expect((await stored(page,'front')).vis).toBe(0);
    await call(page,'undo');expect((await stored(page,'front')).vis).toBeGreaterThan(0);
    const source='<camera name="file camera" zoom="77" distance="200" slab="50"><center>(1,2,3)</center></camera>';
    await load(page,'camera source.cam',Buffer.from(source));await expect.poll(async()=>(await status(page)).message).toBe('Loaded camera file camera');
    await expect.poll(async()=>await names(page)).toContain('file camera');expect((await status(page)).view.zoom).toBe(77);
    const saved=await stored(page,'file camera');expect(saved.src).toBe('camera source.cam');
    await call(page,'undo');expect(await names(page)).not.toContain('file camera');await call(page,'redo');expect(await names(page)).toContain('file camera');
    await click(page,'camera:file camera',{button:'right'});await click(page,'Camera menu Camera file');
    const downloadPromise=page.waitForEvent('download');await click(page,'Camera menu Save');
    const downloaded=await downloadPromise;expect(downloaded.suggestedFilename()).toBe('file camera.cam');
    const exported=await readFile(await downloaded.path(),'utf8');expect(exported).toContain('file camera');
    await call(page,'execute_script',{source:'var c=scene.getCamera("file camera");c.zoom=99;scene.setCamera("file camera",c);'});expect((await stored(page,'file camera')).zoom).toBe(99);await call(page,'set_view',{zoom:44});await call(page,'clear_undo');
    await click(page,'camera:file camera',{button:'right'});await click(page,'Camera menu Camera file');await click(page,'Camera menu Reload');
    await expect.poll(async()=>(await stored(page,'file camera')).zoom).toBe(77);expect((await status(page)).view.zoom).toBe(44);
    await call(page,'undo');expect((await stored(page,'file camera')).zoom).toBe(99);expect((await status(page)).view.zoom).toBe(44);
    await load(page,'exported.cam',Buffer.from(exported));await expect.poll(async()=>(await status(page)).message).toBe('Loaded camera copy1_file camera');await expect.poll(async()=>await names(page)).toContain('copy1_file camera');
    expect((await stored(page,'copy1_file camera')).zoom).toBe(77);
    await page.screenshot({path:testInfo.outputPath('camera-file.png')});
  });
}

import {readFile} from 'node:fs/promises';
import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {call, load} from './support.js';

const molecule='HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n';
const frame=page=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
async function click(page,key,options={}) {
  await expect.poll(()=>page.evaluate(key=>window.cuemol.status().ui.rects[key],key)).toBeTruthy();
  await frame(page);
  const [x,y,w,h]=await page.evaluate(key=>window.cuemol.status().ui.rects[key],key);
  await page.mouse.click(x+w/2,y+h/2,options);
  await frame(page);
}
async function field(page,key,value) {
  await click(page,`Image export ${key}`,{clickCount:2});
  await page.keyboard.press('ControlOrMeta+A');await frame(page);
  await page.keyboard.insertText(String(value));await frame(page);
  await page.keyboard.press('Tab');await frame(page);
}
async function openExport(page,kind) {
  await click(page,'Rendering menu');await click(page,'Export scene');await click(page,`Export ${kind}`);
}
async function configure(page,width=64,height=48) {
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().imageExport.phase)).toBe('options');
  await click(page,'Image export Aspect');
  await field(page,'Width',width);await field(page,'Height',height);
  await click(page,'Image export Transparent');
  expect(await page.evaluate(()=>window.cuemol.status().imageExport)).toMatchObject({width,height,dpi:150,transparent:true,aspect:false});
}
async function open(page,renderer='webgl',picker=true) {
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(picker=>{
    if(!picker) {window.showSaveFilePicker=undefined;return;}
    window.showSaveFilePicker=async options=>{
      window.__imagePicker=options;
      window.__phaseAtPicker=window.cuemol.status().imageExport.phase;
      if(window.__cancelImagePicker)throw new DOMException('Cancelled','AbortError');
      if(window.__imagePickerError)throw new DOMException('Permission denied','SecurityError');
      const root=await navigator.storage.getDirectory();
      const handle=await root.getFileHandle('capture.png',{create:true});
      window.__imageHandle=handle;
      const create=handle.createWritable.bind(handle);
      handle.createWritable=async(...args)=>{
        window.__imageWrites=(window.__imageWrites||0)+1;
        if(window.__imageWriteError)throw new DOMException('Image destination is not writable','NotAllowedError');
        return create(...args);
      };
      return handle;
    };
  },picker);
  await page.goto(`./?renderer=${renderer}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'atom.pdb',Buffer.from(molecule));
  await call(page,'set_render_settings',{settings:'backend=povray; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
  await call(page,'set_view',{zoom:6,center:[0,0,0],slab:50,distance:100});
  return errors;
}
const idle=page=>expect.poll(()=>page.evaluate(()=>window.cuemol.status().imageExport.phase),{timeout:60000}).toBe('idle');
const bytes=page=>page.evaluate(async()=>Array.from(new Uint8Array(await(await window.__imageHandle.getFile()).arrayBuffer()))).then(Buffer.from);
function checkPng(bytes) {
  const image=PNG.sync.read(bytes);
  expect([image.width,image.height,image.alpha]).toEqual([64,48,true]);
  expect(image.data[3]).toBe(0);
  expect(image.data.filter((_,i)=>i%4===3).some(a=>a>128)).toBe(true);
  const phys=bytes.indexOf(Buffer.from('pHYs'));
  expect(phys).toBeGreaterThan(0);expect(bytes.readUInt32BE(phys+4)).toBe(5906);
}

for(const renderer of ['webgl','webgpu']) {
  test(`Browser image options ${renderer} preserve captured view and explicit PNG Umbreon output`,async({page})=>{
    const errors=await open(page,renderer);
    const before=await page.evaluate(()=>window.cuemol.status());
    const expectedDownload=page.waitForEvent('download');
    await call(page,'export_scene',{path:'expected.png',format:'png',width:64,height:48,dpi:150,transparent:true});
    const expected=await readFile(await(await expectedDownload).path());
    await page.evaluate(()=>{window.__cancelImagePicker=true;});
    await openExport(page,'png');await idle(page);
    expect((await page.evaluate(()=>window.cuemol.status())).view).toEqual(before.view);
    await page.evaluate(()=>{window.__cancelImagePicker=false;});
    await openExport(page,'png');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().imageExport.phase)).toBe('options');
    expect(await page.evaluate(()=>window.__phaseAtPicker)).not.toBe('options');
    expect(await page.evaluate(()=>window.__imageWrites||0)).toBe(0);
    await click(page,'Image export Cancel');await idle(page);
    expect(await page.evaluate(()=>window.__imageWrites||0)).toBe(0);
    await openExport(page,'png');await configure(page);
    const live=await call(page,'set_view',{zoom:18});
    await click(page,'Image export OK');await idle(page);
    const png=await bytes(page);checkPng(png);expect(png).toEqual(expected);
    expect((await page.evaluate(()=>window.cuemol.status())).view).toEqual(live.view);
    expect((await page.evaluate(()=>window.cuemol.status())).scene).toEqual(before.scene);
    await call(page,'set_view',{zoom:6});
    await openExport(page,'umbreon');await configure(page);
    await click(page,'Image export OK');await idle(page);
    const umbreon=await bytes(page);checkPng(umbreon);expect(umbreon).not.toEqual(png);
    expect((await page.evaluate(()=>window.cuemol.status())).scene).toEqual(before.scene);
    expect((await page.evaluate(()=>window.cuemol.status())).rendering.historyCount).toBe(0);
    expect(errors).toEqual([]);
  });
}

test('Browser image options fallback, cancellation and invalid targets preserve the output',async({page})=>{
  const errors=await open(page,'webgl');
  const before=await page.evaluate(()=>window.cuemol.status());
  await openExport(page,'umbreon');await configure(page,2048,1536);
  await page.evaluate(async()=>{
    const writer=await window.__imageHandle.createWritable();await writer.write('existing image');await writer.close();
    window.__imageWrites=0;
  });
  await click(page,'Image export OK');
  await click(page,'Image export stop');await idle(page);
  expect(await bytes(page)).toEqual(Buffer.from('existing image'));
  expect(await page.evaluate(()=>window.__imageWrites)).toBe(0);
  expect((await page.evaluate(()=>window.cuemol.status())).message).toBe('Image export cancelled');
  expect((await page.evaluate(()=>window.cuemol.status())).scene).toEqual(before.scene);
  await openExport(page,'png');await configure(page);
  await page.evaluate(()=>{window.__imageWriteError=true;});
  await click(page,'Image export OK');await idle(page);
  expect((await page.evaluate(()=>window.cuemol.status())).message).toContain('not writable');
  expect(await bytes(page)).toEqual(Buffer.from('existing image'));
  await page.evaluate(()=>{window.__imageWriteError=false;window.__imagePickerError=true;});
  await openExport(page,'png');await idle(page);
  expect((await page.evaluate(()=>window.cuemol.status())).message).toContain('Permission denied');
  await page.evaluate(()=>{window.__imagePickerError=false;window.showSaveFilePicker=undefined;});
  await openExport(page,'png');await configure(page);
  const download=page.waitForEvent('download');
  await click(page,'Image export OK');await idle(page);
  const file=await download;expect(file.suggestedFilename()).toBe('cuemol.png');checkPng(await readFile(await file.path()));
  await openExport(page,'png');await configure(page);
  await call(page,'close_scene',{sceneId:before.scene.sceneId,discardChanges:true});
  await click(page,'Image export OK');await idle(page);
  expect((await page.evaluate(()=>window.cuemol.status())).message).toContain('image export scene or view');
  expect(errors).toEqual([]);
});

test('Browser image destination abort keeps an existing file after staged bytes have been written',async({page})=>{
  await page.goto('./');
  await expect.poll(()=>page.evaluate(()=>!!globalThis.cuemolImageExports)).toBe(true);
  const result=await page.evaluate(async()=>{
    const root=await navigator.storage.getDirectory();
    const handle=await root.getFileHandle('atomic.png',{create:true});
    let writer=await handle.createWritable();await writer.write('original');await writer.close();
    window.showSaveFilePicker=async()=>handle;
    const selected=await cuemolImageExports.pick('atomic.png');
    const controller=new AbortController();
    const create=handle.createWritable.bind(handle);
    handle.createWritable=async()=>{
      const output=await create();
      const write=output.write.bind(output);
      output.write=async bytes=>{await write(bytes);controller.abort();};
      return output;
    };
    let aborted=false;
    try {await cuemolImageExports.write(selected.id,new TextEncoder().encode('replacement'),controller.signal);}
    catch(error){aborted=error.name==='AbortError';}
    let released=false;
    try {await cuemolImageExports.write(selected.id,new Uint8Array(),new AbortController().signal);}
    catch(error){released=error.message.includes('expired');}
    return {aborted,released,content:await(await handle.getFile()).text()};
  });
  expect(result).toEqual({aborted:true,released:true,content:'original'});
});

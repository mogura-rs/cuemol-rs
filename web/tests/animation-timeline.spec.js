import {test, expect} from '@playwright/test';
import {call} from './support.js';
const status=page=>page.evaluate(()=>window.cuemol?.status());
async function rect(page,key) {
  await expect.poll(async()=>Boolean((await status(page)).ui.rects[key]),{timeout:10000}).toBe(true);
  return (await status(page)).ui.rects[key];
}
async function settle(page) {await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));}
async function click(page,key) {const [x,y,w,h]=await rect(page,key);await page.mouse.click(x+w/2,y+h/2);await settle(page);}
async function timeline(page) {return JSON.parse((await call(page,'list_anims')).message);}
async function enter(page,key,value) {await click(page,key);await page.keyboard.press('ControlOrMeta+a');await page.keyboard.insertText(value);await page.keyboard.press('Enter');await settle(page);}
for(const backend of ['webgl','webgpu']) {
  test(`Animation strips ${backend} move resize scrub and keep one undo entry`,async({page},testInfo)=>{
    const errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async()=>Boolean((await status(page))?.scene.sceneId),{timeout:60000}).toBe(true);
    for(const name of ['base','child','last'])await call(page,'add_anim',{type:'spin',name});
    await click(page,'Animation');await call(page,'clear_undo');
    let data=await timeline(page);const ids=data.elements.map(e=>e.uid);
    let [x,y,w,h]=await rect(page,`Animation strip ${ids[0]}`);let px=x+w/2,py=y+h/2;
    await page.mouse.move(px,py);await page.mouse.down();await settle(page);
    await page.mouse.move(px+40,py,{steps:5});await settle(page);
    expect((await timeline(page)).elements[0].startMs).toBe(0);
    await page.mouse.up();await settle(page);
    data=await timeline(page);expect(data.elements[0].startMs).toBeCloseTo(400,0);expect(data.elements[1].absStartMs).toBeCloseTo(1400,0);
    await call(page,'undo');expect((await timeline(page)).elements[0].startMs).toBe(0);
    await call(page,'redo');expect((await timeline(page)).elements[0].startMs).toBeCloseTo(400,0);
    [x,y,w,h]=await rect(page,`Animation strip ${ids[1]}`);px=x+w-2;py=y+h/2;
    await page.mouse.move(px,py);await page.mouse.down();await settle(page);await page.mouse.move(px+25,py,{steps:5});
    expect((await timeline(page)).elements[1].endMs).toBe(1000);
    await page.mouse.up();await settle(page);expect((await timeline(page)).elements[1].endMs).toBeCloseTo(1250,0);
    await call(page,'undo');data=await timeline(page);expect(data.elements[1].endMs).toBe(1000);
    [x,y,w,h]=await rect(page,`Animation strip ${ids[1]}`);px=x+w/2;py=y+h/2;
    await page.mouse.move(px,py);await page.mouse.down();await settle(page);await page.mouse.move(px+35,py,{steps:5});
    await page.keyboard.press('Escape');await page.mouse.up();await settle(page);
    expect((await timeline(page)).elements.map(e=>[e.startMs,e.endMs])).toEqual(data.elements.map(e=>[e.startMs,e.endMs]));
    await call(page,'redo');expect((await timeline(page)).elements[1].endMs).toBeCloseTo(1250,0);
    await enter(page,'Animation property angle','120');expect((await call(page,'list_anims')).message).not.toContain('Error');
    await call(page,'undo');await call(page,'redo');
    [x,y,w,h]=await rect(page,'Animation ruler');px=x+20;py=y+h/2;
    await page.mouse.move(px,py);await page.mouse.down();await page.mouse.move(px+50,py,{steps:4});await settle(page);
    expect((await timeline(page)).mgr.elapsedMs).toBe(0);
    await page.mouse.up();await settle(page);expect((await timeline(page)).mgr.elapsedMs).toBeCloseTo(700,0);
    await click(page,'Animation Stop');
    await click(page,'Animation zoom Fit');await click(page,'Animation zoom +');await click(page,'Animation zoom −');
    await page.screenshot({path:testInfo.outputPath('animation-strips.png')});
    expect(errors).toEqual([]);
  });
  test(`Animation element inspector ${backend} repairs names timing references and targets`,async({page})=>{
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async()=>Boolean((await status(page))?.scene.sceneId),{timeout:60000}).toBe(true);
    await call(page,'add_anim',{type:'spin',name:'parent'});await call(page,'add_anim',{type:'wait',name:'child'});
    await click(page,'Animation');await click(page,'Animation label child');
    await enter(page,'Animation property name','renamed');expect((await timeline(page)).elements[1].name).toBe('renamed');
    await enter(page,'Animation property durationMs','0:02.500');expect((await timeline(page)).elements[1].endMs).toBe(2500);
    await call(page,'undo');expect((await timeline(page)).elements[1].endMs).toBe(1000);
    await click(page,'Animation label parent');await click(page,'Animation property timeRefName');
    const disabled=await rect(page,'Animation reference renamed');
    await page.mouse.click(disabled[0]+disabled[2]/2,disabled[1]+disabled[3]/2);await page.keyboard.press('Escape');
    expect((await timeline(page)).elements[0].timeRefName).toBe('');
    await click(page,'Animation ↓');expect((await timeline(page)).elements.map(e=>e.name)).toEqual(['renamed','parent']);
    await click(page,'Animation Delete');expect((await timeline(page)).elements.map(e=>e.name)).toEqual(['renamed']);
    await call(page,'undo');expect((await timeline(page)).elements.map(e=>e.name)).toEqual(['renamed','parent']);
  });
}

for(const backend of ['webgl','webgpu']) {
  test(`Animation Inspector realtime timing ${backend} preserves redo and generic drag history`,async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(async()=>Boolean((await status(page))?.scene.sceneId),{timeout:60000}).toBe(true);
    await call(page,'add_anim',{type:'spin',name:'base'});await call(page,'add_anim',{type:'wait',name:'child'});
    await click(page,'Animation');await click(page,'Animation label base');await call(page,'clear_undo');
    const begin=async(key,delta)=>{const [x,y,w,h]=await rect(page,key);const px=x+w/2,py=y+h/2;await page.mouse.move(px,py);await page.mouse.down();await settle(page);await page.mouse.move(px+delta,py,{steps:6});await settle(page);return {px,py};};
    await begin('Animation property durationMs',20);
    let data=await timeline(page);expect(data.elements[0].endMs).toBeGreaterThan(1000);expect(data.elements[1].absStartMs).toBe(data.elements[0].endMs);
    await page.mouse.up();await settle(page);const committed=(await timeline(page)).elements[0].endMs;
    await call(page,'undo');expect((await timeline(page)).elements[0].endMs).toBe(1000);
    await begin('Animation property startMs',30);expect((await timeline(page)).elements[0].startMs).toBeGreaterThan(0);
    await page.keyboard.press('Escape');await page.mouse.up();await settle(page);expect((await timeline(page)).elements[0].startMs).toBe(0);
    await call(page,'redo');expect((await timeline(page)).elements[0].endMs).toBe(committed);
    await click(page,'Animation Generic tab');await call(page,'clear_undo');
    await begin('Animation property quadric',20);expect((await timeline(page)).elements[0].quadric).toBeGreaterThan(0);await page.mouse.up();await settle(page);
    const quadric=(await timeline(page)).elements[0].quadric;await call(page,'undo');expect((await timeline(page)).elements[0].quadric).toBeCloseTo(0,12);
    await call(page,'redo');expect((await timeline(page)).elements[0].quadric).toBe(quadric);
    await click(page,'Animation Properties tab');
    await click(page,'Animation axis preset');await click(page,'Animation axis x');
    await click(page,'Animation axis preset');await click(page,'Animation axis cart');
    await enter(page,'Animation axis component y','2');
    await page.screenshot({path:testInfo.outputPath('animation-inspector-presets.png')});
  });
}

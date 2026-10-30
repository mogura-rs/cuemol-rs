import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {load, call} from './support.js';

const molecule = 'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n';
async function present(page) {
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function rect(page, name) {
  await expect.poll(()=>page.evaluate(name=>window.cuemol.status().ui.rects[name]?.length,name)).toBe(4);
  await present(page);
  return page.evaluate(name=>window.cuemol.status().ui.rects[name],name);
}
async function click(page,name,options={}) {
  const r=await rect(page,name);await page.mouse.click(r[0]+r[2]/2,r[1]+r[3]/2,options);await present(page);
}
async function field(page,name,value) {
  await click(page,name,{clickCount:2});
  await page.keyboard.press('ControlOrMeta+a');await present(page);
  await page.keyboard.insertText(String(value));await present(page);
  await page.keyboard.press('Tab');await present(page);
}
async function open(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'preview.pdb',Buffer.from(molecule));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_render_settings',{settings:'backend=umbreon; width=64; height=48; unit=px; dpi=300; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
  await click(page,'Render');
}
async function save(page) {
  const pending=page.waitForEvent('download');await click(page,'Save render');
  const download=await pending;const chunks=[];
  for await (const chunk of await download.createReadStream()) chunks.push(chunk);
  return Buffer.concat(chunks);
}
function resolution(bytes) {
  for(let offset=8;offset+12<=bytes.length;) {
    const size=bytes.readUInt32BE(offset), type=bytes.toString('ascii',offset+4,offset+8);
    if(type==='pHYs')return [bytes.readUInt32BE(offset+8),bytes.readUInt32BE(offset+12),bytes[offset+16]];
    offset+=size+12;
  }
  return null;
}

test('Render preview uses saved dimensions and saves the displayed transparent PNG with DPI',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await open(page);
  await click(page,'Render transparent');
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message),{timeout:60000}).toContain('Rendered 64 × 48');
  const bytes=await save(page), image=PNG.sync.read(bytes);
  expect([image.width,image.height]).toEqual([64,48]);
  expect(image.data[3]).toBe(0);
  expect(image.data.filter((_,i)=>i%4===3).some(a=>a>0)).toBe(true);
  expect(resolution(bytes)).toEqual([11811,11811,1]);
  const state=await call(page,'get_screenshot',{width:64,height:48});
  expect(PNG.sync.read(Buffer.from(JSON.parse(state.message)._image,'base64')).data.equals(image.data)).toBe(true);
  await page.screenshot({path:testInfo.outputPath('render-preview.png')});
  await field(page,'Render Width',72);await field(page,'Render Height',54);
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 72 × 54');
  const resized=PNG.sync.read(await save(page));expect([resized.width,resized.height]).toEqual([72,54]);
  expect(errors).toEqual([]);
});

test('Render Stop cancels work and preserves the last completed preview',async({page})=>{
  await open(page);await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 64 × 48');
  const before=await save(page);
  await field(page,'Render Width',2400);await field(page,'Render Height',2400);
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(true);
  await click(page,'Stop render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy),{timeout:30000}).toBe(false);
  expect((await page.evaluate(()=>window.cuemol.status())).message).toMatch(/cancelled/i);
  expect((await save(page)).equals(before)).toBe(true);
});

test('Scene eye controls, collapsed children and paint entries update live state and undo',async({page},testInfo)=>{
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'paint.pdb',Buffer.from(molecule));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0], renderer=object.renderers[0];
  await click(page,`object-visibility:${object.id}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects[0].visible)).toBe(false);
  await click(page,'Undo');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects[0].visible)).toBe(true);
  await click(page,`object-expand:${object.id}`);
  await expect.poll(()=>page.evaluate(id=>window.cuemol.status().ui.rects[`renderer:${id}`],renderer.id)).toBeUndefined();
  await click(page,`object-expand:${object.id}`);
  await click(page,`renderer-visibility:${renderer.id}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects[0].renderers[0].visible)).toBe(false);
  await click(page,'Undo');
  await click(page,`object:${object.id}`);
  const swatches = [];
  for (let i=0;i<4;i++) swatches.push(await rect(page,`paint-swatch:${i}`));
  const screenshot = PNG.sync.read(await page.screenshot({scale:'css'}));
  for (const [i,expected] of [[0,[70,130,180]],[1,[240,230,140]],[2,[255,255,0]],[3,[255,250,240]]]) {
    const [x,y,w,h]=swatches[i], offset=(Math.floor(y+h/2)*screenshot.width+Math.floor(x+w/2))*4;
    expect([...screenshot.data.subarray(offset,offset+3)]).toEqual(expected);
  }
  await click(page,'paint-color:0');
  await field(page,'Paint color','#ff00ff');
  await click(page,'Apply paint');
  const painted=await call(page,'list_paint',{nodeType:'object',nodeId:object.id});
  expect(JSON.parse(painted.message).entries[0].color).toBe('#ff00ff');
  await call(page,'undo');
  const restored=await call(page,'list_paint',{nodeType:'object',nodeId:object.id});
  expect(JSON.parse(restored.message).entries[0].color).toBe('SteelBlue');
  await click(page,'Add paint');
  let entries=JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries;
  expect(entries).toHaveLength(5);expect(entries[4].color).toBe('white');
  await click(page,'Move paint up');
  entries=JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries;
  expect(entries[3].color).toBe('white');
  await click(page,'Delete paint');
  entries=JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries;
  expect(entries).toHaveLength(4);expect(entries[3].color).toBe('FloralWhite');
  await click(page,'Color mode');await click(page,'Color mode CPK');
  expect(JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries).toEqual([]);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['paint-swatch:0'])).toBeUndefined();
  await call(page,'undo');
  expect(JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries).toEqual(entries);
  await rect(page,'paint-swatch:0');
  await page.screenshot({path:testInfo.outputPath('scene-paint.png')});
});

test('Render remains usable in a small desktop window',async({page},testInfo)=>{
  await page.setViewportSize({width:800,height:600});
  await open(page);
  for (const name of ['Start Render','Save render','Render Width','Render Height','Render DPI']) {
    const [x,y,w,h]=await rect(page,name);
    expect(x).toBeGreaterThanOrEqual(0);expect(y).toBeGreaterThanOrEqual(0);
    expect(x+w).toBeLessThanOrEqual(800);expect(y+h).toBeLessThanOrEqual(600);
  }
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 64 × 48');
  const image=PNG.sync.read(await save(page));
  expect([image.width,image.height]).toEqual([64,48]);
  await page.screenshot({path:testInfo.outputPath('render-small-window.png')});
});

for (const width of [900,760]) {
test(`Bottom tabs remain reachable beside the Inspector at ${width}px`,async({page},testInfo)=>{
  await page.setViewportSize({width,height:650});
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'tabs.pdb',Buffer.from(molecule));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0];
  await click(page,`object:${object.id}`);
  const viewport=await rect(page,'Viewport');
  for (const name of ['Console','Output','Sequence','Animation','MD Trajectory']) {
    const target=await rect(page,name);
    if (target[0]+target[2]>viewport[0]+viewport[2]-24) {
      await page.mouse.move(viewport[0]+20,target[1]+target[3]/2);
      await page.mouse.wheel(500,0);
      await present(page);
    }
    const [x,y,w,h]=await rect(page,name);
    expect(x).toBeGreaterThanOrEqual(viewport[0]);
    expect(x+w).toBeLessThanOrEqual(viewport[0]+viewport[2]-24);
    expect(y+h).toBeLessThanOrEqual(650);
    await click(page,name);
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.bottom)).toBe(name);
  }
  const expanded=await rect(page,'Viewport');
  await click(page,'Collapse panel');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects.Viewport[3])).toBeGreaterThan(expanded[3]);
  await click(page,'Expand panel');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects.Viewport[3])).toBe(expanded[3]);
  await page.screenshot({path:testInfo.outputPath('compact-bottom-tabs.png')});
});
}


test('DPI edits preserve physical image dimensions', async ({page}) => {
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'physical.pdb',Buffer.from(molecule));
  await call(page,'set_render_settings',{settings:'backend=umbreon; width=1.3; height=0.7; unit=in; dpi=64.5; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
  await click(page,'Render');
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 84 × 45');
  await field(page,'Render DPI',128.5);
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 167 × 90');
  const bytes=await save(page), image=PNG.sync.read(bytes);
  expect([image.width,image.height]).toEqual([167,90]);
  expect(resolution(bytes)).toEqual([5059,5059,1]);
  const settings=JSON.parse((await call(page,'set_render_settings',{settings:'dpi=128.5'})).message).settings;
  expect([settings.width,settings.height,settings.unit,settings.dpi]).toEqual([1.3,0.7,'in',128.5]);
});

test('webgpu Stop during OIDN loading preserves the preview and suppresses late downloads', async ({page, browserName}) => {
  test.skip(browserName !== 'chromium', 'OIDN requires WebGPU.');
  await open(page);
  await click(page,'Start Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 64 × 48');
  const previous=await save(page);
  let release;
  const gate=new Promise(resolve=>{release=resolve;});
  let entered=false;
  await page.route('**/pkg/oidn/rt_hdr_calb_cnrm.tza', async route=>{
    entered=true;
    await gate;
    await route.continue();
  });
  const downloads=[];
  const record=download=>downloads.push(download.suggestedFilename());
  page.on('download',record);
  try {
    await call(page,'set_render_settings',{settings:'backend=umbreon; umbreon.useGI=true; umbreon.giSamples=2; umbreon.giBounces=1; umbreon.supersample=1; umbreon.denoise=OIDN'});
    await page.evaluate(()=>window.cuemol.command('render_image',{width:32,height:32,fileName:'cancelled.png'}));
    await expect.poll(()=>entered,{timeout:60000}).toBe(true);
    await click(page,'Stop render');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(false);
    expect((await page.evaluate(()=>window.cuemol.status())).message).toMatch(/cancelled/i);
    release();
    await expect.poll(()=>page.evaluate(()=>performance.getEntriesByType('resource').some(e=>e.name.includes('rt_hdr_calb_cnrm.tza'))),{timeout:60000}).toBe(true);
    await present(page);
    expect((await page.evaluate(()=>window.cuemol.status())).message).toMatch(/cancelled/i);
    expect(downloads).toEqual([]);
    page.off('download',record);
    expect((await save(page)).equals(previous)).toBe(true);
  } finally {
    release();
  }
});

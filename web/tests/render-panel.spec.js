import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {load, call, enablePlugins} from './support.js';

const molecule = 'HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C  \nEND\n';

test('Render requests select their mode and reapply its preset without immediately writing scene settings', async ({page}) => {
  await open(page);
  await click(page,'Render mode Movie'); await field(page,'Movie frame rate',12.5);
  await click(page,'Render');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().rendering.movieMode)).toBe(false);
  expect(await page.evaluate(()=>window.cuemol.status().rendering.settings)).toMatchObject({width:1200,height:1200,dpi:600,unit:'px'});
  await field(page,'Render Width',42); await click(page,'Render');
  expect(await page.evaluate(()=>window.cuemol.status().rendering.settings)).toMatchObject({width:1200,height:1200,dpi:600});
  const stored=await call(page,'execute_script',{source:"print(scene.getAppData('render').width);"});
  expect(JSON.parse(stored.message).log).toBe('42\n');
  await click(page,'Rendering menu'); await click(page,'Animation rendering');
  expect(await page.evaluate(()=>window.cuemol.status().rendering.movieMode)).toBe(true);
  expect(await page.evaluate(()=>window.cuemol.status().rendering.settings)).toMatchObject({width:320,height:240,unit:'px'});
  expect(await page.evaluate(()=>window.cuemol.status().rendering.movieOptions.fps)).toBe(12.5);
  await click(page,'Rendering menu'); await click(page,'Render image');
  expect(await page.evaluate(()=>window.cuemol.status().rendering.movieMode)).toBe(false);
});
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
async function open(page, renderer = 'webgl') {
  await page.goto(`./?renderer=${renderer}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'preview.pdb',Buffer.from(molecule));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await click(page,'Render');
  await call(page,'set_render_settings',{settings:'backend=umbreon; width=64; height=48; unit=px; dpi=300; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
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

for (const renderer of ['webgl', 'webgpu']) {
  test(`Rendering displays saved values without clamping or adding history (${renderer})`, async ({page}) => {
    await open(page, renderer);
    await call(page, 'set_render_settings', {settings:'unit=in; width=0.01; height=0.02; dpi=6000; umbreon.lightIntensity=11'});
    await rect(page, 'Render DPI');
    await page.waitForTimeout(650);
    expect(await page.evaluate(() => window.cuemol.status().rendering.settings)).toMatchObject({unit:'in',width:0.01,height:0.02,dpi:6000});
    await click(page, 'Render settings Render');
    await click(page, 'Render group Lights');
    await rect(page, 'Light intensity');
    await page.waitForTimeout(650);
    expect((await page.evaluate(() => window.cuemol.status().rendering.settings))['umbreon.lightIntensity']).toBe(11);
    await call(page, 'undo');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().rendering.settings.width)).toBe(64);
    await click(page, 'Render mode Movie');
    await click(page, 'Render settings Image');
    await call(page, 'set_render_settings', {settings:'unit=px; width=9000; height=10000'});
    await rect(page, 'Render Width');
    await page.waitForTimeout(650);
    expect(await page.evaluate(() => window.cuemol.status().rendering.settings)).toMatchObject({width:9000,height:10000});
    await field(page, 'Render Width', 10001);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().rendering.settings.width)).toBe(8192);
    await call(page, 'undo');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().rendering.settings.width)).toBe(9000);
  });
}

test('Still history restores its mode and physical size after movie mode', async ({page}) => {
  await open(page);
  await call(page, 'set_render_settings', {settings:'unit=in; width=2; height=1; dpi=24'});
  await click(page, 'Start Render');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Rendered 48 × 24');
  const image = await save(page);
  await click(page, 'Render mode Movie');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Movie frame rate']?.length)).toBe(4);
  await click(page, 'Use render settings');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Render DPI']?.length)).toBe(4);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Movie frame rate'])).toBeUndefined();
  const settings = await call(page, 'execute_script', {source:"var s=scene.getAppData('render'); print(s.unit+':'+s.width+':'+s.height+':'+s.dpi);"});
  expect(JSON.parse(settings.message).log).toBe('in:2:1:24\n');
  await click(page, 'Start Render');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Rendered 48 × 24');
  expect((await save(page)).equals(image)).toBe(true);
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
  for (const [i,expected] of [[0,[230,159,0]],[1,[230,159,0]],[2,[230,217,0]],[3,[255,224,153]]]) {
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
  expect(JSON.parse(restored.message).entries[0].color).toBe('okabeito_1');
  await click(page,'Add paint');
  let entries=JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries;
  expect(entries).toHaveLength(5);expect(entries[4].color).toBe('white');
  await click(page,'Move paint up');
  entries=JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries;
  expect(entries[3].color).toBe('white');
  await click(page,'Delete paint');
  entries=JSON.parse((await call(page,'list_paint',{nodeType:'object',nodeId:object.id})).message).entries;
  expect(entries).toHaveLength(4);expect(entries[3].color).toBe('okabeito_1{mod_s:-0.6;mod_b:0.1}');
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
  for (const name of ['Start Render','Render Width','Render Height','Render DPI']) {
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
  await enablePlugins(page,['mdtools']);
  await page.setViewportSize({width,height:650});
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'tabs.pdb',Buffer.from(molecule));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0];
  await click(page,`object:${object.id}`);
  const viewport=await rect(page,'Viewport');
  expect(viewport[2]).toBeGreaterThan(100);
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
  await click(page,'Render');
  await call(page,'set_render_settings',{settings:'backend=umbreon; width=1.3; height=0.7; unit=in; dpi=64.5; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
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


for (const backend of ['webgl', 'webgpu']) {
  test(`Render history exports the selected image and restores settings with Undo (${backend})`, async ({page}, testInfo) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      Object.defineProperty(navigator.clipboard, 'write', { configurable: true, value: async items => {
        if (window.__copyError) throw new Error('Clipboard rejected');
        const blob = await items[0].getType('image/png');
        window.__copiedImage = Array.from(new Uint8Array(await blob.arrayBuffer()));
      }});
    });
    await open(page, backend);
    await click(page, 'Render transparent');
    await click(page, 'Start Render');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Rendered 64 × 48');
    const first = await save(page);
    await field(page, 'Render Width', 96);
    await field(page, 'Render Height', 72);
    await field(page, 'Render DPI', 72);
    await click(page, 'Start Render');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Rendered 96 × 72');
    const second = await save(page);
    await call(page, 'clear_undo');
    const camera = await page.evaluate(() => window.cuemol.status().view);
    await click(page, 'Previous render');
    expect((await save(page)).equals(first)).toBe(true);
    await click(page, 'Copy render');
    await expect.poll(() => page.evaluate(() => window.__copiedImage?.length)).toBe(first.length);
    expect(Buffer.from(await page.evaluate(() => window.__copiedImage)).equals(first)).toBe(true);
    await page.evaluate(() => { window.__copyError = true; });
    await click(page, 'Copy render');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Clipboard rejected');
    const viewport = page.viewportSize();
    for (const name of ['Start Render', 'Save render', 'Render preview', 'Use render settings']) {
      const [x, y, width, height] = await rect(page, name);
      expect(x).toBeGreaterThanOrEqual(0); expect(y).toBeGreaterThanOrEqual(0);
      expect(x + width).toBeLessThanOrEqual(viewport.width);
      expect(y + height).toBeLessThanOrEqual(viewport.height);
    }
    await click(page, 'Next render');
    expect((await save(page)).equals(second)).toBe(true);
    await click(page, 'Previous render');
    const readSettings = async () => {
      const state = await call(page, 'execute_script', { source: "var s=scene.getAppData('render'); print(s.width+':'+s.height+':'+s.dpi);" });
      return JSON.parse(state.message).log;
    };
    expect(await readSettings()).toBe('96:72:72\n');
    await call(page, 'undo');
    expect(await readSettings()).toBe('96:72:72\n');
    await click(page, 'Use render settings');
    expect(await readSettings()).toBe('64:48:300\n');
    await call(page, 'undo');
    expect(await readSettings()).toBe('96:72:72\n');
    await call(page, 'redo');
    expect(await readSettings()).toBe('64:48:300\n');
    expect(await page.evaluate(() => window.cuemol.status().view)).toEqual(camera);
    await click(page, 'Render actual size');
    await click(page, 'Render zoom in');
    const [x, y, w, h] = await rect(page, 'Render preview');
    await page.mouse.move(x + w / 2, y + h / 2);
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, -500);
    await page.keyboard.up('Control');
    await page.mouse.down();
    await page.mouse.move(x + w / 2 + 20, y + h / 2 + 10, {steps: 3});
    await page.mouse.up();
    await click(page, 'Fit render');
    await page.screenshot({path: testInfo.outputPath(`render-history-${backend}.png`)});
    await click(page, 'Clear render history');
    await click(page, 'Cancel clear render history');
    expect((await save(page)).equals(first)).toBe(true);
    await click(page, 'Clear render history');
    await click(page, 'Confirm clear render history');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Save render'])).toBeUndefined();
    expect(await readSettings()).toBe('64:48:300\n');
    expect(errors).toEqual([]);
  });
}


for (const backend of ['webgl', 'webgpu']) {
  test(`Render editor persists independent backend values and follows Undo (${backend})`, async ({page}, testInfo) => {
    const errors=[];page.on('pageerror',e=>errors.push(e.message));
    await open(page,backend);
    const read = async expression => {
      const result=await call(page,'execute_script',{source:`var s=scene.getAppData('render'); print(${expression});`});
      return JSON.parse(result.message).log.trim();
    };
    await call(page,'clear_undo');
    const camera=await page.evaluate(()=>window.cuemol.status().view);
    await field(page,'Render DPI',150);
    await expect.poll(()=>read('s.dpi')).toBe('150');
    await call(page,'undo');
    expect(await read('s.dpi')).toBe('300');
    await call(page,'redo');
    expect(await read('s.dpi')).toBe('150');
    await click(page,'Render size unit');await click(page,'Render unit in');
    await expect.poll(()=>read('s.unit')).toBe('in');
    await field(page,'Render Width',2);
    await field(page,'Render Height',1);
    await expect.poll(()=>read('s.width+":"+s.height+":"+s.dpi')).toBe('2:1:150');
    await click(page,'Render size unit');await click(page,'Render unit px');
    await expect.poll(()=>read('s.width+":"+s.height+":"+s.unit')).toBe('300:150:px');
    await click(page,'Render settings Render');
    await click(page,'Render lighting');await click(page,'Render lighting ao');
    await click(page,'Render AO quality');await click(page,'Render quality ao high');
    await click(page,'Render Shadows');await click(page,'Render quality shadows soft');
    await expect.poll(()=>read('s.umbreon.aoSamples+":"+s.umbreon.shadowSamples')).toBe('256:16');
    await click(page,'Render backend');await click(page,'Render backend umbreon_npr');
    await expect.poll(()=>read('s.backend+":"+s.umbreon_npr.shadowSamples+":"+s.umbreon_npr.aoEnabled')).toBe('umbreon_npr:1:false');
    await click(page,'Render lighting');
    expect(await page.evaluate(()=>window.cuemol.status().ui.rects['Render lighting gi'])).toBeUndefined();
    await click(page,'Render lighting ao');
    await click(page,'Render AO quality');await click(page,'Render quality ao low');
    await expect.poll(()=>read('s.umbreon_npr.aoSamples')).toBe('16');
    await click(page,'Render settings Detail');
    await page.screenshot({path:testInfo.outputPath(`render-detail-${backend}.png`)});
    await click(page,'Render backend');await click(page,'Render backend umbreon');
    await expect.poll(()=>read('s.backend+":"+s.umbreon.aoSamples+":"+s.umbreon.shadowSamples')).toBe('umbreon:256:16');
    await click(page,'Render lighting');await click(page,'Render lighting gi');
    await expect.poll(()=>read('s.umbreon.lightIntensity+":"+s.umbreon.flashFraction+":"+s.umbreon.ambientFraction')).toBe('1.3:0:0.6');
    await click(page,'Render GI lighting');await click(page,'Render quality giLighting 2');
    await expect.poll(()=>read('s.umbreon.lightIntensity+":"+s.umbreon.flashFraction+":"+s.umbreon.ambientFraction')).toBe('1.37:0.3:0.46');
    await call(page,'undo');
    expect(await read('s.umbreon.lightIntensity+":"+s.umbreon.flashFraction+":"+s.umbreon.ambientFraction')).toBe('1.3:0:0.6');
    await click(page,'Render lighting');await click(page,'Render lighting none');
    await expect.poll(()=>read('s.umbreon.lightIntensity+":"+s.umbreon.flashFraction')).toBe('1.55:0.6');
    await click(page,'Render lighting');await click(page,'Render lighting gi');
    await expect.poll(()=>read('s.umbreon.lightIntensity+":"+s.umbreon.flashFraction')).toBe('1.3:0');
    expect(await page.evaluate(()=>window.cuemol.status().view)).toEqual(camera);
    await page.screenshot({path:testInfo.outputPath(`render-settings-${backend}.png`)});
    expect(errors).toEqual([]);
  });
}

for (const backend of ['webgl', 'webgpu']) {
  test(`Rendering keyboard history stays with the selected scene (${backend})`, async ({page}) => {
    await open(page, backend);
    const script = async source => JSON.parse((await call(page, 'execute_script', {source})).message).log.trim();
    const first = (await page.evaluate(() => window.cuemol.status())).scene.sceneId;
    const firstView = Number(await script('print(scene.activeViewID);'));
    await call(page, 'clear_undo');
    await call(page, 'set_render_settings', {settings:'dpi=72'});
    await call(page, 'create_scene', {name:'Second'});
    const second = (await page.evaluate(() => window.cuemol.status())).scene.sceneId;
    const secondView = Number(await script('print(scene.activeViewID);'));
    await call(page, 'set_render_settings', {settings:'dpi=150'});
    const camera = await page.evaluate(() => window.cuemol.status().view);
    const read = id => script(`var r=cuemol.getScene(${id}).getAppData('render'); print(r?r.dpi:600);`);
    await click(page, 'Render target'); await click(page, `Render target ${firstView}`);
    await click(page, 'Render settings Image');
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => read(first)).toBe('300');
    expect(await read(second)).toBe('150');
    expect((await page.evaluate(() => window.cuemol.status())).scene.sceneId).toBe(second);
    expect(await page.evaluate(() => window.cuemol.status().view)).toEqual(camera);
    await page.keyboard.press('ControlOrMeta+Shift+z');
    await expect.poll(() => read(first)).toBe('72');
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => read(first)).toBe('300');
    await click(page, 'Render target'); await click(page, `Render target ${secondView}`);
    await click(page, 'Render settings Image');
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => read(second)).toBe('600');
    await call(page, 'set_render_settings', {sceneId:first, settings:'dpi=120'});
    await page.keyboard.press('ControlOrMeta+Shift+z');
    await expect.poll(() => read(second)).toBe('150');
    expect(await read(first)).toBe('120');
    expect(await page.evaluate(() => window.cuemol.status().view)).toEqual(camera);
  });

  test(`Render target owns settings and camera while the active view stays unchanged (${backend})`, async ({page}, testInfo) => {
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await open(page, backend);
    const script = async source => JSON.parse((await call(page, 'execute_script', {source})).message).log.trim();
    const first = (await page.evaluate(() => window.cuemol.status())).scene.sceneId;
    const firstView = Number(await script('print(scene.activeViewID);'));
    await call(page, 'set_node_prop', {nodeType:'scene', prop:'bgcolor', value:'#ff0000'});
    await call(page, 'create_scene', {name:'Other target'});
    const second = (await page.evaluate(() => window.cuemol.status())).scene.sceneId;
    const secondView = Number(await script('print(scene.activeViewID);'));
    await call(page,'set_render_settings',{settings:'backend=umbreon; width=80; height=60; unit=px; dpi=150; umbreon.useGI=false; umbreon.aoEnabled=false; umbreon.supersample=1; umbreon.denoise=None'});
    const viewBefore = await page.evaluate(() => window.cuemol.status().view);
    await click(page, 'Render target');
    await click(page, `Render target ${firstView}`);
    await field(page, 'Render DPI', 72);
    await expect.poll(() => script(`print(cuemol.getScene(${first}).getAppData('render').dpi);`)).toBe('72');
    expect(await script(`print(cuemol.getScene(${second}).getAppData('render').dpi);`)).toBe('150');
    await call(page,'undo');
    expect(await script(`print(cuemol.getScene(${first}).getAppData('render').dpi);`)).toBe('300');
    await call(page,'redo');
    await click(page,'Start Render');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Rendered 64 × 48');
    const firstPng = await save(page), firstImage = PNG.sync.read(firstPng);
    expect([firstImage.width,firstImage.height]).toEqual([64,48]);
    expect([...firstImage.data.subarray(0,3)]).toEqual([255,0,0]);
    const direct = JSON.parse((await call(page,'get_screenshot',{sceneId:first,viewId:firstView})).message);
    expect(PNG.sync.read(Buffer.from(direct._image,'base64')).data.equals(firstImage.data)).toBe(true);
    await click(page, 'Render target'); await click(page, `Render target ${secondView}`);
    await click(page,'Start Render');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Rendered 80 × 60');
    await click(page,'Previous render');
    await click(page,'Use render settings');
    expect(await script(`print(cuemol.getScene(${second}).getAppData('render').width);`)).toBe('64');
    expect(await script(`print(cuemol.getScene(${first}).getAppData('render').width);`)).toBe('64');
    await call(page,'undo');
    expect(await script(`print(cuemol.getScene(${second}).getAppData('render').width);`)).toBe('80');
    expect((await page.evaluate(() => window.cuemol.status())).scene.sceneId).toBe(second);
    expect(await page.evaluate(() => window.cuemol.status().view)).toEqual(viewBefore);
    await click(page, 'Render target');
    await page.screenshot({path:testInfo.outputPath(`render-target-${backend}.png`)});
    expect(errors).toEqual([]);
  });
}

for (const backend of ['webgl','webgpu']) {
  test(`POV-Ray settings persist while browser execution reports its native requirement (${backend})`, async ({page},testInfo) => {
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await open(page,backend);
    await click(page,'Start Render');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Rendered 64 × 48');
    const previous=await save(page);
    const read=async expression=>JSON.parse((await call(page,'execute_script',{source:`var r=scene.getAppData('render');print(${expression});`})).message).log.trim();
    await click(page,'Render backend');await click(page,'Render backend povray');
    await expect.poll(()=>read('r.backend')).toBe('povray');
    await click(page,'Render settings Render');
    await field(page,'Light intensity',1.1);
    await click(page,'Radiosity mode');await click(page,'Render radiosityMode Fast');
    await expect.poll(()=>read('r.povray.lightIntensity+":"+r.povray.radiosityMode')).toBe('1.1:Fast');
    await click(page,'Start Render');
    expect((await page.evaluate(()=>window.cuemol.status())).busy).toBe(false);
    expect((await save(page)).equals(previous)).toBe(true);
    const failed=await call(page,'get_screenshot');
    expect(failed.message).toContain('POV-Ray runs in the native application');
    await click(page,'Render backend');await click(page,'Render backend umbreon');
    await expect.poll(()=>read('r.backend')).toBe('umbreon');
    await click(page,'Render backend');await click(page,'Render backend povray');
    await expect.poll(()=>read('r.povray.lightIntensity+":"+r.povray.radiosityMode')).toBe('1.1:Fast');
    await page.screenshot({path:testInfo.outputPath(`render-pov-${backend}.png`)});
    const viewport=page.viewportSize();
    for(const key of ['Start Render','Render target','Radiosity mode']) {
      const [x,y,w,h]=await rect(page,key);expect(x+w).toBeLessThanOrEqual(viewport.width);expect(y+h).toBeLessThanOrEqual(viewport.height);
    }
    expect(errors).toEqual([]);
  });
}

for(const renderer of ['webgl','webgpu']) {
test(`NPR Detail ${renderer} exposes tone controls, resets the style and leaves Detail when the backend changes`, async ({page}) => {
  await open(page,renderer);
  await call(page,'set_render_settings',{settings:'backend=umbreon_npr; umbreon_npr.hatchStyle=pencil; umbreon_npr.aoEnabled=false; umbreon_npr.useGI=false'});
  await click(page,'Render settings Detail');
  await click(page,'NPR Layers');
  await field(page,'NPR shading Strength',1.5);
  await field(page,'NPR shading Curve',0.75);
  await expect.poll(async()=>{
    const result=await call(page,'execute_script',{source:"print(scene.getAppData('render').umbreon_npr.hatchToneSpec);"});
    return JSON.parse(result.message).log;
  }).toContain('strength=1.5');
  await expect.poll(async()=>{
    const result=await call(page,'execute_script',{source:"print(scene.getAppData('render').umbreon_npr.hatchToneSpec);"});
    return JSON.parse(result.message).log;
  }).toContain('curve=0.75');
  await page.screenshot({path:test.info().outputPath('npr-detail.png')});
  await click(page,'NPR Reset to style');
  await expect.poll(async()=>{
    const result=await call(page,'execute_script',{source:"print(scene.getAppData('render').umbreon_npr.hatchToneSpec);"});
    return JSON.parse(result.message).log;
  }).toBe('\n');
  await call(page,'set_render_settings',{settings:'backend=umbreon'});
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['Render lighting']?.length)).toBe(4);
  expect(await page.evaluate(()=>window.cuemol.status().ui.rects['Render settings Detail'])).toBeUndefined();
});

}

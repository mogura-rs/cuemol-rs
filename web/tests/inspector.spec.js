import {test, expect} from '@playwright/test';

const pdb = `HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C
HETATM    2  O1  LIG A   1       1.400   0.000   0.000  1.00 10.00           O
END
`;
const status = page => page.evaluate(() => window.cuemol.status());
async function command(page, name, args = {}) {
  const id = await page.evaluate(([name,args]) => window.cuemol.command(name,args), [name,args]);
  await expect.poll(async () => (await status(page)).lastCommand === id && !(await status(page)).busy).toBe(true);
  const message = (await status(page)).message;
  try { return JSON.parse(message); } catch { return message; }
}
async function click(page, name, inspector = true, double = false) {
  await expect.poll(async () => (await status(page)).ui[inspector ? 'inspector' : 'rects'][name]?.length).toBe(4);
  const [x,y,w,h] = (await status(page)).ui[inspector ? 'inspector' : 'rects'][name];
  if (double) await page.mouse.dblclick(x+w/2,y+h/2); else await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}
async function filter(page, text) {
  await click(page, 'Filter properties');
  await expect(page.locator('input[type="text"]')).toBeFocused();
  await page.keyboard.press('ControlOrMeta+a');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  if (text) await page.keyboard.insertText(text); else await page.keyboard.press('Backspace');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.keyboard.press('Tab');
}
async function setup(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await page.locator('#files').setInputFiles({name:'inspector.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  const object = (await status(page)).scene.objects[0];
  const created = await command(page,'create_renderer',{objId:object.id,rendererType:'ribbon',name:'Properties'});
  await click(page, `object:${object.id}`, false);
  await click(page, `renderer:${created.rendererId}`, false);
  await click(page,'Generic tab');
  await command(page,'clear_undo');
  return {object, id:created.rendererId};
}
async function prop(page,id,key) {
  return (await command(page,'get_prop',{path:`#${id}.${key}`})).value;
}

async function number(page,key,value) {
  await filter(page,key);
  await click(page,key,true,true);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(value);
  await page.keyboard.press('Enter');
}

for (const fontAccess of ['resolved', 'denied']) {
  test(`Font picker preserves scene fonts and handles ${fontAccess} local enumeration`, async ({page}, testInfo) => {
    await page.addInitScript(access => {
      window.fontQueries = 0;
      window.fontPreviewReads = 0;
      window.queryLocalFonts = () => {
        window.fontQueries++;
        if (access === 'denied') return Promise.reject(new DOMException('Not allowed', 'NotAllowedError'));
        return new Promise(resolve => { window.resolveFonts = resolve; });
      };
      const previews = new WeakSet();
      const prototype = Object.getPrototypeOf(new OffscreenCanvas(1, 1).getContext('2d'));
      const fill = prototype.fillText, read = prototype.getImageData;
      prototype.fillText = function(text, ...args) {
        if (text.startsWith('Test font ')) previews.add(this);
        return fill.call(this,text,...args);
      };
      prototype.getImageData = function(...args) {
        if (previews.has(this)) window.fontPreviewReads++;
        return read.apply(this,args);
      };
    }, fontAccess);
    const {object} = await setup(page);
    await command(page,'execute_script',{source:`var labels=cuemol.getObject(${object.id}).createRenderer('*namelabel');labels.name='Fonts';`});
    const id = (await status(page)).scene.objects.find(item=>item.id===object.id).renderers.find(renderer=>renderer.name==='Fonts').id;
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:id,prop:'font_name',value:'Scene-only family'});
    await click(page,`renderer:${id}`,false);
    await click(page,'Properties tab');
    await filter(page,'font_name');
    await expect.poll(()=>page.evaluate(()=>window.fontQueries)).toBe(1);
    await click(page,'font_name');
    await expect.poll(async()=>(await status(page)).ui.inspector['font_name option Scene-only family']?.length).toBe(4);
    await expect.poll(async()=>(await status(page)).ui.inspector['font_name option Arial']?.length).toBe(4);
    if (fontAccess === 'resolved') {
      await page.evaluate(()=>window.resolveFonts(Array.from({length:500},(_,index)=>({family:`Test font ${String(index).padStart(3,'0')}`}))));
      await expect.poll(async()=>(await status(page)).ui.inspector['font_name option Test font 000']?.length).toBe(4);
      expect(await prop(page,id,'font_name')).toBe('Scene-only family');
      const reads = await page.evaluate(()=>window.fontPreviewReads);
      expect(reads).toBeGreaterThan(0);
      expect(reads).toBeLessThan(20);
      const visible = Object.keys((await status(page)).ui.inspector).filter(key=>key.startsWith('font_name option '));
      expect(visible.length).toBeLessThan(20);
      await page.evaluate(()=>new Promise(resolve=>{let frames=12;const next=()=>--frames?requestAnimationFrame(next):resolve();requestAnimationFrame(next);}));
      expect(await page.evaluate(()=>window.fontPreviewReads)).toBe(reads);
      await page.screenshot({path:testInfo.outputPath('inspector-font-faces.png')});
    }
    await click(page,'font_name option monospace');
    await expect.poll(async()=>prop(page,id,'font_name')).toBe('monospace');
    await command(page,'undo');
    expect(await prop(page,id,'font_name')).toBe('Scene-only family');
    expect(await page.evaluate(()=>window.fontQueries)).toBe(1);
  });
}

test('Optional loop sizes remember values and dash toggles undo the complete pattern', async ({page},testInfo) => {
  const {object}=await setup(page);
  const overlay=await command(page,'create_renderer',{objId:object.id,rendererType:'disorder',name:'Loops'});
  const id=overlay.rendererId;
  await click(page,`renderer:${id}`,false);
  await click(page,'Properties tab');
  await filter(page,'loopsize2');
  await click(page,'Enable loopsize2');
  await expect.poll(async()=>prop(page,id,'loopsize2')).toBe('2');
  await number(page,'loopsize2','3.4');
  await expect.poll(async()=>prop(page,id,'loopsize2')).toBe('3.4');
  await click(page,'Enable loopsize2');
  await expect.poll(async()=>prop(page,id,'loopsize2')).toBe('-1');
  await click(page,'Enable loopsize2');
  await expect.poll(async()=>prop(page,id,'loopsize2')).toBe('3.4');
  await click(page,'Reset loopsize2');
  await expect.poll(async()=>prop(page,id,'loopsize2')).toBe('-1');
  await click(page,'Enable loopsize2');
  await expect.poll(async()=>prop(page,id,'loopsize2')).toBe('2');
  const interaction=await command(page,'create_renderer',{objId:object.id,rendererType:'atomintr',name:'Measurements'});
  const rid=interaction.rendererId;
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:rid,prop:'stipple0',value:'1'});
  await click(page,`renderer:${rid}`,false);
  const defaultBefore=(await command(page,'list_node_props',{nodeType:'renderer',nodeId:rid})).properties.find(row=>row.key==='stipple2').isdefault;
  expect(defaultBefore).toBe(true);
  await filter(page,'stipple2'); await click(page,'stipple2');
  await page.keyboard.press('Tab');
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  expect((await command(page,'list_node_props',{nodeType:'renderer',nodeId:rid})).properties.find(row=>row.key==='stipple2').isdefault).toBe(true);
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:rid,prop:'stipple2',value:'-2.5'});
  await click(page,'stipple2'); await page.keyboard.press('Tab');
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  expect(await prop(page,rid,'stipple2')).toBe('-2.5');
  await command(page,'set_node_props',{nodeType:'renderer',nodeId:rid,updates:Array.from({length:6},(_,i)=>({prop:`stipple${i}`,value:'-1'}))});
  await click(page,`renderer:${rid}`,false);
  await filter(page,'dash-pattern');
  await command(page,'clear_undo');
  await click(page,'dash-pattern');
  await expect.poll(async()=>prop(page,rid,'stipple0')).toBe('1');
  expect(await prop(page,rid,'stipple1')).toBe('1');
  for(let i=2;i<6;i++)expect(await prop(page,rid,`stipple${i}`)).toBe('-1');
  await command(page,'undo');
  for(let i=0;i<6;i++)expect(await prop(page,rid,`stipple${i}`)).toBe('-1');
  await command(page,'redo');
  await filter(page,'stipple2');
  await click(page,'stipple2');
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.insertText('2.75'); await page.keyboard.press('Enter');
  await expect.poll(async()=>prop(page,rid,'stipple2')).toBe('2.75');
  await click(page,'stipple2');
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.press('Backspace'); await page.keyboard.press('Enter');
  await expect.poll(async()=>prop(page,rid,'stipple2')).toBe('-1');
  await page.screenshot({path:testInfo.outputPath('inspector-dash-pattern.png')});
  await filter(page,'mode');
  await click(page,'mode'); await click(page,'mode option fancy');
  await filter(page,'captype_start');
  await click(page,'captype_start'); await click(page,'captype_start option arrow');
  expect(await prop(page,rid,'captype_start')).toBe('arrow');
  await number(page,'arrowwidth','1.5');
  expect(await prop(page,rid,'arrowwidth')).toBe('1.5');
});

test('Map pages follow map kind, retain an empty display limit and batch center updates', async ({page},testInfo) => {
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  const bytes=Buffer.alloc(1024+64*4);
  for(let i=0;i<3;i++){
    bytes.writeInt32LE(4,i*4); bytes.writeInt32LE(4,(i+7)*4);
    bytes.writeFloatLE(4,(i+10)*4); bytes.writeFloatLE(90,(i+13)*4); bytes.writeInt32LE(i+1,(i+16)*4);
  }
  bytes.writeInt32LE(2,12); bytes.write('MAP ',208);
  bytes.writeFloatLE(7,80); bytes.writeFloatLE(3.5,84); bytes.writeFloatLE(2,216);
  for(let i=0;i<64;i++)bytes.writeFloatLE(i%8,1024+i*4);
  await page.locator('#files').setInputFiles({name:'kind.mrc',mimeType:'application/octet-stream',buffer:bytes});
  await expect.poll(async()=>(await status(page)).scene.maps.length).toBe(1);
  const map=(await status(page)).scene.maps[0];
  const created=await command(page,'create_renderer',{objId:map.id,rendererType:'contour',name:'Map controls'});
  const rid=created.rendererId;
  await command(page,'set_node_prop',{nodeType:'object',nodeId:map.id,prop:'map_type',value:'xtal'});
  await click(page,`object:${map.id}`,false);
  await filter(page,'map_type');
  await command(page,'clear_undo');
  await click(page,'map_type'); await click(page,'map_type option em');
  await expect.poll(async()=>prop(page,map.id,'map_type_resolved')).toBe('em');
  await command(page,'undo');
  expect(await prop(page,map.id,'map_type_resolved')).toBe('xtal');
  await command(page,'redo');
  await click(page,`renderer:${rid}`,false);
  await number(page,'width','0.85');
  await expect.poll(async()=>prop(page,rid,'width')).toBe('0.85');
  await filter(page,'lod_budget');
  await expect.poll(async()=>(await status(page)).ui.inspector.lod_budget?.length).toBe(4);
  await filter(page,'bufsize');
  await expect.poll(async()=>(await status(page)).ui.inspector.bufsize).toBeUndefined();
  await filter(page,'center-update');
  await click(page,'center-update'); await click(page,'center-update option None');
  await command(page,'clear_undo');
  await click(page,'center-update'); await click(page,'center-update option Automatic (drag)');
  expect(await prop(page,rid,'autoupdate')).toBe('true');
  expect(await prop(page,rid,'dragupdate')).toBe('true');
  await click(page,'Reset center-update');
  expect(await prop(page,rid,'autoupdate')).toBe('true');
  expect(await prop(page,rid,'dragupdate')).toBe('false');
  await command(page,'undo');
  expect(await prop(page,rid,'dragupdate')).toBe('true');
  await command(page,'undo');
  expect(await prop(page,rid,'autoupdate')).toBe('false');
  expect(await prop(page,rid,'dragupdate')).toBe('false');
  await filter(page,'limit-display'); await click(page,'limit-display');
  await filter(page,'bndry_molname'); await click(page,'bndry_molname');
  await expect.poll(async()=>(await status(page)).ui.inspector['bndry_molname option ']?.length).toBe(4);
  await page.keyboard.press('Escape');
  await filter(page,'limit-display'); await click(page,'limit-display');
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:rid,prop:'width',value:'0.9'});
  await filter(page,'bndry_molname'); await click(page,'bndry_molname');
  expect((await status(page)).ui.inspector['bndry_molname option ']).toBeUndefined();
  await page.screenshot({path:testInfo.outputPath('inspector-map-controls.png')});
});

test('Renderer pages gate ring controls, clamp numeric edits and retain custom detail', async ({page},testInfo) => {
  const {object}=await setup(page);
  const created=await command(page,'create_renderer',{objId:object.id,rendererType:'ballstick',name:'Atoms'});
  const id=created.rendererId;
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:id,prop:'ring',value:'false'});
  await click(page,`renderer:${id}`,false);
  await click(page,'Properties tab');
  const before=await prop(page,id,'thickness');
  await number(page,'thickness','0.9');
  expect(await prop(page,id,'thickness')).toBe(before);
  await filter(page,'ring');
  await click(page,'ring');
  await expect.poll(async()=>prop(page,id,'ring')).toBe('true');
  await command(page,'clear_undo');
  await number(page,'thickness','5');
  await expect.poll(async()=>prop(page,id,'thickness')).toBe('3');
  await command(page,'undo');
  expect(await prop(page,id,'thickness')).toBe(before);
  await number(page,'thickness','0.333');
  await expect.poll(async()=>prop(page,id,'thickness')).toBe('0.333');
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:id,prop:'detail',value:'24'});
  await filter(page,'detail');
  await click(page,'detail');
  await expect.poll(async()=>(await status(page)).ui.inspector['detail option 24']?.length).toBe(4);
  await click(page,'detail option 16');
  await expect.poll(async()=>prop(page,id,'detail')).toBe('16');
  await page.screenshot({path:testInfo.outputPath('inspector-renderer-detail.png')});
});

test('Tube dimensions and cartoon junctions form atomic edits through the structured page', async ({page,context},testInfo) => {
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  const {object}=await setup(page);
  const created=await command(page,'create_renderer',{objId:object.id,rendererType:'tube',name:'Axes'});
  const id=created.rendererId;
  await command(page,'set_node_props',{nodeType:'renderer',nodeId:id,updates:[{prop:'section.width',value:'1'},{prop:'section.tuber',value:'0.5'}]});
  await click(page,`renderer:${id}`,false);
  await click(page,'Properties tab');
  await command(page,'clear_undo');
  await number(page,'section.width','2');
  await expect.poll(async()=>prop(page,id,'section.width')).toBe('2');
  expect(await prop(page,id,'section.tuber')).toBe('0.25');
  await filter(page,'section.tuber');
  await click(page,'section.tuber',true,true);
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(async()=>Number(await navigator.clipboard.readText()))).toBe(0.5);
  await command(page,'undo');
  expect(await prop(page,id,'section.width')).toBe('1');
  expect(await prop(page,id,'section.tuber')).toBe('0.5');
  await command(page,'redo');
  expect(await prop(page,id,'section.width')).toBe('2');
  expect(await prop(page,id,'section.tuber')).toBe('0.25');
  const cartoon=await command(page,'create_renderer',{objId:object.id,rendererType:'cartoon',name:'Helix'});
  await click(page,`renderer:${cartoon.rendererId}`,false);
  await filter(page,'helix_ribbon');
  await click(page,'helix_ribbon');
  await click(page,'helix_ribbon option true');
  await filter(page,'ribhelix_head.type');
  await click(page,'ribhelix_head.type');
  await click(page,'ribhelix_head.type option arrow');
  expect(await prop(page,cartoon.rendererId,'ribhelix_tail.type')).toBe('arrow');
  await command(page,'clear_undo');
  const oldHead=await prop(page,cartoon.rendererId,'ribhelix_head.arrow');
  const oldTail=await prop(page,cartoon.rendererId,'ribhelix_tail.arrow');
  await number(page,'ribhelix_head.arrow','80');
  await expect.poll(async()=>prop(page,cartoon.rendererId,'ribhelix_head.arrow')).toBe('2.6');
  expect(await prop(page,cartoon.rendererId,'ribhelix_tail.arrow')).toBe('2.6');
  await command(page,'undo');
  expect(await prop(page,cartoon.rendererId,'ribhelix_head.arrow')).toBe(oldHead);
  expect(await prop(page,cartoon.rendererId,'ribhelix_tail.arrow')).toBe(oldTail);
  await page.screenshot({path:testInfo.outputPath('inspector-derived-junctions.png')});
});

test('Inspector edits typed values and resets them in one undo step', async ({page},testInfo) => {
  const {id} = await setup(page);
  await filter(page,'alpha');
  await click(page,'alpha',true,true);
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.type('0.35'); await page.keyboard.press('Enter');
  await expect.poll(async () => prop(page,id,'alpha')).toBe('0.35');
  await filter(page,'egtype');
  await click(page,'egtype');
  await click(page,'egtype option silhouette');
  await expect.poll(async () => prop(page,id,'egtype')).toBe('silhouette');
  await click(page,'Reset all to default');
  await expect.poll(async () => prop(page,id,'alpha')).toBe('1');
  expect(await prop(page,id,'egtype')).toBe('none');
  expect(await prop(page,id,'name')).toBe('Properties');
  await command(page,'undo');
  expect(await prop(page,id,'alpha')).toBe('0.35');
  expect(await prop(page,id,'egtype')).toBe('silhouette');
  await page.screenshot({path:testInfo.outputPath('inspector-typed-reset.png')});
  await command(page,'set_node_prop',{nodeType:'view',prop:'zoom',value:'25'});
  await command(page,'reset_node_props',{nodeType:'scene'});
  await command(page,'redo');
  expect(await prop(page,id,'alpha')).toBe('1');
});

test('Inspector exposes nested enum defaults and keeps identity read-only', async ({page},testInfo) => {
  const {id} = await setup(page);
  await filter(page,'helixhead.type');
  await click(page,'helixhead.type');
  await click(page,'helixhead.type option flat');
  await expect.poll(async () => prop(page,id,'helixhead.type')).toBe('flat');
  await click(page,'Reset helixhead.type');
  const rows = (await command(page,'list_node_props',{nodeType:'renderer',nodeId:id})).properties;
  expect(rows.find(row=>row.key==='helixhead.type').isdefault).toBe(true);
  await filter(page,'uid');
  await click(page,'uid',true,true);
  await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.type('999999'); await page.keyboard.press('Enter');
  expect(await prop(page,id,'uid')).toBe(String(id));
  await filter(page,'helix');
  await page.screenshot({path:testInfo.outputPath('inspector-nested.png')});
});

test('Scene presets use one undo and View fields follow viewport navigation', async ({page,context},testInfo) => {
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await setup(page);
  await click(page,'Scene menu',false);
  await click(page,'Scene Properties',false);
  await click(page,'name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Inspector scene');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await command(page,'get_prop',{path:'name'})).value).toBe('Inspector scene');
  await click(page,'aoEnabled');
  await command(page,'clear_undo');
  await click(page,'ao-preset');
  await click(page,'ao-preset option High');
  await expect.poll(async () => (await command(page,'get_prop',{path:'aoRadius'})).value).toBe('12');
  expect((await command(page,'get_prop',{path:'aoSteps'})).value).toBe('5');
  await command(page,'undo');
  expect((await command(page,'get_prop',{path:'aoRadius'})).value).toBe('4');
  expect((await command(page,'get_prop',{path:'aoSteps'})).value).toBe('3');
  await command(page,'redo');
  await page.screenshot({path:testInfo.outputPath('inspector-scene-presets.png')});
  await click(page,'Generic tab');
  await filter(page,'name');
  await click(page,'name');
  await page.keyboard.insertText('forbidden');
  await page.keyboard.press('Enter');
  expect((await command(page,'get_prop',{path:'name'})).value).toBe('Inspector scene');
  await click(page,'View menu',false);
  await click(page,'View Properties',false);
  await filter(page,'zoom');
  const oldZoom=(await status(page)).view.zoom;
  const viewport=(await status(page)).ui.rects.Viewport;
  expect(viewport).toHaveLength(4);
  await page.mouse.move(viewport[0]+viewport[2]/2,viewport[1]+viewport[3]/2);
  await page.mouse.wheel(0,-180);
  await expect.poll(async () => (await status(page)).view.zoom).not.toBe(oldZoom);
  await click(page,'zoom',true,true);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toMatch(/^\d/);
  const value=await page.evaluate(()=>navigator.clipboard.readText());
  expect(Number(value)).toBeCloseTo((await status(page)).view.zoom,1);
  await page.keyboard.type('37.25');
  const beforeDraftNavigation=(await status(page)).view.zoom;
  await page.mouse.move(viewport[0]+viewport[2]/2,viewport[1]+viewport[3]/2);
  await page.mouse.wheel(0,180);
  await expect.poll(async () => (await status(page)).view.zoom).not.toBe(beforeDraftNavigation);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('37.25');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBeCloseTo(37.25,2);
  await page.keyboard.press('Escape');
  await filter(page,'name');
  await click(page,'name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Orbit view');
  const beforeNameNavigation=(await status(page)).view.zoom;
  await page.mouse.move(viewport[0]+viewport[2]/2,viewport[1]+viewport[3]/2);
  await page.mouse.wheel(0,-180);
  await expect.poll(async () => (await status(page)).view.zoom).not.toBe(beforeNameNavigation);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('Orbit view');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await command(page,'get_prop',{path:'view.name'})).value).toBe('Orbit view');
  await click(page,'name');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('Obsolete draft');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('Obsolete draft');
  const renamed=await command(page,'set_node_prop',{nodeType:'view',prop:'name',value:'External rename'});
  expect(renamed.value).toBe('External rename');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('ControlOrMeta+c');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('External rename');
  await page.screenshot({path:testInfo.outputPath('inspector-scene-view.png')});
});

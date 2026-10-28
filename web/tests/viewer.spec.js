import {test,expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {existsSync, readFileSync} from 'node:fs';
const structure=`HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00 10.00           C  
HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00 10.00           O  
HETATM    3  N1  LIG A   1       1.300   1.000   0.000  1.00 10.00           N  
CONECT    1    2
CONECT    2    3
END
`;
async function ready(page){
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>!!window.cuemol),{timeout:60000}).toBe(true);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.sceneId)).toBeGreaterThan(0);
}
async function load(page,name,buffer){
  await page.locator('#files').setInputFiles({name,mimeType:'application/octet-stream',buffer});
}
async function call(page,name,args={}){
  const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await expect.poll(()=>page.evaluate(id=>{
    const state=window.cuemol.status();return state.lastCommand===id&&!state.busy;
  },id),{timeout:60000}).toBe(true);
  return page.evaluate(()=>window.cuemol.status());
}
test('default WebGPU viewport presents colored geometry',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto('./');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  expect(await page.evaluate(()=>window.cuemol.backend)).toBe('webgpu');
  await load(page,'webgpu.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  await expect.poll(async()=>{
    const png=PNG.sync.read(await page.screenshot({clip:{x:rect[0],y:rect[1],width:rect[2],height:rect[3]}}));
    let colored=0;
    for(let i=0;i<png.data.length;i+=4)if(Math.max(...png.data.subarray(i,i+3))-Math.min(...png.data.subarray(i,i+3))>40)colored++;
    return colored;
  }).toBeGreaterThan(500);
  await page.screenshot({path:testInfo.outputPath('webgpu.png')});
  expect(errors).toEqual([]);
});
function dcd(frames){
  const chunks=[];
  const record=data=>{const n=Buffer.alloc(4);n.writeInt32LE(data.length);chunks.push(n,data,n);};
  const header=Buffer.alloc(84);header.write('CORD');header.writeInt32LE(frames,4);record(header);record(Buffer.alloc(4));
  const count=Buffer.alloc(4);count.writeInt32LE(3);record(count);
  for(let frame=0;frame<frames;frame++){for(let axis=0;axis<3;axis++){const coords=Buffer.alloc(12);for(let i=0;i<3;i++)coords.writeFloatLE(frame+i+axis*.1,i*4);record(coords);}}
  return Buffer.concat(chunks);
}
test('wgpu structure display, mouse controls, commands, undo, and embedded scene export',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
  await ready(page);await load(page,'ligand.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  let state=await page.evaluate(()=>window.cuemol.status());
  expect(state.scene.objects[0].atoms).toBe(3);
  expect(state.scene.objects[0].renderers[0].type).toBe('ballstick');
  const viewport=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  const png=PNG.sync.read(await page.screenshot({clip:{x:viewport[0],y:viewport[1],width:viewport[2],height:viewport[3]}}));
  let colored=0;for(let i=0;i<png.data.length;i+=4){if(Math.max(...png.data.subarray(i,i+3))-Math.min(...png.data.subarray(i,i+3))>40)colored++;}
  expect(colored).toBeGreaterThan(500);
  const object=state.scene.objects[0].id;
  const initial=state.view.zoom;
  await page.mouse.move(480,400);await page.mouse.wheel(0,-120);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.zoom)).not.toBe(initial);
  const slab=(await page.evaluate(()=>window.cuemol.status())).view.slab;
  await page.keyboard.down('Shift');await page.mouse.down();await page.mouse.move(540,400,{steps:6});await page.mouse.up();await page.keyboard.up('Shift');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.slab)).not.toBe(slab);
  state=await call(page,'create_renderer',{objId:object,rendererType:'cpk'});
  expect(state.scene.objects[0].renderers).toHaveLength(2);
  state=await call(page,'undo');expect(state.scene.objects[0].renderers).toHaveLength(1);
  state=await call(page,'redo');expect(state.scene.objects[0].renderers).toHaveLength(2);
  state=await call(page,'count_selection',{objId:object,selection:'elem C'});expect(JSON.parse(state.message).count).toBe(1);
  state=await call(page,'get_screenshot',{width:32,height:32});
  const screenshot=JSON.parse(state.message);expect(Buffer.from(screenshot._image,'base64').subarray(1,4).toString()).toBe('PNG');
  await page.screenshot({path:testInfo.outputPath('viewer.png')});
  const download=page.waitForEvent('download');const button=await page.evaluate(()=>window.cuemol.status().ui.rects['Save Scene']);await page.mouse.click(button[0]+button[2]/2,button[1]+button[3]/2);const saved=await download;
  const stream=await saved.createReadStream();const chunks=[];for await(const chunk of stream)chunks.push(chunk);
  await load(page,'roundtrip.qsc',Buffer.concat(chunks));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('Loaded roundtrip.qsc');
  expect((await page.evaluate(()=>window.cuemol.status())).scene.objects[0].atoms).toBe(3);
  expect(errors).toEqual([]);
});
test('browser trajectory keeps a File and seeks individual frames',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
  await ready(page);await load(page,'topology.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await load(page,'trajectory.dcd',dcd(5));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Indexed 5 frames');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(false);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0].id;
  await call(page,'set_frame',{objId:object,frame:4});
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().frame)).toBe(4);
  await call(page,'set_frame',{objId:object,frame:1});
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().frame)).toBe(1);
  await page.screenshot({path:testInfo.outputPath('trajectory.png')});
  expect(errors).toEqual([]);
});

test('browser startup scripts, shared properties, and animation use the native command API',async({page})=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>localStorage.setItem('cuemol.startup',"print('startup ' + (Date.now() > 0));"));
  await ready(page);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('startup true');
  await load(page,'scripted.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0].id;
  let state=await call(page,'execute_script',{source:"print(21 * 2);"});expect(JSON.parse(state.message).log).toContain('42');
  await call(page,'define_selection',{name:'carbon',selection:'elem C'});
  state=await call(page,'count_selection',{objId:object,selection:'carbon'});expect(JSON.parse(state.message).atomCount).toBe(1);
  await call(page,'set_node_prop',{nodeType:'object',nodeId:object,prop:'name',value:'renamed'});
  state=await call(page,'get_scene_state');expect(state.scene.objects[0].name).toBe('renamed');
  await call(page,'add_anim',{type:'spin',name:'turn'});
  await call(page,'set_anim_time',{element:'turn',startMs:0,endMs:2000});
  const before=(await page.evaluate(()=>window.cuemol.status())).view.rotation;
  state=await call(page,'animate',{action:'seek',timeMs:1000});expect(state.view.rotation).not.toEqual(before);
  await call(page,'animate',{action:'play'});
  await page.waitForTimeout(200);
  state=await call(page,'list_anims');expect(JSON.parse(state.message).mgr.elapsedMs).toBeGreaterThan(1000);
  await call(page,'animate',{action:'stop'});
  expect(errors).toEqual([]);
});

test('AMBER ASCII frames and periodic cells are read through browser slices',async({page})=>{
  await ready(page);await load(page,'topology.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  let data='AMBER trajectory\n';
  for(let frame=0;frame<5;frame++){
    data+=Array.from({length:9},(_,i)=>(frame+i*.25).toFixed(3).padStart(8)).join('')+'\n';
    data+=[30+frame,40,50].map(x=>x.toFixed(3).padStart(8)).join('')+'\n';
  }
  await load(page,'trajectory.mdcrd',Buffer.from(data));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Indexed 5 frames');
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0].id;
  for(const frame of [4,0,2]){
    await call(page,'set_frame',{objId:object,frame});
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().frame)).toBe(frame);
  }
});

test('USalign fits rotated browser molecules with the native calculation kernel',async({page})=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await ready(page);
  const points=Array.from({length:12},(_,i)=>[Math.cos(i*.8)*4,Math.sin(i*.8)*4,i*1.5]);
  const moving=points.map(([x,y,z])=>[-y+8,x-3,z+2]);
  const pdb=xyz=>xyz.map(([x,y,z],i)=>'ATOM  '+String(i+1).padStart(5)+'  CA  ALA A'+String(i+1).padStart(4)+'    '+[x,y,z].map(v=>v.toFixed(3).padStart(8)).join('')+'  1.00 10.00           C  ').join('\n')+'\nEND\n';
  await load(page,'reference.pdb',Buffer.from(pdb(points)));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await load(page,'moving.pdb',Buffer.from(pdb(moving)));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(2);
  const objects=(await page.evaluate(()=>window.cuemol.status())).scene.objects;
  const state=await call(page,'superpose',{refId:objects[0].id,movId:objects[1].id,algo:'USalign'});
  const result=JSON.parse(state.message);
  expect(result.algorithm).toBe('USalign');
  const matrix=result.matrix.slice(1,-1).split(',').map(Number);
  expect(matrix).toHaveLength(16);
  for(let i=0;i<points.length;i++)for(let row=0;row<3;row++){
    const value=moving[i].reduce((sum,x,col)=>sum+matrix[col*4+row]*x,matrix[12+row]);
    expect(value).toBeCloseTo(points[i][row],3);
  }
  const values=await page.evaluate(([moving,reference])=>Array.from(window.cuemolKernels.usalign(
    new Float64Array(moving.flat()),new TextEncoder().encode('ALA'.repeat(moving.length)),
    new Float64Array(reference.flat()),new TextEncoder().encode('ALA'.repeat(reference.length)),'CA')),[moving,points]);
  expect(values[12]).toBeCloseTo(1,7);expect(values[13]).toBeCloseTo(1,7);
  expect(values[14]).toBeCloseTo(0,5);expect(values[15]).toBe(12);
  expect(errors).toEqual([]);
});

test('ICC profiles load, proof colors, and persist in browser storage',async({page})=>{
  await ready(page);
  const profile=await page.evaluate(async()=>{
    const {default:create}=await import('/pkg/kernels/lcms.js');
    const cms=await create();
    const handle=cms._cmsCreate_sRGBProfile(), size=cms._malloc(4);
    try {
      cms._cmsSaveProfileToMem(handle,0,size);
      const length=cms.HEAPU32[size/4],data=cms._malloc(length);
      try {cms._cmsSaveProfileToMem(handle,data,size);return Array.from(cms.HEAPU8.subarray(data,data+length));}
      finally {cms._free(data);}
    } finally {cms._free(size);cms._cmsCloseProfile(handle);}
  });
  await load(page,'proof.icc',Buffer.from(profile));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('Loaded ICC profile proof.icc');
  const result=await page.evaluate(profile=>{
    const handle=window.cuemolKernels.cmsCreate(new Uint8Array(profile),0);
    try {return {color:window.cuemolKernels.cmsColor(handle,0x4080c0,false),info:window.cuemolKernels.cmsInfo(handle)};}
    finally {window.cuemolKernels.cmsDestroy(handle);}
  },profile);
  expect(result.info).toContain('sRGB');
  for(const shift of [0,8,16])expect(Math.abs(((result.color>>shift)&255)-((0x4080c0>>shift)&255))).toBeLessThanOrEqual(1);
  await ready(page);await load(page,'ligand.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  let state=await call(page,'set_node_prop',{nodeType:'scene',prop:'use_colproof',value:'true'});
  expect(state.message).not.toContain('Error');
  state=await call(page,'get_screenshot',{width:32,height:32});
  expect(JSON.parse(state.message)._image).toBeTruthy();
  await load(page,'broken.icc',Buffer.alloc(128));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('LittleCMS rejected');
  expect(await page.evaluate(()=>!!localStorage.getItem('cuemol.icc.proof.icc'))).toBe(true);
});

test('browser CMYK proof matches the reference LittleCMS colors',async({page})=>{
  const file=new URL('../../../cuemol2/src/xul_gui/data/icc_profiles/GenericCMYK.icm',import.meta.url);
  test.skip(!existsSync(file),'Optional CueMol2 profile is absent');
  await ready(page);
  const result=await page.evaluate(profile=>{
    const handle=window.cuemolKernels.cmsCreate(new Uint8Array(profile),0);
    try {return [0xffffff,0x777777,0x0000ff,0xff00ff].map(rgb=>window.cuemolKernels.cmsColor(handle,rgb,false));}
    finally {window.cuemolKernels.cmsDestroy(handle);}
  },Array.from(readFileSync(file)));
  expect(result).toEqual([0xf2f3fa,0x767578,0x345ba0,0xb469a4]);
});

test('browser analytic surfaces support both backends and density regeneration',async({page})=>{
  await ready(page);await load(page,'surface-source.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0].id;
  for(const backend of ['meshms','ball']) {
    let state=await call(page,'create_surface',{objId:object,name:backend,backend,density:1.5});
    let result=JSON.parse(state.message);
    expect(result.backend).toBe(backend);
    state=await call(page,'recalc_surface',{surfId:result.objectId,density:3});
    result=JSON.parse(state.message);
    expect(result.backend).toBe(backend);expect(result.triangles).toBeGreaterThan(0);
  }
});

test('browser direct surfaces use EDTSurf and MeshMS with stable atom ownership',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await ready(page);await load(page,'direct-surface.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  let state=await page.evaluate(()=>window.cuemol.status());
  const object=state.scene.objects[0].id;
  await call(page,'delete_node',{nodeType:'renderer',nodeId:state.scene.objects[0].renderers[0].id});
  state=await call(page,'create_renderer',{objId:object,rendererType:'dsurface'});
  const renderer=state.scene.objects[0].renderers[0].id;
  const images=[];
  for(const algorithm of ['distfield','edtsurf','meshms']) {
    state=await call(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer,prop:'surfalgor',value:algorithm});
    expect(state.message).not.toMatch(/error|failed/i);
    await page.waitForTimeout(300);
    const rect=state.ui.rects.Viewport;
    const png=PNG.sync.read(await page.screenshot({clip:{x:rect[0],y:rect[1],width:rect[2],height:rect[3]}}));
    let colored=0;
    for(let i=0;i<png.data.length;i+=4)if(Math.max(...png.data.subarray(i,i+3))-Math.min(...png.data.subarray(i,i+3))>40)colored++;
    expect(colored).toBeGreaterThan(100);
    images.push(png.data);
  }
  expect(images[0].equals(images[1])).toBe(false);
  expect(images[0].equals(images[2])).toBe(false);
  const kernels=await page.evaluate(()=>[3,4,5,6].map(backend=>{
    const result=window.cuemolKernels.surface(new Float64Array([0,0,0,1.7,2,0,0,1.52]),2,1.4,backend);
    return {vertices:result[0].length/6,owners:Array.from(new Set(result[3]))};
  }));
  for(const result of kernels){expect(result.vertices).toBeGreaterThan(0);expect(result.owners.sort()).toEqual([0,1]);}
  await page.screenshot({path:testInfo.outputPath('direct-surface.png')});
  expect(errors).toEqual([]);
});

test('browser render app data stays live across scripts, commands, and scene reloads',async({page})=>{
  await ready(page);await load(page,'render-settings.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_render_settings',{settings:'width=640; povray.shadow=true; umbreon_npr.hatchDensity=2.5'});
  let state=await call(page,'execute_script',{source:"var s=scene.getAppData('render'); s.umbreon.giSamples=16; print(s.width+':'+s.povray.shadow+':'+s.umbreon_npr.hatchDensity);"});
  expect(JSON.parse(state.message).log).toBe('640:true:2.5\n');
  const download=page.waitForEvent('download');
  const button=state.ui.rects['Save Scene'];
  await page.mouse.click(button[0]+button[2]/2,button[1]+button[3]/2);
  const saved=await download;const stream=await saved.createReadStream();const chunks=[];
  for await(const chunk of stream)chunks.push(chunk);
  const bytes=Buffer.concat(chunks);
  expect(bytes.toString()).toContain('type="RenderSettings"');
  await load(page,'render-roundtrip.qsc',bytes);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('Loaded render-roundtrip.qsc');
  state=await call(page,'execute_script',{source:"var s=scene.getAppData('render'); print(s.width+':'+s.umbreon.giSamples+':'+s.umbreon_npr.hatchDensity);"});
  expect(JSON.parse(state.message).log).toBe('640:16:2.5\n');
});

test('browser NPR styles and edited hatch layers produce distinct transparent images',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await ready(page);await load(page,'npr-source.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  let state=await call(page,'get_hatch_style',{style:'richardson'});
  const styles=JSON.parse(state.message).styles;
  expect(styles).toHaveLength(9);
  const images=[];
  for(const style of styles){
    await call(page,'set_render_settings',{settings:`backend=umbreon_npr; width=72; height=72; transparentBg=true; umbreon_npr.supersample=2; umbreon_npr.hatchStyle=${style}`});
    state=await call(page,'get_screenshot',{});const image=JSON.parse(state.message)._image;
    const bytes=Buffer.from(image,'base64');const decoded=PNG.sync.read(bytes);
    expect(decoded.width).toBe(72);expect(decoded.height).toBe(72);
    const alpha=Array.from({length:72*72},(_,i)=>decoded.data[i*4+3]);
    expect(alpha.some(v=>v===0)).toBe(true);expect(alpha.some(v=>v===255)).toBe(true);
    if(style==='richardson')await testInfo.attach('richardson',{body:bytes,contentType:'image/png'});
    images.push(image);
  }
  expect(new Set(images).size).toBeGreaterThanOrEqual(7);
  const spec='layer:kind=line,angle=20,spacing=8; layer:kind=dot,spacing=10';
  await call(page,'set_render_settings',{settings:`umbreon_npr.hatchLayersSpec=${JSON.stringify(spec)}; umbreon_npr.hatchToneSpec="tone:strength=0.4,curve=0.8"`});
  state=await call(page,'get_screenshot',{});expect(images).not.toContain(JSON.parse(state.message)._image);
  expect(errors).toEqual([]);
});

test('browser lighting settings alter traced illumination',async({page})=>{
  await ready(page);await load(page,'lighting.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const image=async settings=>{
    await call(page,'set_render_settings',{settings:`width=48; height=48; umbreon.supersample=1; umbreon.denoise=None; ${settings}`});
    const state=await call(page,'get_screenshot');return JSON.parse(state.message)._image;
  };
  const dark=await image('umbreon.useGI=false; umbreon.lightIntensity=0.2; umbreon.flashFraction=1; umbreon.ambientFraction=0');
  const bright=await image('umbreon.lightIntensity=1.8');
  expect(bright).not.toBe(dark);
  const gi=await image('umbreon.useGI=true; umbreon.giSamples=8; umbreon.giBounces=2; umbreon.giSkyGradient=true; umbreon.giGroundColor=#7799aa; umbreon.ambientFraction=0.6');
  expect(gi).not.toBe(bright);
  const mean=data=>{const p=PNG.sync.read(Buffer.from(data,'base64')).data;let sum=0;for(let i=0;i<p.length;i+=4)sum+=p[i]+p[i+1]+p[i+2];return sum;};
  expect(mean(bright)).toBeGreaterThan(mean(dark));
});

test('OIDN uses bundled WebGPU weights and preserves transparent output',async({page})=>{
  await ready(page);await load(page,'oidn.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_render_settings',{settings:'backend=umbreon; supersample=1; useGI=true; giSamples=2; giBounces=1; transparentBg=true; denoise=None'});
  const raw=JSON.parse((await call(page,'get_screenshot',{width:48,height:48})).message)._image;
  await call(page,'set_render_settings',{settings:'denoise=OIDN'});
  const state=await call(page,'get_screenshot',{width:48,height:48});
  const filtered=JSON.parse(state.message)._image;
  expect(filtered).not.toBe(raw);
  const a=PNG.sync.read(Buffer.from(raw,'base64')),b=PNG.sync.read(Buffer.from(filtered,'base64'));
  expect(b.width).toBe(48);expect(b.height).toBe(48);
  for(let i=3;i<a.data.length;i+=4)expect(b.data[i]).toBe(a.data[i]);
  expect(await page.evaluate(()=>performance.getEntriesByType('resource').some(e=>e.name.includes('rt_hdr_calb_cnrm.tza')))).toBe(true);
});

test('browser rendering yields to mouse input and can be cancelled',async({page})=>{
  await ready(page);await load(page,'responsive.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_render_settings',{settings:'backend=umbreon; supersample=4; useGI=true; giSamples=256; giBounces=4; denoise=None'});
  const zoom=await page.evaluate(()=>window.cuemol.status().view.zoom);
  await page.evaluate(()=>window.cuemol.command('get_screenshot',{width:384,height:384}));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(true);
  await page.mouse.move(480,400);await page.mouse.wheel(0,-120);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.zoom)).not.toBe(zoom);
  expect(await page.evaluate(()=>window.cuemol.status().busy)).toBe(true);
  const state=await call(page,'cancel_render');expect(state.message).toContain('Render cancelled');
  await call(page,'set_render_settings',{settings:'useGI=false; supersample=1'});
  const image=JSON.parse((await call(page,'get_screenshot',{width:32,height:32})).message)._image;
  expect(PNG.sync.read(Buffer.from(image,'base64')).width).toBe(32);
});

function agentSse(output,text='') {
  return [
    {type:'response.output_text.delta',delta:text},
    {type:'response.completed',response:{status:'completed',output,usage:{input_tokens:20,output_tokens:5}}},
  ].map(value=>`data: ${JSON.stringify(value)}\n\n`).join('');
}
const agentCall=(name,args,id)=>({type:'function_call',call_id:id,name,arguments:JSON.stringify(args)});
async function startAgent(page,prompt) {
  await page.evaluate(prompt=>window.cuemol.agent.start(prompt,{enabled:true,model:'openai:test-model',reasoning:'default',endpoint:`${location.origin}/mock-api`},'mock-browser-key'),prompt);
}
test('browser agent streams, downloads structures, sees rendered images, and groups undo',async({page},testInfo)=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  let round=0, imageSeen=false;
  await page.route('**/agent-structure.pdb',route=>route.fulfill({contentType:'text/plain',body:structure}));
  await page.route('**/mock-api/responses',async route=>{
    const request=route.request().postDataJSON();
    expect(route.request().headers().authorization).toBe('Bearer mock-browser-key');
    round++;
    let output;
    if(round===1) output=[agentCall('enable_toolsets',{toolsets:['files']},'enable'),agentCall('fetch_url',{url:new URL('/agent-structure.pdb',route.request().url()).href,reader:'pdb',name:'Agent ligand',rendererType:'cpk',selection:'elem C'},'fetch')];
    else if(round===2) output=[agentCall('capture_view',{longSide:256},'image')];
    else {
      imageSeen=JSON.stringify(request.input).includes('data:image/png;base64,');
      await new Promise(resolve=>setTimeout(resolve,350));
      output=[{type:'message',role:'assistant',content:[{type:'output_text',text:'Prepared the ligand.',annotations:[]}]}];
    }
    await route.fulfill({contentType:'text/event-stream',body:agentSse(output,round===3?'Prepared the ligand.':'Working. ')});
  });
  await ready(page);
  const before=(await page.evaluate(()=>window.cuemol.status())).view.zoom;
  await startAgent(page,'Download the structure, show carbon atoms as spheres, and check the image.');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await page.mouse.move(1000,650);await page.mouse.wheel(0,-120);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().agent.running)).toBe(false);
  const state=await page.evaluate(()=>window.cuemol.status());
  expect(state.view.zoom).not.toBe(before);
  expect(state.scene.objects[0].name).toBe('Agent ligand');
  expect(state.scene.objects[0].renderers).toHaveLength(1);
  expect(state.scene.objects[0].renderers[0].type).toBe('cpk');
  expect(state.agent.messages.some(m=>m.image)).toBe(true);
  expect(state.agent.messages.some(m=>m.text.includes('Prepared the ligand.'))).toBe(true);
  expect(imageSeen).toBe(true);
  expect(await page.evaluate(()=>JSON.stringify(localStorage))).not.toContain('mock-browser-key');
  await page.screenshot({path:testInfo.outputPath('agent.png')});
  expect((await call(page,'undo')).scene.objects).toHaveLength(0);
  expect((await call(page,'redo')).scene.objects).toHaveLength(1);
  expect(errors).toEqual([]);
});

test('browser agent cancellation keeps completed edits and rejects late tool calls',async({page})=>{
  await ready(page);await load(page,'ligand.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0].id;
  let round=0, release;
  const pending=new Promise(resolve=>{release=resolve;});
  await page.route('**/mock-api/responses',async route=>{
    round++;
    if(round===1) await route.fulfill({contentType:'text/event-stream',body:agentSse([agentCall('set_node_prop',{nodeType:'object',nodeId:object,prop:'name',value:'Completed edit'},'first')])});
    else {
      await pending;
      try {await route.fulfill({contentType:'text/event-stream',body:agentSse([agentCall('delete_node',{nodeType:'object',nodeId:object},'late')])});}catch{}
    }
  });
  await startAgent(page,'Edit this object.');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects[0].name)).toBe('Completed edit');
  await expect.poll(()=>round).toBe(2);
  await page.evaluate(()=>window.cuemol.agent.cancel());
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().agent.running)).toBe(false);
  release();await page.waitForTimeout(150);
  let state=await page.evaluate(()=>window.cuemol.status());
  expect(state.scene.objects).toHaveLength(1);expect(state.scene.objects[0].name).toBe('Completed edit');
  state=await call(page,'undo');expect(state.scene.objects[0].name).not.toBe('Completed edit');
});

test('browser agent seeks an indexed trajectory and undo restores the previous coordinates',async({page})=>{
  await ready(page);await load(page,'topology.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await load(page,'agent.dcd',dcd(5));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toContain('Indexed 5 frames');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(false);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0].id;
  let round=0;
  await page.route('**/mock-api/responses',route=>{
    round++;
    const output=round===1?[agentCall('enable_toolsets',{toolsets:['trajectory']},'enable'),agentCall('set_frame',{objId:object,frame:4},'frame')]:[];
    return route.fulfill({contentType:'text/event-stream',body:agentSse(output)});
  });
  await startAgent(page,'Show frame 4.');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().frame)).toBe(4);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().agent.running)).toBe(false);
  expect((await page.evaluate(()=>window.cuemol.status())).agent.messages.some(m=>m.role==='Error')).toBe(false);
  expect((await call(page,'undo')).frame).toBe(0);
  expect((await call(page,'redo')).frame).toBe(4);
});

test('Tritium Inspector edits the selected object and toolbar undo restores it',async({page},testInfo)=>{
  await ready(page);await load(page,'inspector.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const object=(await page.evaluate(()=>window.cuemol.status())).scene.objects[0];
  const clickRect=async key=>{const r=await page.evaluate(key=>window.cuemol.status().ui.rects[key],key);await page.mouse.click(r[0]+r[2]/2,r[1]+r[3]/2);};
  await clickRect(`object:${object.id}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.inspector.name?.length)).toBe(4);
  const field=await page.evaluate(()=>window.cuemol.status().ui.inspector.name);
  await page.mouse.click(field[0]+field[2]/2,field[1]+field[3]/2);
  await page.keyboard.press('ControlOrMeta+a');await page.keyboard.type('Inspector edit');
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await page.keyboard.press('Tab');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects[0].name)).toBe('Inspector edit');
  await clickRect('Undo');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects[0].name)).toBe(object.name);
  await page.screenshot({path:testInfo.outputPath('tritium-inspector.png')});
});

test('Tritium viewport selection and right-drag pan match the native controls',async({page})=>{
  await ready(page);await load(page,'selection.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  const rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  const selection=await page.evaluate(()=>window.cuemol.status().ui.rects['Rect Select']);
  await page.mouse.click(selection[0]+selection[2]/2,selection[1]+selection[3]/2);
  await page.mouse.move(rect[0]+80,rect[1]+20);await page.mouse.down();
  await page.mouse.move(rect[0]+rect[2]-20,rect[1]+rect[3]-20,{steps:12});await page.mouse.up();
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().selection)).toMatch(/^aid /);
  await call(page,'undo');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().selection)).toBe('');
  const before=await page.evaluate(()=>window.cuemol.status().view);
  await page.mouse.move(rect[0]+rect[2]/2,rect[1]+rect[3]/2);await page.mouse.down({button:'right'});
  await page.mouse.move(rect[0]+rect[2]/2,rect[1]+rect[3]/2+80,{steps:8});await page.mouse.up({button:'right'});
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.center)).not.toEqual(before.center);
  expect((await page.evaluate(()=>window.cuemol.status())).view.zoom).toBe(before.zoom);
});

test('pointer threshold, axis locking and wheel scaling match C++ camera input',async({page})=>{
  await page.addInitScript(()=>localStorage.setItem('cuemol.config','view_input:device\tmouse'));
  await ready(page);await load(page,'input.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_view',{zoom:50,slab:50,distance:200,center:[0,0,0]});
  const rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  const x=Math.ceil(rect[0]+rect[2]/2),y=Math.ceil(rect[1]+rect[3]/2);
  const initial=(await page.evaluate(()=>window.cuemol.status())).view;
  await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+1,y+1);
  await page.waitForTimeout(40);
  expect((await page.evaluate(()=>window.cuemol.status())).view.rotation).toEqual(initial.rotation);
  await page.mouse.move(x+2,y+1);await page.waitForTimeout(40);
  expect((await page.evaluate(()=>window.cuemol.status())).view.rotation).toEqual(initial.rotation);
  await page.mouse.move(x+12,y+5);await page.mouse.up();
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.rotation)).not.toEqual(initial.rotation);
  await page.keyboard.down('Shift');await page.mouse.move(x,y);await page.mouse.down();
  await page.mouse.move(x+3,y);await page.mouse.move(x+23,y+1);await page.mouse.move(x+24,y+31);await page.mouse.up();await page.keyboard.up('Shift');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.slab)).toBeCloseTo(50*1.1*1.005,9);
  expect((await page.evaluate(()=>window.cuemol.status())).view.zoom).toBe(50);
  await page.mouse.wheel(0,-120);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.zoom)).toBe(38);
  expect((await page.evaluate(()=>window.cuemol.status())).view.distance).toBe(200);
  const before=(await page.evaluate(()=>window.cuemol.status())).view;
  await page.keyboard.down('Shift');await page.mouse.wheel(0,120);await page.keyboard.up('Shift');await page.waitForTimeout(60);
  expect((await page.evaluate(()=>window.cuemol.status())).view).toEqual(before);
  await call(page,'execute_script',{source:"getService('ViewInputConfig').conf_zoom='SHIFT|WHEEL2';"});
  await page.keyboard.down('Shift');await page.mouse.wheel(0,100);await page.keyboard.up('Shift');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.zoom)).toBeCloseTo(45.6,9);
});

test('saved trackpad input pans, pinches and survives a reload',async({page})=>{
  await page.addInitScript(()=>localStorage.setItem('cuemol.config','view_input:device\ttrackpad\ncuemol2.ui.viewinconf\tDefaultViewInConf'));
  await ready(page);await load(page,'trackpad.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_view',{zoom:50,slab:50,distance:200,center:[0,0,0]});
  let rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  await page.mouse.move(rect[0]+rect[2]/2,rect[1]+rect[3]/2);await page.mouse.wheel(20,40);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.center[0])).toBeCloseTo(-8*50/rect[3],9);
  let view=(await page.evaluate(()=>window.cuemol.status())).view;
  expect(view.center[1]).toBeCloseTo(16*50/rect[3],9);expect(view.zoom).toBe(50);
  await page.keyboard.down('Control');await page.mouse.wheel(0,-10);await page.keyboard.up('Control');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.zoom)).toBe(42);
  await page.reload();await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId)).toBeGreaterThan(0);
  rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  const initial=(await page.evaluate(()=>window.cuemol.status())).view;
  await page.mouse.move(rect[0]+rect[2]/2,rect[1]+rect[3]/2);await page.mouse.wheel(0,40);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.center)).not.toEqual(initial.center);
  expect((await page.evaluate(()=>window.cuemol.status())).view.zoom).toBe(initial.zoom);
});

test('input settings capture their own events and persist across reloads',async({page})=>{
  await ready(page);await load(page,'settings.pdb',Buffer.from(structure));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await call(page,'set_view',{zoom:50,slab:50,distance:200,center:[0,0,0]});
  const clickRect=async key=>{
    await expect.poll(()=>page.evaluate(key=>window.cuemol.status().ui.rects[key]?.length,key)).toBe(4);
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const r=await page.evaluate(key=>window.cuemol.status().ui.rects[key],key);
    await page.mouse.click(r[0]+r[2]/2,r[1]+r[3]/2);
  };
  await clickRect('Tools menu');await clickRect('Settings');
  await clickRect('Input device');await clickRect('input:view_input:device:trackpad');
  await expect.poll(()=>page.evaluate(()=>localStorage.getItem('cuemol.config'))).toContain('view_input:device\ttrackpad');
  const before=(await page.evaluate(()=>window.cuemol.status())).view;
  const field=await page.evaluate(()=>window.cuemol.status().ui.rects['Input device']);
  await page.mouse.move(field[0]+field[2]/2,field[1]+field[3]/2);await page.mouse.wheel(0,120);
  await page.waitForTimeout(80);
  expect((await page.evaluate(()=>window.cuemol.status())).view).toEqual(before);
  await page.reload();await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId)).toBeGreaterThan(0);
  const rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
  const initial=(await page.evaluate(()=>window.cuemol.status())).view;
  await page.mouse.move(rect[0]+rect[2]/2,rect[1]+rect[3]/2);await page.mouse.wheel(0,40);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.center)).not.toEqual(initial.center);
  expect((await page.evaluate(()=>window.cuemol.status())).view.zoom).toBe(initial.zoom);
});

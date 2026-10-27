import {test,expect} from '@playwright/test';
import {PNG} from 'pngjs';
const structure=`HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00 10.00           C  
HETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00 10.00           O  
HETATM    3  N1  LIG A   1       1.300   1.000   0.000  1.00 10.00           N  
CONECT    1    2
CONECT    2    3
END
`;
async function ready(page){
  await page.goto('/?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>!!window.cuemol),{timeout:60000}).toBe(true);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.sceneId)).toBeGreaterThan(0);
}
async function load(page,name,buffer){
  await page.locator('#files').setInputFiles({name,mimeType:'application/octet-stream',buffer});
}
async function call(page,name,args={}){
  await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await page.waitForTimeout(100);
  return page.evaluate(()=>window.cuemol.status());
}
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
  const png=PNG.sync.read(await page.screenshot({clip:{x:80,y:80,width:700,height:650}}));
  let colored=0;for(let i=0;i<png.data.length;i+=4){if(Math.max(...png.data.subarray(i,i+3))-Math.min(...png.data.subarray(i,i+3))>40)colored++;}
  expect(colored).toBeGreaterThan(500);
  const object=state.scene.objects[0].id;
  const initial=state.view.zoom;
  await page.mouse.move(480,400);await page.mouse.wheel(0,-120);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.zoom)).not.toBe(initial);
  const slab=(await page.evaluate(()=>window.cuemol.status())).view.slab;
  await page.keyboard.down('Shift');await page.mouse.wheel(0,120);await page.keyboard.up('Shift');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().view.slab)).not.toBe(slab);
  state=await call(page,'create_renderer',{objId:object,rendererType:'cpk'});
  expect(state.scene.objects[0].renderers).toHaveLength(2);
  state=await call(page,'undo');expect(state.scene.objects[0].renderers).toHaveLength(1);
  state=await call(page,'redo');expect(state.scene.objects[0].renderers).toHaveLength(2);
  state=await call(page,'count_selection',{objId:object,selection:'elem C'});expect(JSON.parse(state.message).count).toBe(1);
  state=await call(page,'get_screenshot',{width:32,height:32});
  const screenshot=JSON.parse(state.message);expect(Buffer.from(screenshot._image,'base64').subarray(1,4).toString()).toBe('PNG');
  await page.screenshot({path:testInfo.outputPath('viewer.png')});
  const download=page.waitForEvent('download');await page.mouse.click(173,15);const saved=await download;
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

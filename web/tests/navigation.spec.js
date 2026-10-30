import {test,expect} from '@playwright/test';
import {PNG} from 'pngjs';

const points=[[-4,0,0],[-1,0,0],[1,2,0],[4,1,1]];
const pdb=`HETATM    1  C1  LIG A   1      -4.000   0.000   0.000  1.00 10.00           C
HETATM    2  C2  LIG A   1      -1.000   0.000   0.000  1.00 10.00           C
HETATM    3  C3  LIG A   2       1.000   2.000   0.000  1.00 10.00           C
HETATM    4  C4  LIG A   3       4.000   1.000   1.000  1.00 10.00           C
END
`;
const status=page=>page.evaluate(()=>window.cuemol.status());
async function command(page,name,args={}) {
  const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await expect.poll(async()=>{const value=await status(page);return value.lastCommand===id&&!value.busy;}).toBe(true);
  return status(page);
}
async function setup(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await page.locator('#files').setInputFiles({name:'picks.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  await command(page,'set_projection',{perspective:false});
  await command(page,'set_view',{center:[0,0,0],zoom:20,distance:200,slab:50});
  const state=await status(page),r=state.ui.rects.Viewport;
  return {id:state.scene.objects[0].id,points:points.map(([x,y])=>[r[0]+r[2]/2+x*r[3]/20,r[1]+r[3]/2-y*r[3]/20])};
}
async function choose(page,label) {
  await expect.poll(async()=>(await status(page)).ui.rects[label]?.length).toBe(4);
  const r=(await status(page)).ui.rects[label];
  await page.mouse.click(r[0]+r[2]/2,r[1]+r[3]/2);
  await page.waitForTimeout(80);
}
async function pick(page,p) {await page.waitForTimeout(550);await page.mouse.click(...p);await page.waitForTimeout(80);}

test('atom labels, residue double-click, range extension, hover and undo share the viewport',async({page},testInfo)=>{
  const {id,points:p}=await setup(page),before=(await status(page)).view;
  await pick(page,p[0]);
  await expect.poll(async()=>(await status(page)).ui.navigation.labels).toBe(1);
  expect((await status(page)).selection).toBe('');
  await page.mouse.move(...p[2]);
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(2);
  await pick(page,p[0]);
  await expect.poll(async()=>(await status(page)).ui.navigation.labels).toBe(0);
  await page.waitForTimeout(550);await page.mouse.dblclick(...p[0],{delay:70});
  await expect.poll(async()=>(await status(page)).selection).not.toBe('');
  let selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(2);
  expect((await status(page)).view.center).toEqual(before.center);
  await page.waitForTimeout(550);await page.keyboard.down('Shift');await page.mouse.dblclick(...p[3],{delay:70});await page.keyboard.up('Shift');
  await expect.poll(async()=>(await status(page)).selection).not.toBe(selection);
  selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(4);
  await command(page,'undo');
  selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(2);
  await page.mouse.click(...p[2],{button:'right'});await choose(page,'Center at this atom');
  await expect.poll(async()=>(await status(page)).view.center).toEqual([1,2,0]);
  await page.screenshot({path:testInfo.outputPath('navigation-labels.png')});
});

test('browser double-click counts survive a blocked input handler',async({page})=>{
  const {id,points:p}=await setup(page);
  await page.evaluate(()=>{
    window.delayedReleaseTimes=[];
    document.addEventListener('pointerup',event=>{
      window.delayedReleaseTimes.push(event.timeStamp);
      if(window.delayedReleaseTimes.length===1){
        const deadline=performance.now()+700;
        while(performance.now()<deadline){}
      }
    },true);
  });
  await page.mouse.dblclick(...p[0],{delay:70});
  const times=await page.evaluate(()=>window.delayedReleaseTimes);
  expect(times).toHaveLength(2);
  expect(times[1]-times[0]).toBeGreaterThan(500);
  await expect.poll(async()=>(await status(page)).selection).not.toBe('');
  const selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(2);
});

test('lasso and rectangle selection add with Shift and retain click behavior',async({page})=>{
  const {id,points:p}=await setup(page),before=(await status(page)).view;
  async function box(point,lasso) {
    await page.mouse.move(point[0]-14,point[1]-14);await page.mouse.down();
    if(lasso){await page.mouse.move(point[0]+14,point[1]-14,{steps:3});await page.mouse.move(point[0]+14,point[1]+14,{steps:3});await page.mouse.move(point[0]-14,point[1]+14,{steps:3});await page.mouse.move(point[0]-14,point[1]-14,{steps:3});}
    else await page.mouse.move(point[0]+14,point[1]+14,{steps:3});
    await page.mouse.up();await page.waitForTimeout(80);
  }
  await choose(page,'Lasso');await box(p[0],true);
  await expect.poll(async()=>(await status(page)).selection).toBe('aid 1');
  await choose(page,'Rect Select');await page.keyboard.down('Shift');await box(p[3],false);await page.keyboard.up('Shift');
  const selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(2);
  expect((await status(page)).view.rotation).toEqual(before.rotation);
  await pick(page,p[2]);await expect.poll(async()=>(await status(page)).ui.navigation.labels).toBe(1);
  await command(page,'undo');await expect.poll(async()=>(await status(page)).ui.navigation.labels).toBe(0);
});

test('distance, angle and torsion picks create visible measurement sets and cancel with Escape',async({page},testInfo)=>{
  const {points:p}=await setup(page);
  for(const [label,count] of [['Distance',2],['Angle',3],['Torsion',4]]) {
    await choose(page,label);
    await pick(page,p[0]);await expect.poll(async()=>(await status(page)).ui.navigation.picks).toBe(1);
    await page.keyboard.press('Escape');await expect.poll(async()=>(await status(page)).ui.navigation.picks).toBe(0);
    for(const point of p.slice(0,count))await pick(page,point);
    await expect.poll(async()=>(await status(page)).ui.navigation.picks).toBe(0);
    expect((await status(page)).message).toContain(label);
    expect((await status(page)).scene.objects[0].renderers.some(r=>r.type==='atomintr')).toBe(true);
  }
  await page.screenshot({path:testInfo.outputPath('measurements.png')});
  await command(page,'undo');await command(page,'undo');await command(page,'undo');
  expect((await status(page)).scene.objects[0].renderers.some(r=>r.type==='atomintr')).toBe(false);
});

test('bond picks reject a repeated atom and support undo',async({page})=>{
  const {points:p}=await setup(page),before=(await status(page)).scene.objects[0].bonds;
  await choose(page,'Add Bond');await pick(page,p[0]);await pick(page,p[0]);
  expect((await status(page)).ui.navigation.picks).toBe(1);
  expect((await status(page)).message).toContain('Cannot bond');
  await pick(page,p[3]);await expect.poll(async()=>(await status(page)).scene.objects[0].bonds).toBe(before+1);
  await command(page,'undo');expect((await status(page)).scene.objects[0].bonds).toBe(before);
});

test('tool shortcuts, cursors and the collapsed palette persist the expected interaction state',async({page},testInfo)=>{
  const {points:p}=await setup(page);
  await page.mouse.click(p[0][0],p[0][1]+60);
  await page.mouse.move(...p[2]);
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(2);
  await page.mouse.wheel(0,-200);
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
  await command(page,'set_view',{center:[0,0,0],zoom:20,distance:200,slab:50});
  for(const [key,label] of [['l','Lasso'],['d','Distance'],['a','Angle'],['t','Torsion'],['e','Add Bond'],['b','Rect Select'],['n','Navigate']]) {
    await page.keyboard.press(key);
    await expect.poll(async()=>(await status(page)).ui.navigation.tool).toBe(label);
    await page.mouse.move(...p[0]);
    await expect.poll(()=>page.locator('canvas').evaluate(e=>getComputedStyle(e).cursor)).toBe(label==='Navigate'?'grab':'crosshair');
  }
  await choose(page,'Hide tool palette');
  await expect.poll(async()=>(await status(page)).ui.rects['Distance']).toBeUndefined();
  await page.reload();
  await expect.poll(()=>page.evaluate(()=>Boolean(window.cuemol?.status()?.ui)),{timeout:60000}).toBe(true);
  await expect.poll(async()=>(await status(page)).ui.rects['Show tool palette']?.length,{timeout:60000}).toBe(4);
  await choose(page,'Show tool palette');
  await expect.poll(async()=>(await status(page)).ui.rects['Distance']?.length).toBe(4);
  await page.screenshot({path:testInfo.outputPath('palette.png')});
});

test('short viewports keep the tool palette above the console and allow scrolling to the last tool',async({page},testInfo)=>{
  await page.setViewportSize({width:960,height:600});
  await setup(page);
  const labels=['Hide tool palette','Navigate','Rect Select','Lasso','Measure options','Distance','Angle','Torsion','Bond editor options','Add Bond'];
  function checkBounds(state) {
    const viewport=state.ui.rects.Viewport,bottom=viewport[1]+viewport[3];
    for(const label of labels) {
      const rect=state.ui.rects[label];
      if(rect)expect(rect[1]+rect[3],`${label} overlaps the console`).toBeLessThanOrEqual(bottom);
    }
  }
  const before=await status(page);
  checkBounds(before);
  expect(before.ui.rects['Add Bond']).toBeUndefined();
  const palette=before.ui.rects['Tool palette'];
  expect(palette).toHaveLength(4);
  const viewport=before.ui.rects.Viewport;
  expect(palette[1]+palette[3]).toBeLessThanOrEqual(viewport[1]+viewport[3]);
  await page.mouse.move(palette[0]+palette[2]/2,palette[1]+palette[3]-20);
  await page.mouse.wheel(0,800);
  await expect.poll(async()=>(await status(page)).ui.rects['Add Bond']?.length).toBe(4);
  const scrolled=await status(page);
  checkBounds(scrolled);
  expect(scrolled.view.zoom).toBe(before.view.zoom);
  await choose(page,'Add Bond');
  await expect.poll(async()=>(await status(page)).ui.navigation.tool).toBe('Add Bond');
  await page.screenshot({path:testInfo.outputPath('short-palette-scrolled.png')});
  await choose(page,'Bond editor options');
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await page.screenshot({path:testInfo.outputPath('short-palette-options.png')});
  await page.keyboard.press('Escape');
  await choose(page,'Hide tool palette');
  await expect.poll(async()=>(await status(page)).ui.rects['Show tool palette']?.length).toBe(4);
  await page.screenshot({path:testInfo.outputPath('short-palette.png')});
});

test('cross-molecule measurement values follow transforms and survive scene reloads',async({page},testInfo)=>{
  const {id}=await setup(page);
  await page.locator('#files').setInputFiles({name:'other.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(2);
  const other=(await status(page)).scene.objects.find(o=>o.id!==id).id;
  await command(page,'set_view',{center:[0,0,0],zoom:20,distance:200,slab:50});
  await command(page,'measure_geometry',{objId:id,atoms:[{chain:'A',resid:'1',atomName:'C1'},{chain:'A',resid:'1',atomName:'C2'}],atomObjects:String(other),draw:true,label:'measure'});
  await expect.poll(async()=>(await status(page)).ui.navigation.measurementLabels).toEqual(['3.00']);
  await command(page,'set_node_prop',{nodeType:'object',nodeId:other,prop:'xformMat',value:'(1,0,0,0,0,1,0,0,0,0,1,0,2,0,0,1)'});
  await expect.poll(async()=>(await status(page)).ui.navigation.measurementLabels).toEqual(['5.00']);
  await command(page,'set_node_prop',{nodeType:'object',nodeId:other,prop:'name',value:'renamed'});
  const pending=page.waitForEvent('download');
  await choose(page,'Save Scene');
  const saved=await pending,stream=await saved.createReadStream(),chunks=[];
  for await(const chunk of stream)chunks.push(chunk);
  const scene=Buffer.concat(chunks);
  expect(scene.toString()).toContain('mol2="renamed"');
  await command(page,'create_scene',{name:'reload'});
  await page.locator('#files').setInputFiles({name:'measure.qsc',mimeType:'application/octet-stream',buffer:scene});
  await expect.poll(async()=>(await status(page)).message).toBe('Loaded measure.qsc');
  await expect.poll(async()=>(await status(page)).ui.navigation.measurementLabels).toEqual(['5.00']);
  const restored=(await status(page)).scene.objects.find(o=>o.name==='renamed').id;
  await command(page,'set_node_prop',{nodeType:'object',nodeId:restored,prop:'xformMat',value:'(1,0,0,0,0,1,0,0,0,0,1,0,4,0,0,1)'});
  await expect.poll(async()=>(await status(page)).ui.navigation.measurementLabels).toEqual(['7.00']);
  await page.screenshot({path:testInfo.outputPath('live-measurements.png')});
});

test('backbone hover ignores hidden sidechains and follows renderer transforms',async({page},testInfo)=>{
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  const protein=`ATOM      1  CA  ALA A   1      -4.000   0.000   0.000  1.00 10.00           C
ATOM      2  CB  ALA A   1      -4.000   3.000   0.000  1.00 10.00           C
ATOM      3  CA  ALA A   2       0.000   0.000   0.000  1.00 10.00           C
ATOM      4  CB  ALA A   2       0.000   3.000   0.000  1.00 10.00           C
ATOM      5  CA  ALA A   3       4.000   0.000   0.000  1.00 10.00           C
ATOM      6  CB  ALA A   3       4.000   3.000   0.000  1.00 10.00           C
ATOM      7  N   ALA A   1      -5.200   0.000   0.000  1.00 10.00           N
ATOM      8  C   ALA A   1      -2.500   0.000   0.000  1.00 10.00           C
ATOM      9  N   ALA A   2      -1.200   0.000   0.000  1.00 10.00           N
ATOM     10  C   ALA A   2       1.500   0.000   0.000  1.00 10.00           C
ATOM     11  N   ALA A   3       2.800   0.000   0.000  1.00 10.00           N
ATOM     12  C   ALA A   3       5.500   0.000   0.000  1.00 10.00           C
END
`;
  await page.locator('#files').setInputFiles({name:'backbone.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(protein)});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  await command(page,'set_projection',{perspective:false});
  await command(page,'set_view',{center:[0,0,0],zoom:20,distance:200,slab:50});
  const state=await status(page),r=state.ui.rects.Viewport,renderer=state.scene.objects[0].renderers.find(r=>r.type==='ribbon');
  const point=(x,y)=>[r[0]+r[2]/2+x*r[3]/20,r[1]+r[3]/2-y*r[3]/20];
  await page.mouse.move(...point(-4,0));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
  await page.mouse.move(...point(-4,3));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'xformMat',value:'(1,0,0,0,0,1,0,0,0,0,1,0,2,0,0,1)'});
  await page.mouse.move(...point(-4,0));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
  await page.mouse.move(...point(-2,0));
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
  await page.screenshot({path:testInfo.outputPath('transformed-backbone-pick.png')});
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'xformMat',value:'(1,0,0,0,0,1,0,0,0,0,1,0,2,0,100,1)'});
  await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
});

for (const backend of ['webgl','webgpu']) {
  test(`${backend} hover follows visible sphere fragments and script hit tests`,async({page},testInfo)=>{
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
    await page.locator('#files').setInputFiles({name:'sphere.pdb',mimeType:'application/octet-stream',buffer:Buffer.from('HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n')});
    await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
    const object=(await status(page)).scene.objects[0].id;
    await command(page,'create_renderer',{objId:object,rendererType:'cpk'});
    const renderer=(await status(page)).scene.objects[0].renderers.find(renderer=>renderer.type==='cpk');
    await command(page,'set_view',{center:[0,0,0],zoom:10,distance:40,slab:20});
    const rect=(await status(page)).ui.rects.Viewport;
    const center=[rect[0]+rect[2]/2,rect[1]+rect[3]/2];
    const edge=[center[0]+rect[3]*.12,center[1]];
    const sample=async()=>{
      const image=PNG.sync.read(await page.screenshot({clip:{x:center[0]+20,y:center[1]+20,width:8,height:8}}));
      return [0,1,2].map(channel=>Array.from({length:64},(_,i)=>image.data[i*4+channel]).reduce((a,b)=>a+b)/64);
    };
    await page.mouse.move(5,5);
    const baseline=await sample();
    const blendError=async(color)=>Math.max(...(await sample()).map((value,index)=>Math.abs(value-(baseline[index]*.65+color[index]*.35))));
    await page.screenshot({path:testInfo.outputPath('before-hover.png')});
    await page.mouse.move(...edge);
    await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
    let reply=await command(page,'execute_script',{source:`var v=scene.getView(scene.activeViewID);print(JSON.parse(v.hitTest(${edge[0]-rect[0]},${edge[1]-rect[1]})).atom_id);`});
    expect(JSON.parse(reply.message).log).toBe('1\n');
    await expect.poll(()=>blendError([255,102,153])).toBeLessThan(2);
    await page.screenshot({path:testInfo.outputPath('sphere-hover.png')});
    await command(page,'execute_script',{source:"getService('ViewInputConfig').gpu_pick=false;"});
    await page.mouse.move(edge[0]+1,edge[1]);
    await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
    await command(page,'execute_script',{source:"cuemol.resetProp(getService('ViewInputConfig'),'gpu_pick');getService('ViewInputConfig').hover_hl_color=cuemol.makeColor('#0000ff');"});
    await page.mouse.move(edge[0],edge[1]);
    await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
    await expect.poll(()=>blendError([0,0,255])).toBeLessThan(2);
    await page.screenshot({path:testInfo.outputPath('blue-hover.png')});
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'alpha',value:'0.6'});
    await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'alpha',value:'1'});
    await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'locked',value:'true'});
    await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
    await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'locked',value:'false'});
    await page.mouse.move(5,5);
    await expect.poll(async()=>(await status(page)).ui.navigation.hover).toBeNull();
    await command(page,'execute_script',{source:`scene.getView(scene.activeViewID).setHoverHit(${renderer.id},1,-1);`});
    await expect.poll(()=>blendError([0,0,255])).toBeLessThan(2);
    await command(page,'execute_script',{source:'scene.getView(scene.activeViewID).clearHoverHit();'});
    await expect.poll(async()=>Math.max(...(await sample()).map((value,index)=>Math.abs(value-baseline[index])))).toBeLessThan(2);
    expect(errors).toEqual([]);
  });
}

test('nucleic base picking retains the owning residue when another pivot is closer',async({page})=>{
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  const structure=`ATOM      1  C1'   A A   1      -4.000   0.000   0.000  1.00 10.00           C
ATOM      2  N9    A A   1      -2.000   0.000   0.000  1.00 10.00           N
ATOM      3  N1    A A   1       0.000   0.000   0.000  1.00 10.00           N
ATOM      4  C1'   U A   2       1.000   2.000   0.000  1.00 10.00           C
ATOM      5  N1    U A   2       2.000   2.000   0.000  1.00 10.00           N
ATOM      6  N3    U A   2       3.000   2.000   0.000  1.00 10.00           N
END
`;
  await page.locator('#files').setInputFiles({name:'bases.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(structure)});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  const object=(await status(page)).scene.objects[0];
  for(const renderer of object.renderers)await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'visible',value:'false'});
  await command(page,'create_renderer',{objId:object.id,rendererType:'nucl'});
  const renderer=(await status(page)).scene.objects[0].renderers.find(renderer=>renderer.type==='nucl');
  expect(renderer).toBeDefined();
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'show_tube',value:'false'});
  await command(page,'set_node_prop',{nodeType:'renderer',nodeId:renderer.id,prop:'show_basepair',value:'false'});
  await command(page,'set_projection',{perspective:false});
  await command(page,'set_view',{center:[0,0,0],zoom:10,distance:40,slab:20});
  const rect=(await status(page)).ui.rects.Viewport;
  await page.mouse.move(rect[0]+rect[2]/2,rect[1]+rect[3]/2);
  await expect.poll(async()=>(await status(page)).ui.navigation.hover?.index).toBe(0);
  const reply=await command(page,'execute_script',{source:`var v=scene.getView(scene.activeViewID);print(JSON.parse(v.hitTest(${rect[2]/2},${rect[3]/2})).atom_id);`});
  expect(JSON.parse(reply.message).log).toBe('1\n');
});

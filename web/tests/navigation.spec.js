import {test,expect} from '@playwright/test';

const points=[[-4,0,0],[-1,0,0],[1,2,0],[4,1,1]];
const pdb=`HETATM    1  C1  LIG A   1      -4.000   0.000   0.000  1.00 10.00           C
HETATM    2  C2  LIG A   1      -1.000   0.000   0.000  1.00 10.00           C
HETATM    3  C3  LIG A   2       1.000   2.000   0.000  1.00 10.00           C
HETATM    4  C4  LIG A   3       4.000   1.000   1.000  1.00 10.00           C
END
`;
const status=page=>page.evaluate(()=>window.cuemol.status());
async function command(page,name,args={}) {
  await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await page.waitForTimeout(80);
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
  selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(4);
  await command(page,'undo');
  selection=(await status(page)).selection;
  expect(JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count).toBe(2);
  await page.mouse.click(...p[2],{button:'right'});await choose(page,'Center at this atom');
  await expect.poll(async()=>(await status(page)).view.center).toEqual([1,2,0]);
  await page.screenshot({path:testInfo.outputPath('navigation-labels.png')});
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

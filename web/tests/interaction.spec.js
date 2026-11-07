import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

const pdb = Buffer.from('ATOM      1  N   GLY A   1       0.000   0.000   0.000  1.00 10.00           N  \nATOM      2  CA  GLY A   2       3.000   0.000   0.000  1.00 10.00           C  \nEND\n');
async function click(page,name,twice=false) {
  await expect.poll(()=>page.evaluate(name=>window.cuemol.status().ui.rects[name],name)).toBeTruthy();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const r=await page.evaluate(name=>window.cuemol.status().ui.rects[name],name);
  if(twice)await page.mouse.dblclick(r[0]+r[2]/2,r[1]+r[3]/2);
  else await page.mouse.click(r[0]+r[2]/2,r[1]+r[3]/2);
}
async function fill(page,name,value,numeric=false) {
  await click(page,name,numeric);await page.keyboard.press('ControlOrMeta+A');await page.keyboard.type(value);
  if(numeric)await page.keyboard.press('Enter');
}
async function open(page) {await click(page,'Tools menu');await click(page,'Interaction…');await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['Interaction window'])).toBeTruthy();}
async function ready(page,backend) {
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await load(page,'first.pdb',pdb);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  await load(page,'second.pdb',pdb);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(2);
}
for(const backend of ['webgl','webgpu']) {
  test(`interaction dialog applies a second-molecule selection and one undo (${backend})`, async({page},testInfo)=>{
    await ready(page,backend);
    const before=await call(page,'clear_undo');
    await open(page);
    await click(page,'Interaction Molecule');
    await click(page,`Interaction Molecule ${before.scene.objects[0].name}`);
    await fill(page,'Interaction selection 1','name N');
    await click(page,'Interaction second molecule');
    await click(page,'Interaction Molecule 2');
    await click(page,`Interaction Molecule 2 ${before.scene.objects[1].name}`);
    await fill(page,'Interaction selection 2','name N');
    await click(page,'Interaction OK');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('No interaction was found');
    expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toEqual(before.scene.objects);
    await fill(page,'Interaction selection 2','invalid (((');
    await click(page,'Interaction OK');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).not.toBe('No interaction was found');
    expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toEqual(before.scene.objects);
    await fill(page,'Interaction selection 2','name CA');
    await fill(page,'Interaction Min distance (Å)','3.5',true);
    const invalidMessage=await page.evaluate(()=>window.cuemol.status().message);
    await click(page,'Interaction OK');
    expect((await page.evaluate(()=>window.cuemol.status())).message).toBe(invalidMessage);
    await fill(page,'Interaction Min distance (Å)','2.5',true);
    expect(await page.evaluate(()=>window.cuemol.status().ui.rects['Interaction label set'][2])).toBeGreaterThan(200);
    await page.screenshot({path:testInfo.outputPath('interaction-dialog.png')});
    await click(page,'Interaction OK');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('Defined 1 interaction(s)');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['Interaction window'])).toBeFalsy();
    const after=await page.evaluate(()=>window.cuemol.status());
    const renderer=after.scene.objects[0].renderers.find(r=>r.type==='atomintr');
    expect(renderer.name).toBe('measure');
    const labels=await call(page,'list_interactions',{rendId:renderer.id});
    expect(JSON.parse(labels.message).entries).toHaveLength(1);
    const undone=await call(page,'undo');expect(undone.scene.objects).toEqual(before.scene.objects);
    const redone=await call(page,'redo');expect(redone.scene.objects).toEqual(after.scene.objects);
    await open(page);
    await click(page,'Interaction choose selection 1');
    await click(page,'Selection tab History');
    await expect.poll(()=>page.evaluate(()=>Object.keys(window.cuemol.status().ui.rects).some(k=>k.includes('name CA')))).toBe(true);
    await click(page,'Interaction Cancel');
    expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toEqual(after.scene.objects);
  });
}

test('interaction same-molecule gate ignores the unused selection and cancels a stale scene', async({page})=>{
  await ready(page,'webgl');
  const before=await call(page,'clear_undo');
  await open(page);
  await click(page,'Interaction Molecule');await click(page,`Interaction Molecule ${before.scene.objects[0].name}`);
  await click(page,'Interaction second molecule');
  await click(page,'Interaction Molecule 2');await click(page,`Interaction Molecule 2 ${before.scene.objects[0].name}`);
  await fill(page,'Interaction selection 2','invalid (((');
  await click(page,'Interaction OK');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('Defined 1 interaction(s)');
  await open(page);
  await call(page,'create_scene',{name:'Other scene'});
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['Interaction window'])).toBeFalsy();
  expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toHaveLength(0);
});

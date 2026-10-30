import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';

const pdb=`ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C
ATOM      2  CA  GLY A   2      10.000   0.000   0.000  1.00 10.00           C
ATOM      3  CA  SER A   3      20.000   0.000   0.000  1.00 10.00           C
END
`;
const status=page=>page.evaluate(()=>window.cuemol.status());
async function command(page,name,args={}) {
  const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await expect.poll(async()=>{const value=await status(page);return value.lastCommand===id&&!value.busy;}).toBe(true);
  return status(page);
}
async function choose(page,label) {
  await expect.poll(async()=>(await status(page)).ui.rects[label]?.length).toBe(4);
  const [x,y,w,h]=(await status(page)).ui.rects[label];
  await page.mouse.click(x+w/2,y+h/2);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
async function cell(page,column,row=0) {
  const [x,y,w,h]=(await status(page)).ui.rects['Sequence origin'];
  return [x+(column+0.5)*w,y+(row+0.5)*h];
}
async function setup(page,theme='dark') {
  await page.addInitScript(theme=>localStorage.setItem('cuemol.theme',theme),theme);
  await page.goto('./?renderer=webgl');
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await page.locator('#files').setInputFiles({name:'sequence.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(1);
  await choose(page,'Sequence');
  await expect.poll(async()=>(await status(page)).ui.rects['Sequence origin']?.length).toBe(4);
  return (await status(page)).scene.objects[0].id;
}
async function selectionCount(page,id) {
  const selection=(await status(page)).selection;
  if(!selection)return 0;
  return JSON.parse((await command(page,'count_selection',{objId:id,selection})).message).count;
}

for(const theme of ['dark','light'])test(`sequence clicks, shift ranges, drag tracking, context actions and undo match the reference in ${theme}`,async({page,context},testInfo)=>{
  const id=await setup(page,theme);
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await page.mouse.click(...await cell(page,1));
  await expect.poll(async()=>(await status(page)).selection).toBe("'A'.1.*");
  await expect.poll(async()=>(await status(page)).view.center).toEqual([0,0,0]);
  await page.mouse.click(...await cell(page,1));
  await expect.poll(async()=>(await status(page)).selection).toBe('');
  await page.keyboard.down('Shift');await page.mouse.click(...await cell(page,3));await page.keyboard.up('Shift');
  await expect.poll(()=>selectionCount(page,id)).toBe(3);
  expect((await status(page)).view.center).toEqual([0,0,0]);
  const expectedHighlight=await page.evaluate(theme=>{
    const canvas=document.createElement('canvas');canvas.width=canvas.height=1;
    const ctx=canvas.getContext('2d');
    ctx.fillStyle=theme==='dark'?'#252830':'#ffffff';ctx.fillRect(0,0,1,1);
    ctx.fillStyle=theme==='dark'?'rgba(95,175,215,0.5)':'rgba(41,121,181,0.35)';ctx.fillRect(0,0,1,1);
    return [...ctx.getImageData(0,0,1,1).data].slice(0,3);
  },theme);
  await expect.poll(async()=>{
    const [x,y,w,h]=(await status(page)).ui.rects['Sequence origin'];
    const image=PNG.sync.read(await page.screenshot({clip:{x:x+w,y,width:w,height:h}}));
    let matching=0;
    for(let i=0;i<image.data.length;i+=4){
      if(expectedHighlight.every((value,channel)=>Math.abs(image.data[i+channel]-value)<=3))matching++;
    }
    return matching;
  }).toBeGreaterThan(20);
  await page.screenshot({path:testInfo.outputPath('sequence-range.png')});
  await command(page,'undo');await expect.poll(()=>selectionCount(page,id)).toBe(0);
  await command(page,'redo');await expect.poll(()=>selectionCount(page,id)).toBe(3);
  const first=await cell(page,1),second=await cell(page,2);
  await page.mouse.move(...first);await page.mouse.down();
  await page.mouse.move(second[0],second[1]+60,{steps:3});await page.mouse.up();
  await expect.poll(async()=>(await status(page)).selection).toBe("'A'.3.*");
  await page.mouse.click(...await cell(page,3),{button:'right'});
  await expect.poll(async()=>(await status(page)).ui.rects['Toggle sel']?.length).toBe(4);
  await page.screenshot({path:testInfo.outputPath('sequence-context.png')});
  await choose(page,'Toggle sel');await expect.poll(async()=>(await status(page)).selection).toBe('');
  await page.mouse.click(...await cell(page,3),{button:'right'});await choose(page,'Center here');
  await expect.poll(async()=>(await status(page)).view.center).toEqual([20,0,0]);
  await page.mouse.click(...await cell(page,3),{button:'right'});await choose(page,'Copy sequence');
  await expect.poll(()=>page.evaluate(()=>navigator.clipboard.readText())).toBe('AGS');
  await page.mouse.click(...await cell(page,3),{button:'right'});await choose(page,'Invert sel');
  await expect.poll(()=>selectionCount(page,id)).toBe(3);
  await page.mouse.click(...await cell(page,3),{button:'right'});await choose(page,'Unselect all');
  await expect.poll(async()=>(await status(page)).selection).toBe('');
});

test('sequence refreshes after another molecule is loaded and selects the clicked object',async({page},testInfo)=>{
  const first=await setup(page);
  await page.mouse.click(...await cell(page,2));
  await expect.poll(async()=>(await status(page)).selection).toBe("'A'.2.*");
  await page.locator('#files').setInputFiles({name:'other.pdb',mimeType:'application/octet-stream',buffer:Buffer.from(pdb.replaceAll('GLY','VAL'))});
  await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(2);
  const other=(await status(page)).scene.objects.find(object=>object.id!==first).id;
  await page.mouse.click(...await cell(page,3,1));
  await expect.poll(async()=>(await status(page)).selection).toBe("'A'.3.*");
  expect(await selectionCount(page,other)).toBe(1);
  await command(page,'undo');
  await expect.poll(async()=>(await status(page)).selection).toBe("'A'.2.*");
  await page.screenshot({path:testInfo.outputPath('sequence-two-molecules.png')});
});

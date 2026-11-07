import {test,expect} from '@playwright/test';
const status=page=>page.evaluate(()=>window.cuemol?.status());
async function present(page){await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));}
async function click(page,key){
  let previous='',unchanged=0;
  await expect.poll(async()=>{
    const rect=(await status(page))?.ui.rects[key];
    const next=JSON.stringify(rect);
    unchanged=rect?.length===4&&next===previous?unchanged+1:0;
    previous=next;return unchanged;
  },{intervals:[50]}).toBeGreaterThanOrEqual(3);
  const [x,y,w,h]=(await status(page)).ui.rects[key];
  await page.mouse.click(x+w/2,y+h/2);await present(page);
}
async function reveal(page,key){
  for(let attempt=0;attempt<8;attempt++){
    const rects=(await status(page)).ui.rects;
    const [x,y,w,h]=rects['Settings window'];
    const item=rects[key];
    if(item && item[1]>y+30 && item[1]+item[3]<y+h-8)return;
    await page.mouse.move(x+w-25,y+h/2);
    await page.mouse.wheel(0,item?.[1]<y+30?-180:180);await present(page);
  }
  throw new Error(`Setting not visible: ${key}`);
}
async function toggle(page,id){
  const key=`Plugin ${id}`;
  const value=()=>page.evaluate(id=>localStorage.getItem('cuemol.config')?.split('\n').find(line=>line.startsWith(`plugins:${id}:enabled\t`)),id);
  const before=await value();
  await reveal(page,key);await click(page,key);
  await expect.poll(value).not.toBe(before);
}
async function openSettings(page){await click(page,'Edit menu');await click(page,'Settings');await click(page,'Settings Installed');}
async function closeSettings(page){const [x,y,w]=(await status(page)).ui.rects['Settings window'];await page.mouse.click(x+w-14,y+13);await present(page);}
for(const backend of ['webgl','webgpu'])test(`Plugin settings ${backend} preserve state and restore supported contributions`,async({page},testInfo)=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  for(const label of ['MD Trajectory','AI Agent','Component Catalog'])expect((await status(page)).ui.rects[label]).toBeUndefined();
  expect((await status(page)).ui.rects.Console).toHaveLength(4);
  const scene=(await status(page)).scene.sceneId;
  await openSettings(page);
  expect((await status(page)).ui.rects['Plugin sequence']).toBeUndefined();
  expect((await status(page)).ui.rects['Plugin getpdb']).toBeUndefined();
  await toggle(page,'mdtools');await toggle(page,'catalog');await toggle(page,'pymol-console');
  await closeSettings(page);
  await click(page,'MD Trajectory');
  expect((await status(page)).ui.bottom).toBe('MD Trajectory');
  await click(page,'Component Catalog');
  await expect.poll(async()=>(await status(page)).ui.activity).toBe('catalog');
  await page.screenshot({path:testInfo.outputPath('catalog.png')});
  await click(page,'Console');
  await expect.poll(async()=>(await status(page)).ui.rects['Console PyMOL']).toBeUndefined();
  expect((await status(page)).ui.rects['Console ChimeraX']).toHaveLength(4);
  await click(page,'MD Trajectory');
  await openSettings(page);
  await toggle(page,'mdtools');await toggle(page,'catalog');await toggle(page,'console');
  await closeSettings(page);
  await expect.poll(async()=>(await status(page)).ui.bottom).toBe('Output');
  await expect.poll(async()=>(await status(page)).ui.activity).toBe('explorer');
  expect((await status(page)).scene.sceneId).toBe(scene);
  await page.reload();
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  for(const label of ['MD Trajectory','AI Agent','Component Catalog','Console'])expect((await status(page)).ui.rects[label]).toBeUndefined();
  await openSettings(page);await toggle(page,'console');await closeSettings(page);
  await click(page,'Console');
  expect((await status(page)).ui.rects['Console PyMOL']).toBeUndefined();
  expect((await status(page)).ui.rects['Console ChimeraX']).toHaveLength(4);
  const saved=await page.evaluate(()=>localStorage.getItem('cuemol.config'));
  expect(saved).toContain('plugins:console:enabled\ttrue');
  expect(saved).toContain('plugins:pymol-console:enabled\tfalse');
  expect(errors).toEqual([]);
});


for(const backend of ['webgl','webgpu'])test(`Settings categories ${backend} share theme controls and search across groups`,async({page},testInfo)=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  const theme=()=>page.evaluate(()=>localStorage.getItem('cuemol.config')?.split('\n').find(line=>line.startsWith('layout:theme\t')));
  await click(page,'Edit menu');await click(page,'Settings');await click(page,'Settings Theme');
  await click(page,'Dark Mode');await expect.poll(theme).toBe('layout:theme\tlight');
  await closeSettings(page);await page.reload();
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  await click(page,'Edit menu');await click(page,'Settings');await click(page,'Settings Theme');
  await click(page,'Dark Mode');await closeSettings(page);
  await expect.poll(theme).toBe('layout:theme\tdark');
  await page.keyboard.press('Escape');
  await click(page,'Edit menu');await click(page,'Settings');await click(page,'Settings Theme');
  await click(page,'Dark Mode');await expect.poll(theme).toBe('layout:theme\tlight');
  await click(page,'Search settings');await page.keyboard.type('pick');await present(page);
  await expect.poll(async()=>!!(await status(page)).ui.rects['GPU picking']).toBe(true);
  expect((await status(page)).ui.rects['Dark Mode']).toBeUndefined();
  const [x,y,w,h]=(await status(page)).ui.rects['Settings window'];
  expect(x+w).toBeLessThanOrEqual(page.viewportSize().width+1);
  expect(y+h).toBeLessThanOrEqual(page.viewportSize().height+1);
  await page.screenshot({path:testInfo.outputPath('settings-search.png')});
  expect(errors).toEqual([]);
});

for(const backend of ['webgl','webgpu'])test(`Component Catalog ${backend} has three independent sample panes`,async({page},testInfo)=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>localStorage.setItem('cuemol.config','plugins:catalog:enabled\ttrue\n'));
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(async()=>(await status(page))?.scene.sceneId,{timeout:60000}).toBeGreaterThan(0);
  const before=(await status(page)).scene;
  await click(page,'Component Catalog');
  for(const name of ['Catalog 1','Catalog 2','Catalog 3'])expect((await status(page)).ui.rects[name]).toHaveLength(4);
  const revealSample=async(pane,key)=>{
    for(let attempt=0;attempt<20;attempt++){
      const rects=(await status(page)).ui.rects;
      if(rects[key]?.[3]>12)return;
      const [x,y,w,h]=rects[`Catalog ${pane} content`];
      await page.mouse.move(x+w/2,y+h/2);await page.mouse.wheel(0,120);await present(page);
    }
    throw new Error(`Catalog field not visible: ${key}`);
  };
  await revealSample(3,'Catalog property Visible');await click(page,'Catalog property Visible');await click(page,'Catalog reset Visible');
  await revealSample(1,'Catalog gate');await click(page,'Catalog gate');
  await click(page,'Catalog gate');
  await revealSample(2,'Catalog list Water');await click(page,'Catalog list Water');
  await page.screenshot({path:testInfo.outputPath('catalog.png')});
  expect((await status(page)).scene).toEqual(before);
  await click(page,'Explorer');await click(page,'Component Catalog');
  for(const name of ['Catalog 1','Catalog 2','Catalog 3'])expect((await status(page)).ui.rects[name]).toHaveLength(4);
  expect(errors).toEqual([]);
});

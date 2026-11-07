import {test, expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';
import {PNG} from 'pngjs';

const root = new URL('../../crates/cuemol-desktop/tauri-ui/',import.meta.url);
const catalog = JSON.parse(await readFile(new URL('../../crates/cuemol-command/src/render_editor_catalog.json',import.meta.url),'utf8'));
const picture = new PNG({width:3000,height:2500});
for(let i=0;i<picture.data.length;i+=4){picture.data[i]=30;picture.data[i+1]=100;picture.data[i+2]=150;picture.data[i+3]=160;}
const image = `data:image/png;base64,${PNG.sync.write(picture).toString('base64')}`;

async function open(page) {
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.route('http://rendering.test/**',async route=>{
    const path=new URL(route.request().url()).pathname.slice(1);
    if(!path){await route.fulfill({contentType:'text/html',body:'<html><head><meta charset="utf-8"><link rel="stylesheet" href="styles.css"><link rel="stylesheet" href="rendering.css"><script src="rendering.js"></script></head><body></body></html>'});return;}
    await route.fulfill({contentType:path.endsWith('.js')?'text/javascript':'text/css',body:await readFile(new URL(path,root))});
  });
  await page.addInitScript(({catalog,image})=>{
    const values={backend:'umbreon',width:1200,height:1200,unit:'px',dpi:600,transparentBg:false};
    for(const [prefix,rows]of Object.entries(catalog))for(const row of rows){const key=prefix==='common'?row.key:`${prefix}.${row.key}`;if(key in values)continue;values[key]=row.type==='boolean'?false:row.type==='color'?'#000000':row.type==='enum'?(row.enumValues||row.options)[0]:row.min??1;}
    values['umbreon_npr.hatchStyle']='pencil';values['umbreon_npr.hatchLayersSpec']='';values['umbreon_npr.hatchToneSpec']='';
    const layer='layer:kind=line,spacing=10,angle=45,width=1,dotscale=1,tonehi=0.95,tonelo=0.55,fade=16,opacity=1,inkscale=1,soft=0.5,jitter=0,tooth=0,toothscale=3,seed=0,subdiv=2,wobble=0,wobwave=40,wjitter=0,slen=0,sgap=0,taper=0.3,anglejitter=0,lenjitter=0,shape=2,aspect=1,dotangle=0,invert=true';
    const tone='tone:diffuse=1,ambient=0.12,wrap=0,rim=0,rimpow=1,rimbias=0.6,contact=1,shape=0.6,black=0,white=1,hl=1,hlsoft=0.06,gamma=1,speccut=0\nink:inkshade=1,mincontrast=0.25,mode=ink,tonefog=true,levels=0,albedoquant=0';
    const state={values,sceneId:1,viewId:11,targets:[{sceneId:1,viewId:11,label:'First / Main'},{sceneId:2,viewId:22,label:'Second / Main'}],pixels:[1200,1200],lighting:'gi',quality:{},historyLength:0,selected:null,current:null,running:false,message:'No render result',catalog,hatchSpec:`${layer}\n${tone}`,defaultLayer:layer};
    for(const axis of ['aa','ao','giLighting','shadows'])state.quality[axis]={selected:'medium',steps:[{id:'low',label:'Low'},{id:'medium',label:'Medium'},{id:'high',label:'High'}]};
    const records=[];let next=1;
    window.renderCalls=[];window.savedPaths=[];window.confirmClear=false;
    window.chooseOutputPath=async()=>{window.savedPaths.push(state.current.id);return 'image.png';};
    window.showConfirmationDialog=async()=>window.confirmClear;
    window.invoke=async(command,{operation,args})=>{
      if(command!=='rendering_request')throw new Error(command);
      window.renderCalls.push({operation,args:structuredClone(args)});
      await new Promise(resolve=>setTimeout(resolve,operation==='target'?80:15));
      if(operation==='open'&&args.backend)values.backend=args.backend;
      if(operation==='edit'){values[args.key]=args.value;state.pixels=[values.width,values.height];}
      if(operation==='lighting')state.lighting=args.value;
      if(operation==='quality')state.quality[args.axis].selected=args.step;
      if(operation==='target'){state.viewId=args.viewId;state.sceneId=args.viewId===11?1:2;}
      if(operation==='hatch'){values['umbreon_npr.hatchLayersSpec']=args.layersSpec;values['umbreon_npr.hatchToneSpec']=args.toneSpec;state.hatchSpec=`${args.layersSpec||layer}\n${args.toneSpec||tone}`;}
      if(operation==='start'){state.running=true;state.message='Rendering image…';}
      if(operation==='stop'){state.running=false;state.message='Render cancelled';}
      if(operation==='save')state.message='Image saved';
      if(operation==='copy')state.message='Image copied to clipboard';
      if(operation==='useSettings'){Object.assign(values,state.current.source.settings);state.message='Applied settings from the selected image';}
      if(operation==='select'){state.selected=args.index;state.current=records[args.index];}
      if(operation==='clear'){records.length=0;state.historyLength=0;state.selected=null;state.current=null;}
      state.povError=values.backend==='povray'?'POV-Ray executable is unavailable':null;
      const result=structuredClone(state);
      if(result.current?.id!==args.imageId)result.image=result.current?image:null;
      return result;
    };
    window.finishRender=()=>{state.running=false;state.message='Rendered 3000 × 2500';const record={id:next++,size:[3000,2500],source:{scene_name:`Scene ${state.sceneId}`,settings:structuredClone(values)},elapsed_seconds:0.1};records.push(record);state.historyLength=records.length;state.selected=records.length-1;state.current=record;};
  },{catalog,image});
  await page.goto('http://rendering.test/');
  await page.evaluate(()=>CueMolRendering.open());
  return errors;
}

test('Tauri Rendering retains image history, framing and save-copy settings operations',async({page})=>{
  const errors=await open(page);const panel=page.getByRole('dialog',{name:'Rendering',exact:true});
  await panel.getByRole('button',{name:'Start Render',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Stop',exact:true})).toBeVisible();
  await expect(panel.getByLabel('Size preset',{exact:true})).toBeDisabled();
  await expect(panel.getByRole('button',{name:'Stop',exact:true})).toBeVisible();
  await page.evaluate(()=>finishRender());
  await expect(panel.getByText('1 / 1 · 3000 × 2500 · Scene 1',{exact:true})).toBeVisible();
  await expect(panel.getByRole('img',{name:'Rendered image',exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Fit',exact:true}).click();
  const imageNode=panel.getByRole('img',{name:'Rendered image',exact:true});
  const width=await imageNode.evaluate(node=>node.getBoundingClientRect().width);
  expect(width).toBeLessThan(700);
  await panel.getByRole('button',{name:'+',exact:true}).click();
  expect(await imageNode.evaluate(node=>node.getBoundingClientRect().width)).toBeGreaterThan(width);
  await panel.getByRole('button',{name:'100%',exact:true}).click();
  expect(await imageNode.evaluate(node=>node.getBoundingClientRect().width)).toBe(3000);
  const preview=panel.getByLabel('Rendered image preview',{exact:true});
  const box=await preview.boundingBox();
  const left=await imageNode.evaluate(node=>parseFloat(node.style.left));
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+50,box.y+box.height/2+20);await page.mouse.up();
  expect(await imageNode.evaluate(node=>parseFloat(node.style.left))).toBeGreaterThan(left);
  await preview.hover();
  await page.mouse.wheel(0,100);
  expect(await imageNode.evaluate(node=>node.getBoundingClientRect().width)).toBe(3000);
  await page.keyboard.down('Control');await page.mouse.wheel(0,-100);await page.keyboard.up('Control');
  await expect.poll(()=>imageNode.evaluate(node=>node.getBoundingClientRect().width)).toBeGreaterThan(3000);
  await panel.getByRole('button',{name:'Fit',exact:true}).click();
  const client=await page.context().newCDPSession(page);
  await client.send('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:2});
  const fitWidth=await imageNode.evaluate(node=>node.getBoundingClientRect().width);
  const x=box.x+box.width/2,y=box.y+box.height/2;
  await client.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:x-30,y,id:1},{x:x+30,y,id:2}]});
  await client.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:x-60,y,id:1},{x:x+60,y,id:2}]});
  await client.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  await expect.poll(()=>imageNode.evaluate(node=>node.getBoundingClientRect().width)).toBeGreaterThan(fitWidth*1.5);
  await client.send('Emulation.setTouchEmulationEnabled',{enabled:false});
  await client.detach();
  await panel.getByRole('button',{name:'Fit',exact:true}).click();
  await panel.getByText('Settings',{exact:true}).click();
  await expect(panel.locator('.rendering-record-settings pre')).toContainText('dpi: 600');
  await panel.getByText('Settings',{exact:true}).click();
  await panel.getByRole('button',{name:'Save Image…',exact:true}).click();
  await expect(panel.getByRole('status')).toHaveText('Image saved');
  await panel.getByRole('button',{name:'Copy Image',exact:true}).click();
  await expect(panel.getByRole('status')).toHaveText('Image copied to clipboard');
  await panel.getByLabel('DPI',{exact:true}).fill('150');
  await panel.getByRole('button',{name:'Use Settings',exact:true}).click();
  await expect(panel.getByLabel('DPI',{exact:true})).toHaveValue('600');
  await panel.getByRole('button',{name:'Start Render',exact:true}).click();
  await panel.getByRole('button',{name:'Stop',exact:true}).click();
  await expect(panel.getByRole('status')).toHaveText('Render cancelled');
  await expect(imageNode).toBeVisible();
  await panel.getByRole('button',{name:'Start Render',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Stop',exact:true})).toBeVisible();
  await page.evaluate(()=>finishRender());
  await expect(panel.getByText('2 / 2 · 3000 × 2500 · Scene 1',{exact:true})).toBeVisible();
  await page.screenshot({path:test.info().outputPath('rendering-history.png')});
  await panel.getByRole('button',{name:'Previous',exact:true}).click();
  await expect(panel.getByText('1 / 2 · 3000 × 2500 · Scene 1',{exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Fit',exact:true}).focus();
  await page.keyboard.press('Control+z');
  await expect.poll(()=>page.evaluate(()=>window.renderCalls.some(call=>call.operation==='undo'))).toBe(true);
  await page.keyboard.press('Control+Shift+z');
  await expect.poll(()=>page.evaluate(()=>window.renderCalls.some(call=>call.operation==='redo'))).toBe(true);
  await panel.getByRole('button',{name:'Close Rendering',exact:true}).click();
  await expect(panel).not.toBeVisible();
  await page.evaluate(()=>CueMolRendering.open());
  await expect(panel.getByText('1 / 2 · 3000 × 2500 · Scene 1',{exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Clear History',exact:true}).click();
  await expect(imageNode).toBeVisible();
  await page.evaluate(()=>{window.confirmClear=true;});
  await panel.getByRole('button',{name:'Clear History',exact:true}).click();
  await expect(imageNode).not.toBeVisible();
  expect(errors).toEqual([]);
});

test('Tauri Rendering edits targets, backend controls and successive NPR layer values',async({page})=>{
  const errors=await open(page);const panel=page.getByRole('dialog',{name:'Rendering',exact:true});
  await panel.getByLabel('Render target',{exact:true}).selectOption('22');
  await expect(panel.getByLabel('Width',{exact:true})).toBeDisabled();
  await panel.getByLabel('DPI',{exact:true}).fill('150');
  await expect.poll(()=>page.evaluate(()=>window.renderCalls.filter(call=>call.operation==='edit').at(-1)?.args.sceneId)).toBe(2);
  await panel.getByLabel('Render backend',{exact:true}).selectOption('povray');
  await expect(panel.getByRole('button',{name:'Start Render',exact:true})).toBeDisabled();
  await expect(panel.getByRole('status')).toContainText('POV-Ray executable is unavailable');
  await panel.getByLabel('Pixel labels',{exact:true}).check();
  await panel.getByRole('tab',{name:'Render',exact:true}).click();
  await expect(panel.getByLabel('Stereo mode',{exact:true})).toBeVisible();
  await panel.getByLabel('Render backend',{exact:true}).selectOption('umbreon_npr');
  await expect(panel.getByRole('button',{name:'Start Render',exact:true})).toBeEnabled();
  await panel.getByRole('tab',{name:'NPR Detail',exact:true}).click();
  await panel.getByLabel('Spacing',{exact:true}).fill('13');
  await panel.getByLabel('Angle',{exact:true}).fill('37');
  await expect.poll(()=>page.evaluate(()=>window.renderCalls.filter(call=>call.operation==='hatch').at(-1)?.args.layersSpec)).toContain('spacing=13,angle=37');
  await panel.getByRole('button',{name:'Add dot',exact:true}).click();
  await panel.getByText('Layer 2 · dot',{exact:true}).click();
  await expect(panel.getByLabel('Merge dark dots',{exact:true})).toBeVisible();
  await panel.getByLabel('Dot scale',{exact:true}).fill('2.5');
  await panel.getByText('Advanced shading',{exact:true}).click();
  await panel.getByLabel('Gamma',{exact:true}).fill('1.4');
  await expect.poll(()=>page.evaluate(()=>window.renderCalls.filter(call=>call.operation==='hatch').at(-1)?.args.toneSpec)).toContain('gamma=1.4');
  await panel.getByRole('button',{name:'Close Rendering',exact:true}).click();
  await page.evaluate(()=>CueMolRendering.open());
  await expect(panel.getByLabel('Gamma',{exact:true})).toHaveValue('1.4');
  await page.screenshot({path:test.info().outputPath('rendering-npr.png')});
  expect(errors).toEqual([]);
});

test('Tauri Rendering remains operable in a small window',async({page})=>{
  await page.setViewportSize({width:500,height:640});
  const errors=await open(page);const panel=page.getByRole('dialog',{name:'Rendering',exact:true});
  await panel.getByLabel('DPI',{exact:true}).fill('72');
  await panel.getByLabel('Transparent background',{exact:true}).check();
  const bounds=await panel.boundingBox();expect(bounds.x).toBeGreaterThanOrEqual(0);expect(bounds.x+bounds.width).toBeLessThanOrEqual(500);expect(bounds.height).toBeLessThanOrEqual(640);
  await panel.getByRole('button',{name:'Start Render',exact:true}).click();
  await expect(panel.getByRole('button',{name:'Stop',exact:true})).toBeVisible();
  await page.evaluate(()=>finishRender());
  await expect(panel.getByRole('img',{name:'Rendered image',exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Fit',exact:true}).click();
  await page.screenshot({path:test.info().outputPath('rendering-small.png')});
  expect(errors).toEqual([]);
});

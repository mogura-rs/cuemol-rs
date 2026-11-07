import {acceptFileOptions} from './support.js';
import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';

const pdb = Array.from({length:6},(_,residue)=>[
  [' N  ','N',-1.3,0],[' CA ','C',0,0],[' C  ','C',1.2,0],[' O  ','O',1.2,1.2],
].map(([name,element,dx,dy],atom)=>{
  const serial=residue*4+atom+1;
  const x=(residue*3.8-9.5+dx).toFixed(3).padStart(8);
  const y=(Math.sin(residue*0.5)*2+dy).toFixed(3).padStart(8);
  return `ATOM  ${String(serial).padStart(5)} ${name} ALA A${String(residue+1).padStart(4)}    ${x}${y}   0.000  1.00 10.00           ${element}`;
}).join('\n')).join('\n')+'\nEND\n';
const status = page => page.evaluate(() => window.cuemol.status());
async function command(page,name,args={}) {
  const id = await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
  await expect.poll(async()=>{const state=await status(page);return state.lastCommand===id&&!state.busy;}).toBe(true);
  const message=(await status(page)).message;
  expect(message).not.toMatch(/^Error:/);
  try { return JSON.parse(message); } catch { return message; }
}
for (const backend of ['webgl','webgpu']) {
  test(`Current CueMol3 palette and ribbon defaults render in ${backend}`,async({page},testInfo)=>{
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
    for(const name of ['First','Second']) {
      await page.locator('#files').setInputFiles({name:`${name}.pdb`,mimeType:'application/octet-stream',buffer:Buffer.from(pdb)});
  await acceptFileOptions(page, {fixtureNames:true});
      await expect.poll(async()=>(await status(page)).scene.objects.length).toBe(name==='First'?1:2);
    }
    const [first,second]=(await status(page)).scene.objects;
    const ribbon=first.renderers.find(renderer=>renderer.type==='ribbon');
    expect(ribbon).toBeDefined();
    const nucl=await command(page,'create_renderer',{objId:first.id,rendererType:'nucl',name:'Nucleotide defaults'});
    const result=await command(page,'execute_script',{source:`
      var first=cuemol.getObject(${first.id});var second=cuemol.getObject(${second.id});
      var ribbon=cuemol.getRenderer(${ribbon.id});var nucl=cuemol.getRenderer(${nucl.rendererId});
      print(JSON.stringify({colors:[first.coloring.getColorAt(0).toString(),second.coloring.getColorAt(0).toString()],ribbon:[ribbon.helix.width,ribbon.helix.tuber,ribbon.coil.width],nucl:[nucl.base_size,nucl.base_thick,nucl.section.width,nucl.section.type]}));
    `});
    expect(JSON.parse(result.log.trim())).toEqual({colors:['okabeito_1','okabeito_2'],ribbon:[0.18,6.5,0.2],nucl:[0.3,0.3,0.4,'roundsquare']});
    await command(page,'set_visible',{nodeType:'object',nodeId:second.id,visible:false});
    await command(page,'set_view',{center:[0,0,0],zoom:26,slab:50});
    const [x,y,w,h]=(await status(page)).ui.rects.Viewport;
    const clip={x:Math.ceil(x+80),y:Math.ceil(y+50),width:Math.floor(w)-160,height:Math.floor(h)-100};
    const pixels=PNG.sync.read(await page.screenshot({clip,path:testInfo.outputPath('current-ribbon.png')})).data;
    expect(Array.from({length:pixels.length/4},(_,i)=>pixels[i*4]>100&&pixels[i*4+1]>60&&pixels[i*4]-pixels[i*4+2]>30).filter(Boolean).length).toBeGreaterThan(100);
    const download=page.waitForEvent('download');
    await command(page,'save_scene',{path:'palette.qsc',embedAll:true});
    const chunks=[];for await(const part of await (await download).createReadStream())chunks.push(part);
    await page.reload();
    await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
    await page.locator('#files').setInputFiles({name:'palette.qsc',mimeType:'application/octet-stream',buffer:Buffer.concat(chunks)});
  await acceptFileOptions(page, {fixtureNames:true});
    await expect.poll(async()=>(await status(page)).message).toBe('Loaded palette.qsc');
    expect((await status(page)).scene.objects).toHaveLength(2);
    const reloaded=(await status(page)).scene.objects;
    const check=await command(page,'execute_script',{source:`print([cuemol.getObject(${reloaded[0].id}).coloring.getColorAt(0).toString(),cuemol.getObject(${reloaded[1].id}).coloring.getColorAt(0).toString()].join(','));`});
    expect(check.log.trim()).toBe('okabeito_1,okabeito_2');
  });
}

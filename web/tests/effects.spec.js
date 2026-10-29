import {test, expect} from '@playwright/test';
import {PNG} from 'pngjs';

const pdb=`HETATM    1  C1  LIG A   1      -1.500   0.000   0.000  1.00 10.00           C
HETATM    2  O1  LIG A   1       1.300   0.000   0.000  1.00 10.00           O
HETATM    3  N1  LIG A   1       0.000   2.000  -0.300  1.00 10.00           N
END
`;

for (const backend of ['webgpu', 'webgl']) {
  test(`${backend} applies scene AO and AA settings to live pixels`, async ({page}, testInfo) => {
    test.setTimeout(210000);
    const errors=[];
    page.on('pageerror', error=>errors.push(error.message));
    page.on('console', message=>{if(message.type()==='error')errors.push(message.text());});
    await page.goto(`./?renderer=${backend}`, {waitUntil:'commit'});
    try {
      await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId||0), {timeout:150000}).toBeGreaterThan(0);
    } finally {
      const startup=await page.evaluate(()=>({
        phase:document.querySelector('#loading')?.textContent,
        marks:performance.getEntriesByType('mark').map(({name,startTime})=>({name,startTime})),
      }));
      console.info(`${backend} startup: ${JSON.stringify(startup)}`);
      await testInfo.attach('startup.json',{body:JSON.stringify({...startup,errors}),contentType:'application/json'});
    }
    expect(await page.evaluate(()=>window.cuemol.backend)).toBe(backend);
    await page.locator('#files').setInputFiles({name:'contacts.pdb', mimeType:'application/octet-stream', buffer:Buffer.from(pdb)});
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
    const command=async(name,args)=>{
      const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
      await expect.poll(()=>page.evaluate(id=>{
        const state=window.cuemol.status();return state.lastCommand===id&&!state.busy;
      },id),{timeout:30000}).toBe(true);
      return page.evaluate(()=>window.cuemol.status());
    };
    let state=await page.evaluate(()=>window.cuemol.status());
    const object=state.scene.objects[0];
    for(const renderer of object.renderers)await command('delete_node',{nodeId:renderer.id,nodeType:'renderer'});
    await command('create_renderer',{objId:object.id, rendererType:'cpk'});
    await command('set_projection',{perspective:false});
    await command('set_view',{center:[0,0.5,0],zoom:12,distance:40,slab:40});
    const property=(key,value)=>command('set_prop',{path:key,value:String(value)});
    const rect=(await page.evaluate(()=>window.cuemol.status())).ui.rects.Viewport;
    await page.mouse.move(10,10);
    const capture=async()=>PNG.sync.read(await page.screenshot({clip:{x:rect[0],y:rect[1],width:rect[2],height:rect[3]}})).data;
    const difference=(a,b)=>{
      let count=0;
      for(let i=0;i<a.length;i+=4)if(Math.max(Math.abs(a[i]-b[i]),Math.abs(a[i+1]-b[i+1]),Math.abs(a[i+2]-b[i+2]))>2)count++;
      return count;
    };
    await property('aa_method','none');
    const plain=await capture();
    state=await command('list_node_props',{nodeType:'scene'});
    expect(JSON.parse(state.message).properties.some(p=>p.key==='aaSmaaThreshold'&&p.value==='0.05')).toBe(true);
    await property('aoEnabled',true);
    let ao;
    await expect.poll(async()=>{
      ao=await capture();let dark=0;
      for(let i=0;i<ao.length;i+=4)if(plain[i]+plain[i+1]+plain[i+2]>ao[i]+ao[i+1]+ao[i+2]+15)dark++;
      return dark;
    }).toBeGreaterThan(100);
    for(const method of ['fxaa','smaa']){
      await property('aa_method',method);
      await expect.poll(async()=>difference(ao,await capture())).toBeGreaterThan(20);
    }
    await property('aa_method','none');
    await property('aoHalfRes',true);
    await expect.poll(async()=>difference(ao,await capture()),{timeout:20000}).toBe(0);
    await property('aaJitterLevel',2);
    let converged;
    await expect.poll(async()=>{
      const first=await capture();await page.waitForTimeout(150);converged=await capture();
      return difference(first,converged);
    },{timeout:20000}).toBe(0);
    expect(difference(ao,converged)).toBeGreaterThan(20);
    await command('set_view',{zoom:14});
    await expect.poll(async()=>difference(converged,await capture())).toBeGreaterThan(500);
    await command('set_view',{zoom:12});
    await expect.poll(async()=>difference(converged,await capture()),{timeout:20000}).toBe(0);
    await page.screenshot({path:testInfo.outputPath(`${backend}-effects.png`)});
    expect(errors).toEqual([]);
  });
}

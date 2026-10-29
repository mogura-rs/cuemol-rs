import {test,expect} from '@playwright/test';
import {PNG} from 'pngjs';
import {browserLaunchArgs} from './browser-options.js';
import {load,call} from './support.js';
test.use({deviceScaleFactor:2,launchOptions:{args:[...browserLaunchArgs,'--force-device-scale-factor=2']}});
for (const backend of ['webgl','webgpu']) {
  test.describe(`center mark ${backend}`,()=>{
    test('inverts displayed colors at HiDPI and in both stereo eyes',async({page},testInfo)=>{
      const errors=[];page.on('pageerror',error=>errors.push(error.message));
      await page.goto(`./?renderer=${backend}`);
      await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
      await load(page,'center.qsc',Buffer.from('<scene bgcolor="#3366b3"><camera name="__current" center="(0,0,0)" zoom="12" slab="50" distance="100" centerMark="none"/></scene>'));
      await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message)).toBe('Loaded center.qsc');
      await page.mouse.move(0,0);
      const rect=await page.evaluate(()=>window.cuemol.status().ui.rects.Viewport);
      const capture=async()=>PNG.sync.read(await page.screenshot({clip:{x:rect[0],y:rect[1],width:rect[2],height:rect[3]}}));
      let baseline;
      await expect.poll(async()=>{
        baseline=await capture();
        const offset=4*(Math.floor(baseline.height/2)*baseline.width+Math.floor(baseline.width/2));
        return Array.from(baseline.data.subarray(offset,offset+3)).every(value=>value>10&&value<240);
      },{timeout:30000,message:'the HiDPI viewport must present the scene background'}).toBe(true);
      await testInfo.attach('viewport-baseline',{body:PNG.sync.write(baseline),contentType:'image/png'});
      for(const stereo of ['none','para']) {
        await call(page,'set_node_prop',{nodeType:'view',prop:'stereoMode',value:stereo});
        await call(page,'set_node_prop',{nodeType:'view',prop:'centerMark',value:'crosshair'});
        const centers=stereo==='none'?[baseline.width/2]:[baseline.width/4,baseline.width*3/4];
        await expect.poll(async()=>{
          const image=await capture();
          return centers.map(center=>{
            let count=0,minX=Infinity,maxX=-Infinity;
            for(let y=Math.floor(image.height/2)-25;y<image.height/2+25;y++) {
              for(let x=Math.floor(center)-25;x<center+25;x++) {
                const offset=4*(y*image.width+x);
                const before=baseline.data.subarray(offset,offset+3),after=image.data.subarray(offset,offset+3);
                if(Array.from(before).every((value,i)=>Math.abs(value+after[i]-255)<=3)) {
                  count++;minX=Math.min(minX,x);maxX=Math.max(maxX,x);
                }
              }
            }
            return count>=80&&maxX-minX>=36&&maxX-minX<=42;
          }).every(Boolean);
        },{timeout:30000,message:'both eyes must present the inverted center mark'}).toBe(true);
        await page.screenshot({path:testInfo.outputPath(`center-${stereo}.png`)});
        await call(page,'set_node_prop',{nodeType:'view',prop:'centerMark',value:'none'});
        await expect.poll(async()=>{
          const image=await capture();
          return centers.every(center=>{
            for(let y=Math.floor(image.height/2)-25;y<image.height/2+25;y++) {
              for(let x=Math.floor(center)-25;x<center+25;x++) {
                const offset=4*(y*image.width+x);
                if(!image.data.subarray(offset,offset+4).equals(baseline.data.subarray(offset,offset+4)))return false;
              }
            }
            return true;
          });
        },{timeout:30000,message:'disabling the center mark must restore the background'}).toBe(true);
      }
      expect(errors).toEqual([]);
    });
  });
}

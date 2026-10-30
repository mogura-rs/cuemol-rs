import {chromium} from '@playwright/test';
import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {join, basename} from 'node:path';
import {parseArgs} from 'node:util';
import {PNG} from 'pngjs';
import {browserLaunchArgs} from '../tests/browser-options.js';

const {values} = parseArgs({options: {
  url: {type:'string'}, fixture: {type:'string'}, output: {type:'string'},
  channel: {type:'string'}, sizes: {type:'string',default:'640x480,1200x1200'},
  samples: {type:'string',default:'3'}, profile: {type:'boolean',default:false},
}});
if (!values.url || !values.fixture || !values.output) throw new Error('--url, --fixture and --output are required');
const count = Number(values.samples);
const sizes = values.sizes.split(',').map(value => value.split('x').map(Number));
if (!Number.isInteger(count) || count < 1 || !sizes.every(size => size.length === 2 && size.every(n => Number.isInteger(n) && n > 0 && n <= 8192) && size[0] * size[1] <= 32_000_000)) {
  throw new Error('Use a positive sample count and dimensions up to 8192 and 32 megapixels');
}
await mkdir(values.output,{recursive:true});
const input = await readFile(values.fixture);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const browser = await chromium.launch({channel:values.channel,args:browserLaunchArgs});
const report = {url:values.url,browser:browser.version(),fixture:basename(values.fixture),fixtureHash:hash(input),samples:[]};
try {
  const page = await browser.newPage({viewport:{width:1280,height:900},acceptDownloads:true});
  const errors = [];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto(values.url);
  await page.waitForFunction(()=>window.cuemol?.status()?.scene?.sceneId>0,{},{timeout:60000});
  report.manifest = await page.evaluate(async()=>{
    const response=await fetch('./build-info.json');return response.ok?response.json():null;
  });
  await page.locator('#files').setInputFiles({name:basename(values.fixture),mimeType:'application/octet-stream',buffer:input});
  await page.waitForFunction(()=>window.cuemol.status().scene.objects.length===1,{},{timeout:60000});
  async function command(name,args={}) {
    const id=await page.evaluate(([name,args])=>window.cuemol.command(name,args),[name,args]);
    await page.waitForFunction(id=>{const s=window.cuemol.status();return s.lastCommand===id&&!s.busy;},id,{timeout:600000});
    return page.evaluate(()=>window.cuemol.status());
  }
  const state=await command('set_render_settings',{settings:'backend=umbreon'});
  report.settings=JSON.parse(state.message).settings;
  report.scene=state.scene;
  const session=await page.context().newCDPSession(page);
  for(const [width,height] of sizes) {
    for(let sample=0;sample<count;sample++) {
      const profile=values.profile&&sample===0;
      if(profile){await session.send('Profiler.enable');await session.send('Profiler.setSamplingInterval',{interval:1000});await session.send('Profiler.start');}
      const memoryBefore = await page.evaluate(async()=>{
        const module=await import('./pkg/cuemol_web.js');return (await module.default()).memory.buffer.byteLength;
      });
      await page.evaluate(()=>{
        window.renderProbe={frames:[],running:true};let previous=performance.now();
        function frame(now){if(!window.renderProbe.running)return;window.renderProbe.frames.push(now-previous);previous=now;requestAnimationFrame(frame);}requestAnimationFrame(frame);
      });
      const fileName=`render-${width}x${height}-${sample}.png`;
      const pending=page.waitForEvent('download',{timeout:600000});
      const start=performance.now();
      await command('render_image',{fileName,width,height});
      const download=await pending;
      const elapsedMs=performance.now()-start;
      const frames=await page.evaluate(()=>{window.renderProbe.running=false;return window.renderProbe.frames;});
      const path=join(values.output,fileName);
      await download.saveAs(path);
      const memoryAfter = await page.evaluate(async()=>{
        const module=await import('./pkg/cuemol_web.js');return (await module.default()).memory.buffer.byteLength;
      });
      if(profile){const {profile}=await session.send('Profiler.stop');await writeFile(join(values.output,`render-${width}x${height}.cpuprofile`),JSON.stringify(profile));}
      const bytes=await readFile(path),image=PNG.sync.read(bytes);
      if(image.width!==width||image.height!==height)throw new Error('Rendered dimensions changed');
      const record={width,height,sample,elapsedMs,pngBytes:bytes.length,sha256:hash(bytes),rgbaSha256:hash(image.data),maxFrameMs:Math.max(0,...frames),over50ms:frames.filter(ms=>ms>50).length,wasmMemoryBefore:memoryBefore,wasmMemoryAfter:memoryAfter};
      report.samples.push(record);console.log(JSON.stringify(record));
      await writeFile(join(values.output,'result.json'),JSON.stringify(report,null,2)+'\n');
    }
  }
  if(errors.length)throw new Error(errors.join('\n'));
} finally {await browser.close();}

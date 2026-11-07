import {test, expect} from '@playwright/test';
import {load, call} from './support.js';
import {gzipSync} from 'node:zlib';

const atom = 'ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \n';
const pdb = `${atom}END\n`;
const file = (name, buffer) => ({name, mimeType:'application/octet-stream', buffer:Buffer.from(buffer)});
async function ready(page, backend = 'webgl') {
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
}
async function click(page, name) {
  await expect.poll(()=>page.evaluate(name=>window.cuemol.status().ui.rects[name],name)).toBeTruthy();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const rect=await page.evaluate(name=>window.cuemol.status().ui.rects[name],name);
  await page.mouse.click(rect[0]+rect[2]/2,rect[1]+rect[3]/2);
}
async function fill(page,name,value) {
  await click(page,name);
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.type(value);
}
async function options(page,name,buffer) {
  await page.locator('#file-options').setInputFiles(file(name,buffer));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['File options Open']),{timeout:60000}).toBeTruthy();
}
async function finish(page,name) {
  await click(page,'File options Open');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().message),{timeout:60000}).toBe(`Loaded ${name}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().loading)).toBe(false);
}
async function companion(page,name,buffer) {
  const chooser=page.waitForEvent('filechooser');
  await click(page,'File choose companion');
  await (await chooser).setFiles(file(name,buffer));
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
}
function mrc() {
  const bytes=Buffer.alloc(1024+4*4*4*4);
  for(const [word,value] of [[0,4],[1,4],[2,4],[3,2],[7,4],[8,4],[9,4],[16,1],[17,2],[18,3],[22,1]])bytes.writeInt32LE(value,word*4);
  for(const [word,value] of [[10,4],[11,4],[12,4],[13,90],[14,90],[15,90],[20,63],[21,31.5],[49,120],[50,30],[51,40],[54,18.5]])bytes.writeFloatLE(value,word*4);
  bytes.write('MAP ',208);
  for(let i=0;i<64;i++)bytes.writeFloatLE(i,1024+4*i);
  return bytes;
}

for(const backend of ['webgl','webgpu']) {
  test(`file options preserve cancellation, reader choices and one-step history (${backend})`,async({page},testInfo)=>{
    await ready(page,backend);
    await load(page,'original.pdb',Buffer.from(pdb));
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
    let before=await call(page,'clear_undo');
    await options(page,'cancelled.pdb',pdb);
    await click(page,'File reader options');
    await click(page,'File Ignore alternate conformations');
    await click(page,'File options Cancel');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().loading)).toBe(false);
    let state=await page.evaluate(()=>window.cuemol.status());
    expect(state.scene.objects).toEqual(before.scene.objects);
    expect(state.view).toEqual(before.view);
    const models=`MODEL        1\n${atom}ENDMDL\nMODEL        2\n${atom}ENDMDL\nEND\n`;
    await options(page,'models.pdb',models);
    await fill(page,'File object name','two models');
    await click(page,'File renderer type');
    await click(page,'File renderer ballstick');
    await click(page,'File recenter');
    await click(page,'File reader options');
    await click(page,'File Ignore multiple models');
    await page.screenshot({path:testInfo.outputPath('reader-options.png')});
    await finish(page,'models.pdb');
    state=await page.evaluate(()=>window.cuemol.status());
    expect(state.scene.objects).toHaveLength(2);
    expect(state.scene.objects[1].name).toBe('two models');
    expect(state.scene.objects[1].atoms).toBe(2);
    expect(state.scene.objects[1].renderers[0].type).toBe('ballstick');
    expect(state.view).toEqual(before.view);
    state=await call(page,'undo');
    expect(state.scene.objects).toHaveLength(1);
    state=await call(page,'redo');
    expect(state.scene.objects).toHaveLength(2);
    await options(page,'invalid.pdb',pdb);
    await click(page,'File selection enabled');
    await fill(page,'File selection','invalid (((');
    await click(page,'File options Open');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().loading)).toBe(false);
    expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toEqual(state.scene.objects);
  });
}

test('CCP4 header inspection is bounded and reader options precede map centering',async({page},testInfo)=>{
  await ready(page);
  const bytes=mrc();
  const inspected=await page.evaluate(async raw=>{
    const header=new Uint8Array(raw);
    const source=new File([header,new Uint8Array(4*1024*1024)],'large.mrc');
    const info=JSON.parse(await window.cuemolLoader.inspectFile(source));
    return {info,metrics:window.cuemolLoader.lastInspection};
  },[...bytes.subarray(0,1024)]);
  expect(inspected.info.mapHeader.counts).toEqual([4,4,4]);
  expect(inspected.metrics.inputBytes).toBe(1024);
  await options(page,'density.mrc.gz',gzipSync(bytes));
  await click(page,'File reader options');
  await click(page,'File map type');
  await click(page,'File map type em');
  await click(page,'File grid stride');
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.type('2');
  await page.keyboard.press('Enter');
  await page.screenshot({path:testInfo.outputPath('map-options.png')});
  await finish(page,'density.mrc.gz');
  let state=await page.evaluate(()=>window.cuemol.status());
  expect(state.scene.maps).toHaveLength(1);
  expect(state.view.center[0]).toBeGreaterThan(100);
  state=await call(page,'undo');
  expect(state.scene.maps).toHaveLength(0);
  state=await call(page,'redo');
  const result=await call(page,'execute_script',{source:`var m=cuemol.getObject(${state.scene.maps[0].id});print(m.map_type_resolved);`});
  expect(JSON.parse(result.message).log.trim()).toBe('em');
});

test('browser companion files load NAMD coordinates and portable MSMS surfaces',async({page})=>{
  await ready(page);
  const coor=Buffer.alloc(28);coor.writeInt32LE(1);[1.25,-2.5,3.75].forEach((x,i)=>coor.writeDoubleLE(x,4+i*8));
  await load(page,'frame.coor',coor);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['File choose companion']),{timeout:60000}).toBeTruthy();
  expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toHaveLength(0);
  const psf='PSF\n\n       1 !NTITLE\n REMARKS tiny\n       1 !NATOM\n       1 SEG      1 LIG  C1   C       0.000000       12.0110           0\n       0 !NBOND: bonds\n';
  await companion(page,'frame.psf',psf);
  await finish(page,'frame.coor');
  let state=await page.evaluate(()=>window.cuemol.status());
  expect(state.scene.objects[0].atoms).toBe(1);
  const result=await call(page,'execute_script',{source:`print(cuemol.getObject(${state.scene.objects[0].id}).getAtomByID(1).pos.x);`});
  expect(Number(JSON.parse(result.message).log.trim())).toBe(1.25);
  await load(page,'triangle.face',Buffer.from('# MSMS\n# faces\n1\n1 2 3\n'));
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['File choose companion'])).toBeTruthy();
  await companion(page,'triangle.vert','# MSMS\n# vertices\n3\n0 0 0 0 0 1\n1 0 0 0 0 1\n0 1 0 0 0 1\n');
  await finish(page,'triangle.face');
  const inspected=await call(page,'execute_script',{source:"var s=scene.getObjectByName('triangle');print(s.name);"});
  expect(JSON.parse(inspected.message).log.trim()).toBe('triangle');
});

test('MTZ options read header records without the reflection body',async({page})=>{
  await ready(page);
  const result=await page.evaluate(async()=>{
    const offset=1024*1024;
    const prefix=new Uint8Array(offset);prefix.set(new TextEncoder().encode('MTZ '));new DataView(prefix.buffer).setInt32(4,offset/4+1,true);
    const records=['NCOL 5 1000000 0','COLUMN H H 0 1','COLUMN K H 0 1','COLUMN L H 0 1','COLUMN FWT F 0 1','COLUMN PHWT P 0 1','END'];
    const header=records.map(line=>line.padEnd(80)).join('');
    const metadata=JSON.parse(await window.cuemolLoader.inspectFile(new File([prefix,header],'density.mtz')));
    return {metadata,metrics:window.cuemolLoader.lastInspection};
  });
  expect(result.metadata.reader.options.columnF).toBe('FWT');
  expect(result.metadata.reader.options.columnPhi).toBe('PHWT');
  expect(result.metrics.inputBytes).toBe(1024+7*80);
});

test('AMBER options keep companion coordinates and cancel stale metadata requests',async({page})=>{
  await ready(page);
  const pointers=Array(31).fill(0);pointers[0]=1;pointers[11]=1;
  let topology='%VERSION VERSION_STAMP = V0001.000\n%FLAG POINTERS\n%FORMAT(10I8)\n';
  for(let start=0;start<pointers.length;start+=10)topology+=pointers.slice(start,start+10).map(x=>String(x).padStart(8)).join('')+'\n';
  topology+='%FLAG ATOM_NAME\n%FORMAT(20a4)\nC1  \n%FLAG ATOMIC_NUMBER\n%FORMAT(10I8)\n       6\n%FLAG RESIDUE_LABEL\n%FORMAT(20a4)\nLIG \n%FLAG RESIDUE_POINTER\n%FORMAT(10I8)\n       1\n';
  await options(page,'sample.prmtop.gz',gzipSync(Buffer.from(topology)));
  await companion(page,'sample.rst7','Coordinates\n1\n'+[1.25,-2.5,3.75].map(x=>x.toFixed(7).padStart(12)).join('')+'\n');
  await finish(page,'sample.prmtop.gz');
  const state=await page.evaluate(()=>window.cuemol.status());
  const read=await call(page,'execute_script',{source:`print(cuemol.getObject(${state.scene.objects[0].id}).getAtomByID(1).pos.x);`});
  expect(Number(JSON.parse(read.message).log.trim())).toBe(1.25);
  await page.evaluate(()=>window.cuemolLoader.cancel());
  let release;
  const gate=new Promise(resolve=>{release=resolve;});
  const hold=async route=>{await gate;await route.continue().catch(()=>{});};
  await page.route('**/loader-worker.js',hold);
  try {
    await page.locator('#file-options').setInputFiles(file('cancelled.pdb',pdb));
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().loading)).toBe(true);
    await call(page,'create_scene');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().loading)).toBe(false);
  } finally { release();await page.unroute('**/loader-worker.js',hold); }
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['File options Open'])).toBeUndefined();
  expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toHaveLength(0);
  await options(page,'recovered.pdb',pdb);
  await finish(page,'recovered.pdb');
});

test('compressed CIF inspection finds late reflection tags with bounded decoded storage',async({page})=>{
  await ready(page);
  const preamble='data_map\n#'+ 'header '.repeat(30000)+'\nloop_\n';
  const payload=gzipSync(Buffer.from(preamble+'_refln.pdbx_FWT\n_refln.pdbx_PHWT\n1 0\n'));
  const result=await page.evaluate(async raw=>{
    const metadata=JSON.parse(await window.cuemolLoader.inspectFile(new File([new Uint8Array(raw)],'late.cif.gz')));
    return metadata;
  }, [...payload]);
  expect(result.target).toBe('densityMap');
  expect(result.reader.kind).toBe('automatic');
});

test('compressed MTZ metadata keeps the decoded reflection body out of WASM memory',async({page})=>{
  await ready(page);
  const offset=64*1024*1024;
  const prefix=Buffer.alloc(offset);prefix.write('MTZ ');prefix.writeInt32LE(offset/4+1,4);
  const records=['NCOL 5 1000000 0','COLUMN H H 0 1','COLUMN K H 0 1','COLUMN L H 0 1','COLUMN FWT F 0 1','COLUMN PHWT P 0 1','END'];
  const bytes=gzipSync(Buffer.concat([prefix, Buffer.from(records.map(line=>line.padEnd(80)).join(''))]));
  const result=await page.evaluate(async raw=>{
    const info=JSON.parse(await window.cuemolLoader.inspectFile(new File([new Uint8Array(raw)],'large.mtz.gz')));
    return {info,metrics:window.cuemolLoader.lastInspection};
  }, [...bytes]);
  expect(result.info.reader.options.columnF).toBe('FWT');
  expect(result.metrics.inputBytes).toBe(bytes.length);
  expect(result.metrics.memoryBytes).toBeLessThan(offset);
});

test('compressed MTZ reader choices create a map with one-step history',async({page},testInfo)=>{
  await ready(page);
  const columns=[['H','H'],['K','H'],['L','H'],['FWT','F'],['PHWT','P'],['FALT','F']];
  const prefix=Buffer.alloc(80+columns.length*4);prefix.write('MTZ ');prefix.writeInt32LE(prefix.length/4+1,4);
  [1,0,0,1000,0,2000].forEach((value,index)=>prefix.writeFloatLE(value,80+index*4));
  const records=['NCOL 6 1 0','CELL 10 10 10 90 90 90',"SYMINF 1 1 P 1 'P 1'",'RESO 0.01 0.25',
    ...columns.map(([name,kind])=>`COLUMN ${name} ${kind} 0 0 0`),'END'];
  const bytes=gzipSync(Buffer.concat([prefix,Buffer.from(records.map(line=>line.padEnd(80)).join(''))]));
  await options(page,'reflection.mtz.gz',bytes);
  await click(page,'File reader options');
  await click(page,'File Amplitude');
  await click(page,'File Amplitude FALT');
  await page.screenshot({path:testInfo.outputPath('mtz-options.png')});
  await finish(page,'reflection.mtz.gz');
  let state=await call(page,'get_scene_state');
  expect(state.scene.maps).toHaveLength(1);
  state=await call(page,'undo');
  expect(state.scene.maps).toHaveLength(0);
  state=await call(page,'redo');
  expect(state.scene.maps).toHaveLength(1);
});

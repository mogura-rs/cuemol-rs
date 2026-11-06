import {test, expect} from '@playwright/test';
import {gzipSync} from 'node:zlib';
import {call} from './support.js';

const headers = {'access-control-allow-origin':'*','access-control-allow-methods':'GET,POST,OPTIONS','access-control-allow-headers':'content-type'};
const cif = `data_probe
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_atom_id
_atom_site.pdbx_PDB_model_num
HETATM 1 C C1 . LIG A 1 1 ? 0 0 0 1 10 1 LIG A C1 1
#
`;
function entry(id, em = false) {
  return {rcsb_id:id.toUpperCase(),struct:{title:`Structure ${id}`},exptl:[{method:em?'ELECTRON CRYSTALLOGRAPHY':'X-RAY DIFFRACTION'}],rcsb_entry_info:{resolution_combined:[2.1]},rcsb_entry_container_identifiers:{emdb_ids:em?['EMD-11638']:[]},pdbx_database_status:{pdb_format_compatible:em?'N':'Y'}};
}
function mapBytes() {
  const map=Buffer.alloc(1024+64*4);
  for(let axis=0;axis<3;axis++) {
    map.writeInt32LE(4,axis*4);map.writeInt32LE(4,(axis+7)*4);
    map.writeFloatLE(8,(axis+10)*4);map.writeFloatLE(90,(axis+13)*4);
    map.writeInt32LE(axis+1,(axis+16)*4);map.writeFloatLE(30,(axis+49)*4);
  }
  map.writeInt32LE(2,12);map.writeFloatLE(63,80);map.writeFloatLE(31.5,84);map.writeFloatLE(18.47,216);map.write('MAP ',208);
  for(let i=0;i<64;i++)map.writeFloatLE(i,1024+i*4);
  return gzipSync(map);
}
async function present(page) {await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));}
async function click(page,name) {
  await expect.poll(()=>page.evaluate(name=>window.cuemol.status().ui.rects[name]?.length,name)).toBe(4);
  await present(page);
  const [x,y,w,h]=await page.evaluate(name=>window.cuemol.status().ui.rects[name],name);
  await page.mouse.click(x+w/2,y+h/2);await present(page);
}
async function input(page,value) {
  await click(page,'Get PDB identifier');await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(value);await present(page);
}
async function setup(page,renderer='webgl') {
  await page.goto(`./?renderer=${renderer}`);
  await expect.poll(()=>page.evaluate(()=>window.cuemol?.status()?.scene?.sceneId),{timeout:60000}).toBeGreaterThan(0);
  await click(page,'Get PDB');
}
async function routeMap(page, requests, status=200) {
  await page.route('https://www.ebi.ac.uk/emdb/api/entry/map/EMD-11638',route=> {
    requests.push('metadata');return route.fulfill({headers,status,json:{map:{dimensions:{col:4,row:4,sec:4},size_kbytes:2,contour_list:{contour:[{level:20,primary:true}]}}}});
  });
  await page.route('https://www.ebi.ac.uk/emdb/api/entry/admin/EMD-11638',route=>route.fulfill({headers,status:503,body:''}));
  await page.route('https://files.wwpdb.org/pub/emdb/structures/EMD-11638/map/emd_11638.map.gz',route=> {
    requests.push('download');return route.fulfill({headers,contentType:'application/octet-stream',body:mapBytes()});
  });
}

for(const renderer of ['webgl','webgpu']) {
  test(`Get PDB ${renderer} searches, pages, selects and loads an EMDB map in the worker`,async({page},testInfo)=>{
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    const searches=[];const mapRequests=[];
    await page.route('https://search.rcsb.org/rcsbsearch/v2/query',route=>{
      if(route.request().method()==='OPTIONS')return route.fulfill({headers,status:204});
      const body=route.request().postDataJSON();searches.push(body);
      const start=body.request_options.paginate.start;
      return route.fulfill({headers,json:{total_count:30,result_set:(start===0?['1CRN','7A4M']:['1MBN']).map(identifier=>({identifier}))}});
    });
    await page.route('https://data.rcsb.org/graphql',route=>{
      if(route.request().method()==='OPTIONS')return route.fulfill({headers,status:204});
      const query=route.request().postDataJSON().query;
      return route.fulfill({headers,json:{data:{entries:query.includes('1MBN')?[entry('1mbn')]:[entry('7a4m',true),entry('1crn')]}}});
    });
    await routeMap(page,mapRequests);
    await setup(page,renderer);await input(page,'ribosome');await click(page,'Search PDB');
    await click(page,'More PDB results');
    await expect.poll(()=>searches.length).toBe(2);
    expect(searches.map(s=>s.request_options.paginate.start)).toEqual([0,25]);
    expect(searches[0].query.parameters.value).toBe('ribosome');
    await click(page,'PDB result 7a4m');
    await expect.poll(()=>mapRequests).toEqual(['metadata']);
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().ui.rects['Fetch EMD-11638']?.length)).toBe(4);
    await click(page,'Fetch coordinates');
    await page.screenshot({path:testInfo.outputPath(`search-selected-${renderer}.png`)});
    await click(page,'Fetch PDB');
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.maps.length)).toBe(1);
    await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(false);
    expect(mapRequests).toEqual(['metadata','download']);
    const state=await page.evaluate(()=>window.cuemol.status());
    expect(state.scene.objects.filter(object=>object.className==='MolCoord')).toHaveLength(0);
    expect(state.scene.maps[0].name).toBe('emd_11638');
    expect(state.scene.objects.find(object=>object.className==='DensityMap').renderers.map(r=>r.type)).toEqual(['isosurf']);
    const contours=JSON.parse((await call(page,'list_map_contours')).message).contours;
    expect(contours).toHaveLength(1);
    const level=JSON.parse((await call(page,'get_renderer_property',{rendId:contours[0].rendererId,prop:'level'})).message).value;
    expect(Number(level)).toBeCloseTo(20,0);
    expect(await page.evaluate(()=>window.cuemolLoader.last.inputBytes)).toBeGreaterThan(0);
    expect(JSON.parse(await page.evaluate(()=>localStorage.getItem('cuemol.getPdbDialog.history')))).toEqual(['7a4m','ribosome']);
    await call(page,'undo');
    expect((await page.evaluate(()=>window.cuemol.status())).scene.maps).toHaveLength(0);
    expect(errors).toEqual([]);
  });
}

test('Get PDB ignores stale searches, accepts direct EMDB IDs and falls back after metadata errors',async({page})=>{
  let release;
  const delayed=new Promise(resolve=>release=resolve);
  let searched=false;
  await page.route('https://search.rcsb.org/rcsbsearch/v2/query',async route=>{
    if(route.request().method()==='OPTIONS')return route.fulfill({headers,status:204});
    searched=true;await delayed;
    await route.fulfill({headers,json:{total_count:1,result_set:[{identifier:'1CRN'}]}}).catch(()=>{});
  });
  let graphql=0;
  await page.route('https://data.rcsb.org/graphql',route=>{graphql++;return route.fulfill({headers,json:{data:{entries:[entry('1crn')]}}});});
  const requests=[];await routeMap(page,requests,503);
  await setup(page);await input(page,'kinase');await click(page,'Search PDB');
  await expect.poll(()=>searched).toBe(true);
  await input(page,'EMD-11638');release();
  await expect.poll(()=>requests).toEqual(['metadata']);
  await click(page,'Fetch EMD-11638');await click(page,'Fetch EMD-11638');
  await click(page,'Fetch PDB');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.maps.length)).toBe(1);
  expect(graphql).toBe(0);
  expect((await page.evaluate(()=>window.cuemol.status())).scene.maps[0].name).toBe('emd_11638');
});

test('Get PDB downloads coordinates in the worker after a failed metadata lookup',async({page})=>{
  let known=false;
  await page.route('https://data.rcsb.org/graphql',route=>{
    if(route.request().method()==='OPTIONS')return route.fulfill({headers,status:204});
    known=true;return route.fulfill({headers,status:503,body:''});
  });
  await page.route('https://files.rcsb.org/download/1crn.cif',route=>route.fulfill({headers,body:cif}));
  await setup(page);await input(page,'1crn');
  await expect.poll(()=>known).toBe(true);
  await click(page,'Fetch PDB');
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(false);
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().scene.objects.length)).toBe(1);
  expect((await page.evaluate(()=>window.cuemol.status())).scene.objects[0].atoms).toBe(1);
});

test('Get PDB cancellation drops late metadata and downloaded files',async({page})=>{
  let releaseMetadata;
  const metadataReady=new Promise(resolve=>releaseMetadata=resolve);
  let metadataStarted=false;
  await page.route('https://data.rcsb.org/graphql',async route=>{
    if(route.request().method()==='OPTIONS')return route.fulfill({headers,status:204});
    metadataStarted=true;await metadataReady;
    await route.fulfill({headers,json:{data:{entries:[entry('1crn')]}}}).catch(()=>{});
  });
  let releaseDownload;
  const downloadReady=new Promise(resolve=>releaseDownload=resolve);
  let downloadStarted=false;
  await page.route('https://files.rcsb.org/download/1crn.cif',async route=>{
    downloadStarted=true;await downloadReady;
    await route.fulfill({headers,body:cif}).catch(()=>{});
  });
  await setup(page);await input(page,'1crn');
  await expect.poll(()=>metadataStarted).toBe(true);
  await click(page,'Cancel Get PDB');releaseMetadata();
  await click(page,'Get PDB');await input(page,'1crn');
  await click(page,'Fetch PDB');
  await expect.poll(()=>downloadStarted).toBe(true);
  await click(page,'Cancel loading');releaseDownload();
  await expect.poll(()=>page.evaluate(()=>window.cuemol.status().busy)).toBe(false);
  await present(page);await present(page);
  expect((await page.evaluate(()=>window.cuemol.status())).scene.objects).toHaveLength(0);
});

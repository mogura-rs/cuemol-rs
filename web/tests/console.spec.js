import {test, expect} from '@playwright/test';
import {load, call} from './support.js';
import {readFile} from 'node:fs/promises';
import {PNG} from 'pngjs';

const pdb = 'ATOM      1  CA  ALA A   1       1.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       2.000   0.000   0.000  1.00 10.00           C\nEND\n';
const status = page => page.evaluate(() => window.cuemol.status());
const present = page => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
async function choose(page, name) {
  await expect.poll(async () => (await status(page)).ui.rects[name]?.length).toBe(4);
  const [x, y, w, h] = (await status(page)).ui.rects[name];
  await page.mouse.click(x + w / 2, y + h / 2);
  await present(page);
}
async function input(page, text) {
  await choose(page, 'Console input');
  await page.keyboard.press('ControlOrMeta+a');
  await present(page);
  await page.keyboard.insertText(text);
  await present(page);
}
async function setup(page, renderer = 'webgl') {
  await page.goto(`./?renderer=${renderer}`);
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
  await load(page, 'probe.pdb', Buffer.from(pdb));
  await expect.poll(async () => (await status(page)).scene.objects.length).toBe(1);
  await call(page, 'rename_node', {nodeType:'object', nodeId:(await status(page)).scene.objects[0].id, name:'probe'});
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('probe');
  await choose(page, 'Console');
}

for (const renderer of ['webgl', 'webgpu']) {
  test(`ChimeraX ${renderer} console shares selections, surfaces, labels, distances and undo`, async ({page}, testInfo) => {
    await setup(page, renderer);
    await choose(page, 'Console ChimeraX');
    await input(page, 'alias mark select $1; show sel');
    for (const line of ['mark #1:2', 'style sel sphere', 'color sel blue', 'label sel residues', 'surface #1:1', 'surface hidePatches #1:1', 'surface #1:2', 'distance @CA']) {
      await page.keyboard.press('Shift+Enter');
      await present(page);
      await page.keyboard.insertText(line);
      await present(page);
    }
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await status(page)).message).toBe('Console executed');
    const current = await status(page);
    const names = current.scene.objects[0].renderers.map(r => r.name);
    expect(names).toEqual(expect.arrayContaining(['chx:sphere', 'chx:surface:A', 'chx:label:residues', 'chx:distance']));
    expect(current.ui.navigation.labels).toBe(1);
    expect(current.ui.navigation.measurementLabels).toContain('1.00');
    const download = page.waitForEvent('download');
    await input(page, 'save exported.data format pdb models #1');
    await page.keyboard.press('Enter');
    const saved = await download;
    expect(saved.suggestedFilename()).toBe('exported.data');
    expect(await readFile(await saved.path(), 'utf8')).toContain('GLY');
    await expect.poll(async () => (await status(page)).message).toBe('Console executed');
    await page.screenshot({path:testInfo.outputPath(`console-chimerax-${renderer}.png`)});
    await input(page, 'undo');
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await status(page)).scene.objects[0].renderers.some(r => r.name.startsWith('chx:'))).toBe(false);
    await input(page, 'redo');
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await status(page)).scene.objects[0].renderers.some(r => r.name === 'chx:sphere')).toBe(true);
  });
}

test('CueMol console resolves names sequentially and a submission has one undo entry', async ({page}, testInfo) => {
  await setup(page);
  await input(page, 'create_renderer probe, simple, name=wire; set probe/wire.visible, false; select probe, resid 2; rename probe, renamed');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('renamed');
  expect((await status(page)).scene.objects[0].renderers.find(renderer => renderer.name === 'wire').visible).toBe(false);
  expect((await status(page)).selection).toBe('resid 2');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  expect((await status(page)).scene.objects[0].renderers.some(renderer => renderer.name === 'wire')).toBe(false);
  await call(page, 'redo');
  await input(page, 'rename renamed, changed; missing_command; hide changed');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console error');
  expect((await status(page)).scene.objects[0].name).toBe('changed');
  expect((await status(page)).scene.objects[0].visible).toBe(true);
  await call(page, 'undo');
  await input(page, 'view zoom=37, rotation=1 0 0 0; get_scene_state');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  expect((await status(page)).view.zoom).toBe(37);
  await call(page, 'redo');
  expect((await status(page)).scene.objects[0].name).toBe('changed');
  await page.screenshot({path: testInfo.outputPath('console-native-dialect.png')});
});

test('Console completes molecule names, recalls history, and submits multiline input once', async ({page}) => {
  await setup(page);
  await input(page, 'select pro');
  await page.keyboard.press('Tab');
  await page.keyboard.insertText(', resid 1');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).selection).toBe('resid 1');
  await input(page, 'set_view zoom=12');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBe(12);
  await call(page, 'set_view', {zoom:21});
  await choose(page, 'Console input');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).view.zoom).toBe(12);
  await input(page, 'rename probe, first');
  await page.keyboard.press('Shift+Enter');
  await present(page);
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  await page.keyboard.insertText('rename first, second');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('second');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  const viewport = (await status(page)).ui.rects.Viewport;
  await input(page, 'rename probe, step0');
  for (let index = 1; index < 12; index++) {
    await page.keyboard.press('Shift+Enter');
    await present(page);
    await page.keyboard.insertText(`rename step${index - 1}, step${index}`);
    await present(page);
  }
  expect((await status(page)).ui.rects.Viewport).toEqual(viewport);
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('step11');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
});

test('Console preserves submissions and the next draft within one browser event batch', async ({page}) => {
  await setup(page);
  await input(page, 'rename probe, renamed');
  await page.evaluate(() => {
    const target = document.activeElement;
    if (!(target instanceof HTMLInputElement)) throw new Error('Console text input is not focused');
    const enter = () => {
      target.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', code:'Enter', bubbles:true}));
      target.dispatchEvent(new KeyboardEvent('keyup', {key:'Enter', code:'Enter', bubbles:true}));
    };
    const text = value => {
      target.value = value;
      target.dispatchEvent(new InputEvent('input', {data:value, inputType:'insertText', bubbles:true}));
    };
    enter();
    text('undo');
    enter();
    text('redo');
  });
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('cuemol.console.history'))))
    .toEqual(['rename probe, renamed', 'undo']);
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('probe');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('renamed');
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('cuemol.console.history'))))
    .toEqual(['rename probe, renamed', 'undo', 'redo']);
});

test('Console waits for an IME commit before running a command', async ({page, browserName}) => {
  test.skip(browserName !== 'chromium', 'Chromium exposes the browser IME protocol.');
  await setup(page);
  await input(page, 'rename probe, ');
  const session = await page.context().newCDPSession(page);
  await session.send('Input.imeSetComposition', {text:'にほん', selectionStart:3, selectionEnd:3});
  await present(page);
  await session.send('Input.dispatchKeyEvent', {type:'keyDown', key:'Enter', code:'Enter', windowsVirtualKeyCode:13});
  await session.send('Input.insertText', {text:'日本語'});
  await present(page);
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('日本語');
});

test('Console scene downloads and explicit history reset respect transaction boundaries', async ({page}) => {
  await setup(page);
  await input(page, 'rename probe, saved');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('saved');
  const download = page.waitForEvent('download');
  await input(page, 'save console-scene.qsc');
  await page.keyboard.press('Enter');
  const scene = await download;
  expect(await readFile(await scene.path(), 'utf8')).toContain('name="saved"');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('saved');
  await input(page, 'rename saved, partial; clear_undo');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console error');
  expect((await status(page)).scene.objects[0].name).toBe('partial');
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('saved');
  await input(page, 'clear_undo');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'redo');
  expect((await status(page)).scene.objects[0].name).toBe('saved');
});

test('PyMOL console updates representations, labels, measurements and selected exports', async ({page}, testInfo) => {
  await setup(page);
  await choose(page, 'Console PyMOL');
  await input(page, 'show sticks, probe; color red, probe and resi 2; label probe and name CA, "%s%s" % (resn, resi); distance d, probe and resi 1, probe and resi 2; set_name probe, renamed');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  const current = await status(page);
  expect(current.scene.objects[0].name).toBe('renamed');
  expect(current.scene.objects[0].renderers.some(renderer => renderer.name === 'pym:sticks' && renderer.visible)).toBe(true);
  expect(current.ui.navigation.labels).toBe(2);
  expect(current.ui.navigation.measurementLabels).toContain('1.00');
  let download = page.waitForEvent('download');
  await input(page, 'save selected.pdb, renamed and resi 2');
  await page.keyboard.press('Enter');
  const selected = await download;
  const output = await readFile(await selected.path(), 'utf8');
  expect(output).toContain('GLY');
  expect(output).not.toContain('ALA');
  download = page.waitForEvent('download');
  await input(page, 'png console.png, 96, 64, ray=0');
  await page.keyboard.press('Enter');
  const captured = await download;
  const image = PNG.sync.read(await readFile(await captured.path()));
  expect([image.width, image.height]).toEqual([96, 64]);
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await page.screenshot({path:testInfo.outputPath('console-pymol.png')});
  await call(page, 'undo');
  expect((await status(page)).scene.objects[0].name).toBe('probe');
  expect((await status(page)).scene.objects[0].renderers.some(renderer => renderer.name === 'pym:sticks')).toBe(false);
});

test('Console awaits downloads in order and stopping rejects late results while keeping one undo', async ({page}) => {
  await setup(page);
  await choose(page, 'Console PyMOL');
  const url = new URL('console-fixture.pdb', page.url()).href;
  await page.route(url, route => route.fulfill({body:pdb, contentType:'text/plain'}));
  await input(page, `load ${url}, fetched; set_name fetched, ready`);
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await status(page)).scene.objects.map(object => object.name)).toEqual(['probe', 'ready']);
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  await call(page, 'undo');
  expect((await status(page)).scene.objects.map(object => object.name)).toEqual(['probe']);
  await page.unroute(url);
  let arrived;
  const requested = new Promise(resolve => { arrived = resolve; });
  await page.route(url, route => { arrived(route); });
  await input(page, `set_name probe, pending; load ${url}, late; set_name pending, unwanted`);
  await page.keyboard.press('Enter');
  const pending = await requested;
  await expect.poll(async () => (await status(page)).scene.objects[0].name).toBe('pending');
  await choose(page, 'Console Stop');
  await pending.fulfill({body:pdb, contentType:'text/plain'}).catch(() => {});
  await page.waitForTimeout(300);
  expect((await status(page)).scene.objects.map(object => object.name)).toEqual(['pending']);
  await call(page, 'undo');
  expect((await status(page)).scene.objects.map(object => object.name)).toEqual(['probe']);
});

test('CueMol object exports honor selection, explicit format and source linking', async ({page}) => {
  await setup(page);
  const object = (await status(page)).scene.objects[0];
  const source = async () => {
    await call(page, 'list_node_props', {nodeType:'object',nodeId:object.id});
    return JSON.parse((await status(page)).message).properties.find(property => property.key === 'src').value;
  };
  const original = await source();
  const partial = page.waitForEvent('download');
  await input(page, 'write probe, selected.data, resid 2, pdb, true');
  await page.keyboard.press('Enter');
  const selected = await partial;
  expect(selected.suggestedFilename()).toBe('selected.data');
  const bytes = await readFile(await selected.path(), 'utf8');
  expect(bytes).toContain('GLY');
  expect(bytes).not.toContain('ALA');
  await expect.poll(async () => (await status(page)).message).toBe('Console executed');
  expect(await source()).toBe(original);
  const unlinked = page.waitForEvent('download');
  await call(page, 'save_object', {objId:object.id,fileName:'unlinked.pdb',link:false});
  await unlinked;
  expect(await source()).toBe(original);
  const whole = page.waitForEvent('download');
  await call(page, 'save_object', {objId:object.id,fileName:'whole.pdb',selection:'*'});
  const all = await whole;
  expect(await readFile(await all.path(), 'utf8')).toContain('ALA');
  expect(await source()).toBe('whole.pdb');
});

for (const renderer of ['webgl', 'webgpu']) {
  test(`Current catalog ${renderer} focus, copy and encoded scene save retain data`, async ({page}) => {
    await setup(page, renderer);
    const id = (await status(page)).scene.objects[0].id;
    await call(page, 'set_mol_selection', {objId:id,selection:'resid 2'});
    await call(page, 'focus_selection', {objId:id,zoom:false});
    expect((await status(page)).view.center).toEqual([2,0,0]);
    await call(page, 'orient_view', {objId:id,selection:'*'});
    expect((await status(page)).selection).toBe('resid 2');
    const copied = JSON.parse((await call(page, 'copy_node', {nodeId:id,nodeType:'object'})).message);
    await call(page, 'paste_node', copied);
    expect((await status(page)).scene.objects.length).toBe(2);
    const pending = page.waitForEvent('download');
    await input(page, 'save_scene current.qsc, embedAll=true, version=QDF1, compress=gzip, base64=true');
    await page.keyboard.press('Enter');
    const saved = await pending;
    const bytes = await readFile(await saved.path());
    expect(bytes.toString('utf8')).not.toContain('\uFFFD');
    await expect.poll(async () => (await status(page)).message).toBe('Console executed');
    await page.reload();
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout:60000}).toBeGreaterThan(0);
    await load(page, 'current.qsc', bytes);
    await expect.poll(async () => (await status(page)).message).toBe('Loaded current.qsc');
    expect((await status(page)).scene.objects.map(object => object.atoms)).toEqual([2,2]);
  });

  test(`Current catalog ${renderer} resolves EMDB metadata and loads its contour`, async ({page}) => {
    await setup(page, renderer);
    const headers = {'access-control-allow-origin':'*','access-control-allow-methods':'GET,POST,OPTIONS','access-control-allow-headers':'content-type'};
    const requests = [];
    await page.route('https://data.rcsb.org/graphql', async route => {
      if (route.request().method() === 'OPTIONS') return route.fulfill({headers,status:204});
      requests.push('pdb');
      expect(route.request().postDataJSON().query).toContain('7A4M');
      await route.fulfill({headers,json:{data:{entries:[{rcsb_id:'7A4M',exptl:[{method:'ELECTRON MICROSCOPY'}],rcsb_entry_container_identifiers:{emdb_ids:['EMD-11638']}}]}}});
    });
    await page.route('https://www.ebi.ac.uk/emdb/api/entry/map/EMD-11638', async route => {
      requests.push('level');
      await route.fulfill({headers,json:{map:{contour_list:{contour:[{level:20,primary:true}]}}}});
    });
    const map = Buffer.alloc(1024 + 64 * 4);
    for (let axis = 0; axis < 3; axis++) {
      map.writeInt32LE(4, axis * 4); map.writeInt32LE(4, (axis + 7) * 4);
      map.writeFloatLE(8, (axis + 10) * 4); map.writeFloatLE(90, (axis + 13) * 4);
      map.writeInt32LE(axis + 1, (axis + 16) * 4); map.writeFloatLE(30, (axis + 49) * 4);
    }
    map.writeInt32LE(2, 12); map.writeFloatLE(63, 80); map.writeFloatLE(31.5, 84);
    map.writeFloatLE(18.47, 216); map.write('MAP ', 208);
    for (let i = 0; i < 64; i++) map.writeFloatLE(i, 1024 + i * 4);
    const {gzipSync} = await import('node:zlib');
    await page.route('https://files.wwpdb.org/pub/emdb/structures/EMD-11638/map/emd_11638.map.gz', async route => {
      requests.push('map');
      await route.fulfill({headers,contentType:'application/octet-stream',body:gzipSync(map)});
    });
    await input(page, 'fetch_emdb 7a4m');
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await status(page)).message).toBe('Console executed');
    expect(requests).toEqual(['pdb','level','map']);
    expect((await status(page)).scene.maps[0].name).toBe('emd_11638');
    const contours = JSON.parse((await call(page, 'list_map_contours')).message).contours;
    expect(contours).toHaveLength(1);
    const level = JSON.parse((await call(page, 'get_renderer_property', {rendId:contours[0].rendererId,prop:'level'})).message).value;
    expect(Number(level)).toBeCloseTo(20,0);
    await page.route('https://www.ebi.ac.uk/emdb/api/entry/map/EMD-9999', route => route.fulfill({headers,status:404,body:''}));
    await input(page, 'fetch_emdb EMD-9999');
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await status(page)).message).toBe('Console error');
    expect((await status(page)).scene.maps).toHaveLength(1);
  });
}

import {test, expect} from '@playwright/test';
import {load, call} from './support.js';
import {gzipSync} from 'node:zlib';

const pdb = 'HETATM    1  C1  LIG A   1      -2.000   0.000   0.000  1.00 10.00           C  \nHETATM    2  O1  LIG A   1       0.000   0.000   0.000  1.00 10.00           O  \nEND\n';
const downloadedCif = 'data_download\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nHETATM 1 C C1 LIG A 1 0 0 0\n#\n';
async function ready(page) {
  await page.goto('./?renderer=webgl');
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
}
async function loaded(page, name) {
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message), {timeout: 60000}).toBe(`Loaded ${name}`);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
}
async function holdWorker(page) {
  await page.evaluate(() => window.cuemolLoader.cancel());
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  const handler = async route => { await gate; await route.continue().catch(() => {}); };
  await page.route('**/loader-worker.js', handler);
  return async () => { release(); await page.unroute('**/loader-worker.js', handler); };
}

async function holdRenderYield(page) {
  await page.evaluate(() => {
    const postMessage = MessagePort.prototype.postMessage;
    let resume;
    window.renderYield = {
      held: false,
      release() { MessagePort.prototype.postMessage = postMessage; resume?.(); },
    };
    MessagePort.prototype.postMessage = function(...args) {
      if (args[0] === null && !window.renderYield.held) {
        window.renderYield.held = true;
        resume = () => postMessage.apply(this, args);
      } else {
        return postMessage.apply(this, args);
      }
    };
  });
  return async () => {
    const id = await page.evaluate(() => window.cuemol.command('cancel_render'));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(id);
    await page.evaluate(() => window.renderYield.release());
  };
}

async function fetchPdb(page) {
  const button = await page.evaluate(() => window.cuemol.status().ui.rects['Get PDB']);
  await page.mouse.click(button[0] + button[2] / 2, button[1] + button[3] / 2);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Get PDB identifier'])).toBeTruthy();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const field = await page.evaluate(() => window.cuemol.status().ui.rects['Get PDB identifier']);
  await page.mouse.click(field[0] + field[2] / 2, field[1] + field[3] / 2);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.type('1CRN');
  const fetch = await page.evaluate(() => window.cuemol.status().ui.rects['Fetch PDB']);
  await page.mouse.click(fetch[0] + fetch[2] / 2, fetch[1] + fetch[3] / 2);
}

test('worker input preserves precise mmCIF coordinates and atom IDs', async ({page}) => {
  await ready(page);
  const workers = [];
  workers.push(...page.workers().map(worker => worker.url()));
  page.on('worker', worker => workers.push(worker.url()));
  const cif = 'data_probe\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 37 C CA GLY A 1 1.23456789012345 2 3\n#\n';
  await load(page, 'precise.mmcif', Buffer.from(cif));
  await loaded(page, 'precise.mmcif');
  const state = await call(page, 'execute_script', {source: "var atom=scene.getObjectByName('precise.mmcif').getAtomByID(37);print(atom.pos.x);"});
  expect(Number(JSON.parse(state.message).log.trim())).toBe(1.23456789012345);
  expect(workers.some(url => url.endsWith('/loader-worker.js'))).toBe(true);
  const metrics = await page.evaluate(() => window.cuemolLoader.last);
  expect(metrics.inputBytes).toBe(Buffer.byteLength(cif));
  expect(metrics.outputBytes).toBeGreaterThan(0);
  await call(page, 'undo');
  expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(0);
});

test('compressed input is decoded in the worker and retains its molecular name', async ({page}) => {
  await ready(page);
  const bytes = gzipSync(Buffer.from(pdb));
  await load(page, 'compressed.pdb.gz', bytes);
  await loaded(page, 'compressed.pdb.gz');
  const state = await page.evaluate(() => window.cuemol.status());
  expect(state.scene.objects[0].name).toBe('compressed.pdb');
  expect(state.scene.objects[0].atoms).toBe(2);
  expect(await page.evaluate(() => window.cuemolLoader.last.inputBytes)).toBe(bytes.length);
});

test('loading keeps navigation and edits responsive and Escape cancels pending input', async ({page}) => {
  await ready(page);
  await load(page, 'original.pdb', Buffer.from(pdb));
  await loaded(page, 'original.pdb');
  const before = await call(page, 'clear_undo');
  const release = await holdWorker(page);
  try {
    await load(page, 'cancelled.pdb', Buffer.from(pdb));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(true);
    const id = await page.evaluate(object => window.cuemol.command('set_node_prop', {nodeType: 'object', nodeId: object, prop: 'name', value: 'edited while loading'}), before.scene.objects[0].id);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(id);
    const rect = before.ui.rects.Viewport;
    await page.mouse.move(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
    await page.mouse.down();
    await page.mouse.move(rect[0] + rect[2] / 2 + 75, rect[1] + rect[3] / 2 + 35, {steps: 8});
    await page.mouse.up();
    await expect.poll(() => page.evaluate(() => window.cuemol.status().view.rotation)).not.toEqual(before.view.rotation);
    await page.keyboard.press('Escape');
    await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
    const state = await page.evaluate(() => window.cuemol.status());
    expect(state.scene.objects).toHaveLength(1);
    expect(state.scene.objects[0].name).toBe('edited while loading');
  } finally { await release(); }
  await load(page, 'recovered.pdb', Buffer.from(pdb));
  await loaded(page, 'recovered.pdb');
  expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(2);
});

test('failed worker input preserves edits and redo history', async ({page}) => {
  await ready(page);
  await load(page, 'original.pdb', Buffer.from(pdb));
  await loaded(page, 'original.pdb');
  let state = await call(page, 'clear_undo');
  await call(page, 'set_node_prop', {nodeType: 'object', nodeId: state.scene.objects[0].id, prop: 'name', value: 'renamed'});
  await call(page, 'undo');
  await load(page, 'broken.pdb', Buffer.from('invalid input'));
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message), {timeout: 60000}).toContain('no ATOM/HETATM records');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
  state = await call(page, 'redo');
  expect(state.scene.objects).toHaveLength(1);
  expect(state.scene.objects[0].name).toBe('renamed');
});

test('queued files load in order and an invalid file does not block the queue', async ({page}) => {
  await ready(page);
  await page.locator('#files').setInputFiles([
    {name: 'first.pdb', mimeType: 'application/octet-stream', buffer: Buffer.from(pdb)},
    {name: 'broken.pdb', mimeType: 'application/octet-stream', buffer: Buffer.from('invalid')},
    {name: 'last.pdb', mimeType: 'application/octet-stream', buffer: Buffer.from(pdb)},
  ]);
  await loaded(page, 'last.pdb');
  const objects = (await page.evaluate(() => window.cuemol.status())).scene.objects;
  expect(objects.map(object => object.name)).toEqual(['first.pdb', 'last.pdb']);
  await call(page, 'undo');
  expect((await page.evaluate(() => window.cuemol.status())).scene.objects.map(object => object.name)).toEqual(['first.pdb']);
});

test('a queued scene becomes the destination for its following molecule', async ({page}) => {
  await ready(page);
  await load(page, 'original.pdb', Buffer.from(pdb));
  await loaded(page, 'original.pdb');
  const before = await page.evaluate(() => window.cuemol.status().scene.sceneId);
  const download = page.waitForEvent('download');
  const button = await page.evaluate(() => window.cuemol.status().ui.rects['Save Scene']);
  await page.mouse.click(button[0] + button[2] / 2, button[1] + button[3] / 2);
  const chunks = [];
  for await (const chunk of await (await download).createReadStream()) chunks.push(chunk);
  await page.locator('#files').setInputFiles([
    {name: 'queued.qsc', mimeType: 'application/octet-stream', buffer: Buffer.concat(chunks)},
    {name: 'following.pdb', mimeType: 'application/octet-stream', buffer: Buffer.from(pdb)},
  ]);
  await loaded(page, 'following.pdb');
  const state = await page.evaluate(() => window.cuemol.status());
  expect(state.scene.sceneId).not.toBe(before);
  expect(state.scene.objects.map(object => object.name)).toEqual(['original.pdb', 'following.pdb']);
});

test('worker is reused for nearby loads and released after idle', async ({page}) => {
  await ready(page);
  const loaders = () => page.workers().filter(worker => worker.url().endsWith('/loader-worker.js'));
  await load(page, 'first.pdb', Buffer.from(pdb));
  await loaded(page, 'first.pdb');
  expect(loaders()).toHaveLength(1);
  const first = loaders()[0];
  await load(page, 'second.pdb', Buffer.from(pdb));
  await loaded(page, 'second.pdb');
  expect(loaders()).toEqual([first]);
  await expect.poll(() => loaders().length, {timeout: 10000}).toBe(0);
  await load(page, 'after-idle.pdb', Buffer.from(pdb));
  await loaded(page, 'after-idle.pdb');
  expect(loaders()).toHaveLength(1);
  expect(loaders()[0]).not.toBe(first);
});

test('cancel button discards queued files and scene changes reject late input', async ({page}) => {
  await ready(page);
  let release = await holdWorker(page);
  try {
    await page.locator('#files').setInputFiles(['cancelled.pdb', 'queued.pdb'].map(name => ({
      name, mimeType: 'application/octet-stream', buffer: Buffer.from(pdb),
    })));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().ui.rects['Cancel loading'])).toBeTruthy();
    const rect = await page.evaluate(() => window.cuemol.status().ui.rects['Cancel loading']);
    await page.mouse.click(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
  } finally { await release(); }
  expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(0);
  release = await holdWorker(page);
  try {
    await page.locator('#files').setInputFiles(['late.pdb', 'late-queued.pdb'].map(name => ({
      name, mimeType: 'application/octet-stream', buffer: Buffer.from(pdb),
    })));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(true);
    const before = await page.evaluate(() => window.cuemol.status().scene.sceneId);
    const id = await page.evaluate(() => window.cuemol.command('create_scene', {name: 'New scene'}));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(id);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.sceneId)).not.toBe(before);
  } finally { await release(); }
  await expect.poll(() => page.evaluate(() => window.cuemol.status().loading), {timeout: 60000}).toBe(false);
  const state = await page.evaluate(() => window.cuemol.status());
  expect(state.message).toBe('File loading cancelled because the active scene changed.');
  expect(state.scene.objects).toHaveLength(0);
});

test('scene changes cancel pending ICC input before it alters the new scene', async ({page}) => {
  await ready(page);
  const profile = await page.evaluate(async () => {
    const {default: create} = await import('./pkg/kernels/lcms.js');
    const cms = await create();
    const handle = cms._cmsCreate_sRGBProfile(), size = cms._malloc(4);
    try {
      cms._cmsSaveProfileToMem(handle, 0, size);
      const length = cms.HEAPU32[size / 4], data = cms._malloc(length);
      try {
        cms._cmsSaveProfileToMem(handle, data, size);
        return Array.from(cms.HEAPU8.subarray(data, data + length));
      } finally { cms._free(data); }
    } finally { cms._free(size); cms._cmsCloseProfile(handle); }
  });
  await page.evaluate(() => {
    const read = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = async function () {
      if (this.name === 'late.icc') await new Promise(resolve => { window.releaseProfile = resolve; });
      return read.call(this);
    };
  });
  await load(page, 'late.icc', Buffer.from(profile));
  await expect.poll(() => page.evaluate(() => typeof window.releaseProfile)).toBe('function');
  const before = await page.evaluate(() => window.cuemol.status().scene.sceneId);
  const id = await page.evaluate(() => window.cuemol.command('create_scene', {name: 'New scene'}));
  await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(id);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.sceneId)).not.toBe(before);
  await page.evaluate(() => window.releaseProfile());
  await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
  expect(await page.evaluate(() => window.cuemol.status().message)).toBe('File loading cancelled because the active scene changed.');
  expect(await page.evaluate(() => localStorage.getItem('cuemol.icc.late.icc'))).toBeNull();
});

test('scene changes cancel files queued before their parser starts', async ({page}) => {
  await ready(page);
  const before = await page.evaluate(() => window.cuemol.status().scene.sceneId);
  const id = await page.evaluate(source => {
    const transfer = new DataTransfer();
    transfer.items.add(new File([source], 'queued-before-start.pdb'));
    const input = document.querySelector('#files');
    input.files = transfer.files;
    input.dispatchEvent(new Event('change', {bubbles: true}));
    return window.cuemol.command('create_scene', {name: 'New scene'});
  }, pdb);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(id);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().scene.sceneId)).not.toBe(before);
  await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
  const state = await page.evaluate(() => window.cuemol.status());
  expect(state.message).toBe('File loading cancelled because the active scene changed.');
  expect(state.scene.objects).toHaveLength(0);
});

test('cancelling queued input keeps an active render busy and cancellable', async ({page}) => {
  await ready(page);
  await load(page, 'render.pdb', Buffer.from(pdb));
  await loaded(page, 'render.pdb');
  await call(page, 'set_render_settings', {settings: 'backend=umbreon; umbreon.supersample=1; umbreon.useGI=false; umbreon.denoise=None'});
  const cancel = await holdRenderYield(page);
  await page.evaluate(() => window.cuemol.command('get_screenshot', {width: 64, height: 48}));
  await expect.poll(() => page.evaluate(() => window.renderYield.held)).toBe(true);
  expect(await page.evaluate(() => window.cuemol.status().busy)).toBe(true);
  try {
    const id = await page.evaluate(source => {
      const transfer = new DataTransfer();
      transfer.items.add(new File([source], 'cancelled-during-render.pdb'));
      const input = document.querySelector('#files');
      input.files = transfer.files;
      input.dispatchEvent(new Event('change', {bubbles: true}));
      return window.cuemol.command('cancel_load');
    }, pdb);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(id);
    const state = await page.evaluate(() => window.cuemol.status());
    expect(state.busy).toBe(true);
    expect(state.loading).toBe(false);
    expect(state.scene.objects).toHaveLength(1);
    const next = await page.evaluate(() => window.cuemol.command('get_screenshot', {width: 32, height: 32}));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(next);
    expect(await page.evaluate(() => window.cuemol.status().message)).toBe('Wait for the current operation to finish.');
  } finally {
    await cancel();
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Render cancelled');
  }
});

for (const action of ['scene change', 'Escape']) {
test(`PDB downloads cancel their pending request on ${action}`, async ({page}) => {
  await ready(page);
  let release, requested;
  let aborted = false;
  page.on('requestfailed', request => {
    if (request.url() === 'https://files.rcsb.org/download/1CRN.cif') aborted = true;
  });
  const gate = new Promise(resolve => { release = resolve; });
  const request = new Promise(resolve => { requested = resolve; });
  await page.route('https://files.rcsb.org/download/1CRN.cif', async route => {
    requested();
    await gate;
    await route.fulfill({contentType: 'text/plain', body: downloadedCif}).catch(() => {});
  });
  try {
    await fetchPdb(page);
    await request;
    const before = await page.evaluate(() => window.cuemol.status().scene.sceneId);
    if (action === 'scene change') {
      await call(page, 'create_scene', {name: 'New destination'});
      expect(await page.evaluate(() => window.cuemol.status().scene.sceneId)).not.toBe(before);
    } else {
      const viewport = await page.evaluate(() => window.cuemol.status().ui.rects.Viewport);
      await page.mouse.click(viewport[0] + viewport[2] / 2, viewport[1] + viewport[3] / 2);
      await page.keyboard.press('Escape');
    }
    await expect.poll(() => page.evaluate(() => window.cuemol.status().loading)).toBe(false);
  } finally { release(); }
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toMatch(/Loaded|cancelled/);
  const state = await page.evaluate(() => window.cuemol.status());
  expect(state.scene.objects).toHaveLength(0);
  expect(state.message).toBe(action === 'scene change'
    ? 'File loading cancelled because the active scene changed.' : 'File loading cancelled.');
  await expect.poll(() => aborted).toBe(true);
});
}

test('PDB download failures do not unlock a concurrent render', async ({page}) => {
  await ready(page);
  await load(page, 'render.pdb', Buffer.from(pdb));
  await loaded(page, 'render.pdb');
  let requested = 0;
  await page.route('https://files.rcsb.org/download/1CRN.cif', route => {
    requested++; return route.fulfill({status: 503, body: 'Unavailable'});
  });
  await call(page, 'set_render_settings', {settings: 'backend=umbreon; umbreon.supersample=1; umbreon.useGI=false; umbreon.denoise=None'});
  const cancel = await holdRenderYield(page);
  await page.evaluate(() => window.cuemol.command('get_screenshot', {width: 64, height: 48}));
  await expect.poll(() => page.evaluate(() => window.renderYield.held)).toBe(true);
  expect(await page.evaluate(() => window.cuemol.status().busy)).toBe(true);
  try {
    await fetchPdb(page);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toBe('Queued download 1CRN.');
    expect(await page.evaluate(() => window.cuemol.status().busy)).toBe(true);
    expect(requested).toBe(0);
  } finally {
    await cancel();
  }
  await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('HTTP 503');
  await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
  expect(requested).toBe(1);
  await page.unroute('https://files.rcsb.org/download/1CRN.cif');
  await page.route('https://files.rcsb.org/download/1CRN.cif', route => route.fulfill({contentType: 'text/plain', body: downloadedCif}));
  await fetchPdb(page);
  await loaded(page, '1CRN.cif');
  expect((await page.evaluate(() => window.cuemol.status())).scene.objects).toHaveLength(2);
});

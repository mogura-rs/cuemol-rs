import {test, expect} from '@playwright/test';
import {readFile, writeFile} from 'node:fs/promises';
import {acceptFileOptions, call, load} from './support.js';

const atomCount = 512;
const frameCount = 128;

function molecule() {
  return Buffer.from(Array.from({length: atomCount}, (_, i) =>
    `HETATM${String(i + 1).padStart(5)}  C1  LIG A${String(i + 1).padStart(4)}    ` +
    [2.5 * (i % 32), 2.5 * Math.floor(i / 32), 0].map(value => value.toFixed(3).padStart(8)).join('') +
    '  1.00 10.00           C\n').join('') + 'END\n');
}

function trajectory() {
  const chunks = [];
  const record = data => {
    const size = Buffer.alloc(4);
    size.writeInt32LE(data.length);
    chunks.push(size, data, size);
  };
  const header = Buffer.alloc(84);
  header.write('CORD');
  header.writeInt32LE(frameCount, 4);
  record(header);
  record(Buffer.alloc(4));
  const count = Buffer.alloc(4);
  count.writeInt32LE(atomCount);
  record(count);
  for (let frame = 0; frame < frameCount; frame++) {
    for (let axis = 0; axis < 3; axis++) {
      const values = Buffer.alloc(atomCount * 4);
      for (let i = 0; i < atomCount; i++) {
        const value = axis === 0 ? 2.5 * (i % 32) + frame / 4
          : axis === 1 ? 2.5 * Math.floor(i / 32) : 0;
        values.writeFloatLE(value, i * 4);
      }
      record(values);
    }
  }
  return Buffer.concat(chunks);
}

function potential() {
  const size = 32;
  const values = Array.from({length: size ** 3}, (_, i) =>
    `${(i % size) / 4 - 4}`).join(' ');
  return Buffer.from(`object 1 class gridpositions counts ${size} ${size} ${size}\n` +
    'origin 0 0 0\ndelta 1 0 0\ndelta 0 1 0\ndelta 0 0 1\n' +
    `object 3 class array type double rank 0 items ${size ** 3} data follows\n${values}\n`);
}

function surface() {
  const size = 48;
  const lines = [];
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    lines.push(`v ${x} ${y} ${(x + y) % 3 / 4}`);
  }
  for (let y = 0; y < size - 1; y++) for (let x = 0; x < size - 1; x++) {
    const i = y * size + x + 1;
    lines.push(`f ${i} ${i + 1} ${i + size}`, `f ${i + 1} ${i + size + 1} ${i + size}`);
  }
  return Buffer.from(lines.join('\n'));
}

for (const backend of ['webgl', 'webgpu']) {
  test(`repeated mixed scenes ${backend} retain history and release closed scene storage`, async ({page}, testInfo) => {
    test.setTimeout(180000);
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.route('https://data.rcsb.org/graphql', route => route.fulfill({
      status: 503, headers: {'access-control-allow-origin': '*'}, body: '',
    }));
    await page.goto(`./?renderer=${backend}`);
    await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
    const base = (await call(page, 'clear_undo')).scene.sceneId;
    const files = [['atoms.pdb', molecule()], ['motion.dcd', trajectory()],
      ['potential.dx', potential()], ['surface.obj', surface()]];
    const samples = [];
    for (let cycle = 0; cycle < 12; cycle++) {
      await call(page, 'create_scene', {name: `Cycle ${cycle}`});
      await page.locator('#files').setInputFiles(files.map(([name, buffer]) =>
        ({name, mimeType: 'application/octet-stream', buffer})));
      await acceptFileOptions(page, {fixtureNames: true});
      let state = await call(page, 'clear_undo');
      expect(state.scene.objects).toHaveLength(3);
      const object = state.scene.objects.find(item => item.atoms === atomCount).id;
      expect(JSON.parse((await call(page, 'list_trajectories')).message).trajectories.map(item => item.frames)).toEqual([frameCount]);
      for (const [frame, expected] of [[0, 0], [63, 15.75], [127, 31.75]]) {
        await call(page, 'set_frame', {objId: object, frame});
        const result = await call(page, 'execute_script', {
          source: `print(cuemol.getObject(${object}).getAtomByID(1).pos.x);`,
        });
        expect(Number(JSON.parse(result.message).log.trim())).toBe(expected);
      }
      await call(page, 'set_mol_selection', {objId: object, selection: 'A.1:8.*'});
      await call(page, 'rename_node', {nodeType: 'object', nodeId: object, name: 'retained motion'});
      const camera = await call(page, 'set_view', {center: [cycle, 4, 8], zoom: 100 + cycle});
      for (let repeat = 0; repeat < 3; repeat++) {
        state = await call(page, 'undo');
        expect(state.scene.objects.find(item => item.id === object).name).toBe('atoms.pdb');
        expect(state.view).toEqual(camera.view);
        state = await call(page, 'redo');
        expect(state.scene.objects.find(item => item.id === object).name).toBe('retained motion');
        expect(state.view).toEqual(camera.view);
      }
      const list = await call(page, 'list_trajectories');
      expect(JSON.parse(list.message).trajectories.map(item => item.frames)).toEqual([frameCount]);
      if (cycle === 3 || cycle === 11) {
        const pending = page.waitForEvent('download');
        await call(page, 'save_scene', {path: 'mixed.qsc', embedAll: true, version: 'QDF1', compress: 'gzip', base64: true});
        const bytes = await readFile(await (await pending).path());
        await load(page, 'mixed.qsc', bytes);
        state = await call(page, 'list_trajectories');
        expect(state.scene.objects).toHaveLength(3);
        expect(JSON.parse(state.message).trajectories.map(item => item.frames)).toEqual([frameCount]);
        const restored = state.scene.objects.find(item => item.name === 'retained motion').id;
        const averaging = await call(page, 'execute_script', {
          source: `var t=cuemol.getObject(${restored});t.frame_aver_size=4;print(t.type+':'+t.frame_aver_size+':'+t.nframe);`,
        });
        expect(JSON.parse(averaging.message).log.trim()).toBe(`Trajectory:4:${frameCount}`);
        await call(page, 'set_frame', {objId: restored, frame: 0});
        const result = await call(page, 'execute_script', {source: `print(cuemol.getObject(${restored}).getAtomByID(1).pos.x);`});
        expect(Number(JSON.parse(result.message).log.trim())).toBe(0.5);
      }
      const scenes = JSON.parse((await call(page, 'list_scenes')).message);
      await call(page, 'switch_scene', {sceneId: base});
      for (const scene of scenes.filter(item => item.sceneId !== base)) {
        await call(page, 'close_scene', {sceneId: scene.sceneId, discardChanges: true});
      }
      state = await call(page, 'clear_undo');
      expect(state.scene.sceneId).toBe(base);
      expect(state.scene.objects).toEqual([]);
      expect(JSON.parse((await call(page, 'list_trajectories')).message).trajectories).toEqual([]);
      const sample = await page.evaluate(async () => {
        const module = await import('./pkg/cuemol_web.js');
        const exports = await module.default();
        return {wasmBytes: exports.memory.buffer.byteLength, jsHeapBytes: performance.memory?.usedJSHeapSize ?? null};
      });
      samples.push({cycle, ...sample});
    }
    await expect.poll(() => page.workers().filter(worker => worker.url().endsWith('/loader-worker.js')).length, {timeout: 10000}).toBe(0);
    const retention = testInfo.outputPath('retention.json');
    await writeFile(retention, JSON.stringify({atomCount, frameCount, samples}, null, 2));
    await testInfo.attach('retention.json', {path: retention, contentType: 'application/json'});
    expect(samples.at(-1).wasmBytes).toBeLessThanOrEqual(samples[3].wasmBytes + 16 * 1024 * 1024);
    expect(errors).toEqual([]);
  });
}

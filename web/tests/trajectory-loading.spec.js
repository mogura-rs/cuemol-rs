import {test, expect} from '@playwright/test';
import {load, call} from './support.js';

function topology(atoms) {
  return Buffer.from(Array.from({length: atoms}, (_, i) =>
    `HETATM${String(i + 1).padStart(5)}  C1  LIG A${String(i + 1).padStart(4)}    ` +
    [i * 2, 0, 0].map(value => value.toFixed(3).padStart(8)).join('') +
    '  1.00 10.00           C\n').join('') + 'END\n');
}

function xtc(frames, atoms = 3) {
  const bytes = Buffer.alloc(frames * (56 + atoms * 12));
  for (let frame = 0; frame < frames; frame++) {
    const offset = frame * (56 + atoms * 12);
    bytes.writeInt32BE(1995, offset);
    bytes.writeInt32BE(atoms, offset + 4);
    bytes.writeInt32BE(frame, offset + 8);
    for (let axis = 0; axis < 3; axis++) bytes.writeFloatBE(1, offset + 16 + axis * 16);
    bytes.writeInt32BE(atoms, offset + 52);
    for (let atom = 0; atom < atoms; atom++) {
      bytes.writeFloatBE(atom + frame / 4, offset + 56 + atom * 12);
    }
  }
  return bytes;
}

function dcd(frames, atoms = 3) {
  const parts = [];
  const record = bytes => {
    const marker = Buffer.alloc(4);
    marker.writeUInt32LE(bytes.length);
    parts.push(marker, bytes, marker);
  };
  const header = Buffer.alloc(84);
  header.write('CORD');
  header.writeUInt32LE(frames, 4);
  record(header);
  record(Buffer.alloc(4));
  const count = Buffer.alloc(4);
  count.writeUInt32LE(atoms);
  record(count);
  for (let frame = 0; frame < frames; frame++) {
    for (let axis = 0; axis < 3; axis++) {
      const coordinates = Buffer.alloc(atoms * 4);
      for (let atom = 0; atom < atoms; atom++) {
        coordinates.writeFloatLE(axis === 0 ? frame + atom * 2 : 0, atom * 4);
      }
      record(coordinates);
    }
  }
  return Buffer.concat(parts);
}

function trr(frames, double) {
  const parts = [];
  for (let frame = 0; frame < frames; frame++) {
    const width = double ? 8 : 4;
    const bytes = Buffer.alloc(76 + 20 * width);
    bytes.writeInt32BE(1993);
    bytes.writeInt32BE(13, 4);
    bytes.writeInt32BE(12, 8);
    bytes.write('GMX_trn_file\0', 12);
    [0, 0, 9 * width, 0, 0, 0, 0, 9 * width, 0, 0, 3, frame, 0]
      .forEach((value, index) => bytes.writeInt32BE(value, 24 + index * 4));
    const values = [frame, 0, 3 + frame, 0, 0, 0, 4, 0, 0, 0, 5,
      ...Array.from({length: 9}, (_, index) => (index + frame) / 4)];
    values.forEach((value, index) => double
      ? bytes.writeDoubleBE(value, 76 + index * width)
      : bytes.writeFloatBE(value, 76 + index * width));
    parts.push(bytes);
  }
  return Buffer.concat(parts);
}

function netcdf(frames) {
  const parts = [Buffer.from('CDF\x01')];
  const ints = (...values) => {
    const bytes = Buffer.alloc(values.length * 4);
    values.forEach((value, index) => bytes.writeUInt32BE(value, index * 4));
    parts.push(bytes);
  };
  const name = value => {
    ints(value.length);
    parts.push(Buffer.from(value), Buffer.alloc((-value.length >>> 0) % 4));
  };
  ints(frames, 10, 3);
  for (const [label, size] of [['frame', 0], ['atom', 3], ['spatial', 3]]) {
    name(label); ints(size);
  }
  ints(12, 1); name('Conventions'); ints(2, 5); parts.push(Buffer.from('AMBER\0\0\0'));
  ints(11, 3);
  const offsets = [];
  for (const [label, dimensions, size, offset] of [
    ['coordinates', [0, 1, 2], 36, 0], ['cell_lengths', [0, 2], 12, 36],
    ['cell_angles', [0, 2], 12, 48],
  ]) {
    name(label); ints(dimensions.length, ...dimensions, 0, 0, 5, size);
    const position = Buffer.alloc(4);
    offsets.push([position, offset]); parts.push(position);
  }
  const length = parts.reduce((sum, part) => sum + part.length, 0);
  for (const [position, offset] of offsets) position.writeUInt32BE(length + offset);
  for (let frame = 0; frame < frames; frame++) {
    const bytes = Buffer.alloc(60);
    [...Array.from({length: 9}, (_, index) => (index + frame) / 4),
      30 + frame, 40, 50, 90, 90, 90]
      .forEach((value, index) => bytes.writeFloatBE(value, index * 4));
    parts.push(bytes);
  }
  return Buffer.concat(parts);
}

async function ready(page, atoms, backend) {
  await page.route('https://data.rcsb.org/graphql', route => route.fulfill({
    status: 503, headers: {'access-control-allow-origin': '*'}, body: '',
  }));
  await page.goto(`./?renderer=${backend}`);
  await expect.poll(() => page.evaluate(() => window.cuemol?.status()?.scene?.sceneId), {timeout: 60000}).toBeGreaterThan(0);
  await load(page, 'topology.pdb', topology(atoms));
  return call(page, 'clear_undo');
}

async function holdFrame(page, name, offset) {
  await page.evaluate(({name, offset}) => {
    const slice = File.prototype.slice;
    let resume;
    const gate = new Promise(resolve => { resume = resolve; });
    window.frameGate = {held: false, release: () => resume()};
    File.prototype.slice = function(...args) {
      const blob = slice.apply(this, args);
      if (this.name === name && args[0] === offset) {
        const read = blob.arrayBuffer.bind(blob);
        blob.arrayBuffer = async () => {
          window.frameGate.held = true;
          await gate;
          return read();
        };
      }
      return blob;
    };
  }, {name, offset});
}

for (const backend of ['webgl', 'webgpu']) {
test(`mixed binary trajectories ${backend} preserve coordinates and cells through scene restoration`, async ({page}) => {
  const state = await ready(page, 3, backend);
  let object = state.scene.objects[0].id;
  for (const [name, bytes] of [['small.xtc', xtc(3)], ['single.trr', trr(3, false)],
    ['double.trr', trr(3, true)], ['amber.nc', netcdf(3)]]) {
    await load(page, name, bytes);
    expect((await page.evaluate(() => window.cuemol.status())).message).toContain('Indexed 3 frames');
  }
  const snapshot = async frame => {
    await call(page, 'set_frame', {objId: object, frame});
    const result = await call(page, 'execute_script', {source: `
      var o = cuemol.getObject(${object}), c = o.getExtData('CrystalInfo');
      print(JSON.stringify({positions: [1,2,3].map(function(i) {
        var a = o.getAtomByID(i); if (!a) return null;
        var p = a.pos; return [p.x,p.y,p.z];
      }), cell: c ? [c.a,c.b,c.c,c.alpha,c.beta,c.gamma] : null}));`});
    expect(result.message).toMatch(/^\{/);
    return JSON.parse(JSON.parse(result.message).log.trim());
  };
  const before = [];
  for (let frame = 0; frame < 12; frame++) before.push(await snapshot(frame));
  for (let frame = 0; frame < 3; frame++) {
    expect(before[frame].positions[0]).toEqual([frame * 2.5, 0, 0]);
    expect(before[frame].cell).toEqual([10, 10, 10, 90, 90, 90]);
    for (const offset of [3, 6]) {
      expect(before[offset + frame].positions[0]).toEqual([frame * 2.5, (frame + 1) * 2.5, (frame + 2) * 2.5]);
      expect(before[offset + frame].cell).toEqual([30 + frame * 10, 40, 50, 90, 90, 90]);
    }
    expect(before[9 + frame].positions[0]).toEqual([frame / 4, (frame + 1) / 4, (frame + 2) / 4]);
    expect(before[9 + frame].cell).toEqual([30 + frame, 40, 50, 90, 90, 90]);
  }
  const pending = page.waitForEvent('download');
  await call(page, 'save_scene', {path: 'mixed.qsc', embedAll: true, version: 'QDF1', compress: 'gzip', base64: true});
  const stream = await (await pending).createReadStream(), chunks = [];
  for await (const chunk of stream) chunks.push(chunk);
  await page.reload();
  await expect.poll(() => page.evaluate(() => !!window.cuemol), {timeout: 60000}).toBe(true);
  await load(page, 'mixed.qsc', Buffer.concat(chunks));
  object = (await page.evaluate(() => window.cuemol.status())).scene.objects[0].id;
  expect(JSON.parse((await call(page, 'list_trajectories')).message).trajectories[0].frames).toBe(12);
  for (let frame = 11; frame >= 0; frame--) expect(await snapshot(frame)).toEqual(before[frame]);
});

test(`overflowing XTC lengths ${backend} terminate without repeated indexing or history changes`, async ({page}) => {
  const original = await ready(page, 10, backend);
  await call(page, 'rename_node', {nodeType: 'object', nodeId: original.scene.objects[0].id, name: 'renamed'});
  await call(page, 'undo');
  await page.evaluate(() => {
    const slice = File.prototype.slice;
    window.overflowReads = 0;
    File.prototype.slice = function(...args) {
      if (this.name === 'overflow.xtc' && ++window.overflowReads > 6) {
        throw new Error('Repeated trajectory read tripwire');
      }
      return slice.apply(this, args);
    };
  });
  const bytes = Buffer.alloc(96);
  bytes.writeInt32BE(2023);
  bytes.writeInt32BE(10, 4);
  bytes.writeInt32BE(10, 52);
  bytes.writeBigUInt64BE(0xffffffffffffffa0n, 88);
  await load(page, 'overflow.xtc', bytes);
  const failed = await page.evaluate(() => window.cuemol.status());
  expect(failed.message).toMatch(/overflow|Invalid.*size|truncated/i);
  expect(await page.evaluate(() => window.overflowReads)).toBeLessThanOrEqual(2);
  expect(failed.scene.objects).toEqual(original.scene.objects);
  const restored = await call(page, 'redo');
  expect(restored.scene.objects.map(object => object.name)).toEqual(['renamed']);
  await load(page, 'mismatched.xtc', xtc(2, 3));
  await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
  expect((await page.evaluate(() => window.cuemol.status())).message).toContain('atom count');
  expect(JSON.parse((await call(page, 'list_trajectories')).message).trajectories).toEqual([]);
});

test(`cancelling trajectory indexing ${backend} stops file reads and preserves the next load`, async ({page}) => {
  const original = await ready(page, 3, backend);
  await page.evaluate(() => {
    const slice = File.prototype.slice;
    let resume;
    const gate = new Promise(resolve => { resume = resolve; });
    window.trajectoryGate = {reads: 0, held: false, release: () => resume()};
    File.prototype.slice = function(...args) {
      const blob = slice.apply(this, args);
      if (this.name === 'cancel.xtc') {
        const count = ++window.trajectoryGate.reads;
        if (count === 2) {
          const read = blob.arrayBuffer.bind(blob);
          blob.arrayBuffer = async () => {
            window.trajectoryGate.held = true;
            await gate;
            return read();
          };
        }
      }
      return blob;
    };
  });
  try {
    await load(page, 'cancel.xtc', xtc(10000), {accept: false});
    await expect.poll(() => page.evaluate(() => window.trajectoryGate.held)).toBe(true);
    await call(page, 'cancel_load');
    await page.evaluate(() => window.trajectoryGate.release());
    await load(page, 'valid.xtc', xtc(2));
    await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
    const trajectories = JSON.parse((await call(page, 'list_trajectories')).message).trajectories;
    expect(trajectories.map(item => item.frames)).toEqual([2]);
    expect(trajectories[0].objectId).toBe(original.scene.objects[0].id);
    expect(await page.evaluate(() => window.trajectoryGate.reads)).toBe(2);
    await call(page, 'set_frame', {objId: original.scene.objects[0].id, frame: 1});
    const result = await call(page, 'execute_script', {source: `print(cuemol.getObject(${original.scene.objects[0].id}).getAtomByID(1).pos.x);`});
    expect(Number(JSON.parse(result.message).log.trim())).toBe(2.5);
  } finally {
    await page.evaluate(() => window.trajectoryGate.release());
  }
});

test(`a superseded frame failure ${backend} preserves the newer edit and its result`, async ({page}) => {
  const original = await ready(page, 3, backend);
  const object = original.scene.objects[0].id;
  const bytes = dcd(2);
  bytes.writeUInt32LE(0, 192);
  await load(page, 'partial.dcd', bytes);
  await holdFrame(page, 'partial.dcd', 176);
  try {
    await page.evaluate(object => window.cuemol.command('set_frame', {objId: object, frame: 1}), object);
    await expect.poll(() => page.evaluate(() => window.frameGate.held)).toBe(true);
    const command = await page.evaluate(object => window.cuemol.command('rename_node', {
      nodeType: 'object', nodeId: object, name: 'newer edit',
    }), object);
    await expect.poll(() => page.evaluate(() => window.cuemol.status().lastCommand)).toBe(command);
    const edited = await page.evaluate(() => window.cuemol.status());
    await page.evaluate(() => window.frameGate.release());
    await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
    const after = await page.evaluate(() => window.cuemol.status());
    expect(after.message).toBe(edited.message);
    expect(after.scene.objects[0].name).toBe('newer edit');
    expect(after.frame).toBe(0);
    await call(page, 'undo');
    expect((await page.evaluate(() => window.cuemol.status())).scene.objects[0].name).toBe('topology.pdb');
    const failed = await call(page, 'set_frame', {objId: object, frame: 1});
    expect(failed.message).toMatch(/DCD|record/i);
    expect(failed.frame).toBe(0);
    await call(page, 'set_frame', {objId: object, frame: 0});
  } finally {
    await page.evaluate(() => window.frameGate.release());
  }
});

test(`action import errors ${backend} retain the busy state of a pending frame`, async ({page}) => {
  const original = await ready(page, 3, backend);
  const object = original.scene.objects[0].id;
  await load(page, 'valid.dcd', dcd(2));
  await holdFrame(page, 'valid.dcd', 176);
  await page.evaluate(() => {
    const text = File.prototype.text;
    File.prototype.text = function(...args) {
      return this.name === 'unreadable.qax'
        ? Promise.reject(new Error('Synthetic action file read failure'))
        : text.apply(this, args);
    };
  });
  try {
    await page.evaluate(object => window.cuemol.command('set_frame', {objId: object, frame: 1}), object);
    await expect.poll(() => page.evaluate(() => window.frameGate.held)).toBe(true);
    await load(page, 'oversized.qax', Buffer.alloc(8_388_609), {accept: false});
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Action files must be smaller');
    expect((await page.evaluate(() => window.cuemol.status())).busy).toBe(true);
    await load(page, 'unreadable.qax', Buffer.from('unreadable'), {accept: false});
    await expect.poll(() => page.evaluate(() => window.cuemol.status().message)).toContain('Synthetic action file read failure');
    expect((await page.evaluate(() => window.cuemol.status())).busy).toBe(true);
    await page.evaluate(() => window.frameGate.release());
    await expect.poll(() => page.evaluate(() => window.cuemol.status().busy)).toBe(false);
    expect((await page.evaluate(() => window.cuemol.status())).frame).toBe(1);
    await call(page, 'set_frame', {objId: object, frame: 0});
  } finally {
    await page.evaluate(() => window.frameGate.release());
  }
});
}

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

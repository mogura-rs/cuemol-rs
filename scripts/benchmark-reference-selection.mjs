import path from 'node:path';
import {existsSync} from 'node:fs';
import {createRequire} from 'node:module';

const [root, fixture, iterationsText, expression] = process.argv.slice(2);
const iterations = Number(iterationsText);
if (!root || !fixture || !Number.isInteger(iterations) || iterations < 1 || !expression) {
  throw new Error('usage: benchmark-reference-selection.mjs APP_ROOT MOLECULE ITERATIONS EXPRESSION');
}
const resources = existsSync(path.join(root, 'Contents/Resources')) ? path.join(root, 'Contents/Resources') : path.join(root, 'resources');
const require = createRequire(import.meta.url);
const cm = require(path.join(resources, 'app/node_modules/@cuemol/core/src/index.cjs')).getModule();
cm.initCueMol(path.join(resources, 'cuemol2/share/sysconfig.xml'));
const reader = cm.getService('StreamManager').invokeMethod('createHandler', 'pdb', 0);
reader.invokeMethod('setPath', path.resolve(fixture));
const molecule = reader.invokeMethod('createDefaultObj');
reader.invokeMethod('attach', molecule);
reader.invokeMethod('read');
reader.invokeMethod('detach');
let selection;
function query() {
  selection = cm.createObj('SelCommand');
  if (!selection.invokeMethod('compile', expression, 0)) throw new Error('Invalid selection');
  return molecule.invokeMethod('getAtomSelSize', selection);
}
let start = performance.now();
const count = query(), cold_ms = performance.now() - start;
start = performance.now();
for (let index = 0; index < iterations; index++) {
  if (query() !== count) throw new Error('Selection count changed between repeated queries');
}
const query_ms = (performance.now() - start) / iterations;
const all = cm.createObj('SelCommand');
all.invokeMethod('compile', '*', 0);
const allIds = molecule.invokeMethod('getSelArray', all);
const order = new Map();
for (let index = 0; index < allIds.getProp('elemCount'); index++) order.set(allIds.invokeMethod('getAt', index), index);
const selected = molecule.invokeMethod('getSelArray', selection);
if (selected.getProp('elemCount') !== count) throw new Error('Selection mask differs from count');
let digest = 0xcbf29ce484222325n;
for (let index = 0; index < count; index++) {
  const position = order.get(selected.invokeMethod('getAt', index));
  if (position === undefined) throw new Error('Selected atom missing from molecule');
  let value = BigInt(position);
  for (let byte = 0; byte < 8; byte++, value >>= 8n) digest = BigInt.asUintN(64, (digest ^ (value & 255n)) * 0x100000001b3n);
}
console.log('SELECTION_RESULT ' + JSON.stringify({expression, iterations, atoms:order.size, count, digest:digest.toString(16).padStart(16, '0'), cold_ms, query_ms}));

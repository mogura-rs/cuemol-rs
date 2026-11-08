import path from 'node:path';
import {existsSync} from 'node:fs';
import {createRequire} from 'node:module';

const [root, input, output] = process.argv.slice(2);
if (!root || !input || !output || path.resolve(input) === path.resolve(output)) {
  throw new Error('usage: export-reference-base-pairs.mjs APP_ROOT INPUT_PDB OUTPUT_QDF');
}
const resources = existsSync(path.join(root, 'Contents/Resources'))
  ? path.join(root, 'Contents/Resources') : path.join(root, 'resources');
const cm = createRequire(import.meta.url)(path.join(resources, 'app/node_modules/@cuemol/core/src/index.cjs')).getModule();
cm.initCueMol(path.join(resources, 'cuemol2/share/sysconfig.xml'));
const streams = cm.getService('StreamManager');
const reader = streams.invokeMethod('createHandler', 'pdb', 0);
reader.invokeMethod('setPath', path.resolve(input));
const molecule = reader.invokeMethod('createDefaultObj');
reader.invokeMethod('attach', molecule);
try { reader.invokeMethod('read'); } finally { reader.invokeMethod('detach'); }
molecule.invokeMethod('calcBasePair', 3.7, 30);
const writer = streams.invokeMethod('createHandler', 'qdfmol', 1);
writer.invokeMethod('setPath', path.resolve(output));
writer.invokeMethod('attach', molecule);
try { writer.invokeMethod('write'); } finally { writer.invokeMethod('detach'); }
console.log(JSON.stringify({atoms: molecule.invokeMethod('getAtomSize'), bonds: molecule.invokeMethod('getBondSize')}));

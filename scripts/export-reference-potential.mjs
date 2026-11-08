import path from 'node:path';
import {existsSync} from 'node:fs';
import {createRequire} from 'node:module';

const [root, input, output] = process.argv.slice(2);
if (!root || !input || !output || path.resolve(input) === path.resolve(output)) {
  throw new Error('usage: export-reference-potential.mjs APP_ROOT INPUT_DX OUTPUT_QDF');
}
const resources = existsSync(path.join(root, 'Contents/Resources'))
  ? path.join(root, 'Contents/Resources') : path.join(root, 'resources');
const cm = createRequire(import.meta.url)(path.join(resources, 'app/node_modules/@cuemol/core/src/index.cjs')).getModule();
cm.initCueMol(path.join(resources, 'cuemol2/share/sysconfig.xml'));
const streams = cm.getService('StreamManager');
const reader = streams.invokeMethod('createHandler', 'apbs', 0);
reader.invokeMethod('setPath', path.resolve(input));
const map = reader.invokeMethod('createDefaultObj');
reader.invokeMethod('attach', map);
try { reader.invokeMethod('read'); } finally { reader.invokeMethod('detach'); }
const writer = streams.invokeMethod('createHandler', 'qdfpot', 1);
writer.invokeMethod('setPath', path.resolve(output));
writer.invokeMethod('attach', map);
try { writer.invokeMethod('write'); } finally { writer.invokeMethod('detach'); }
console.log(JSON.stringify({exported: true}));

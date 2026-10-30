import path from 'node:path';
import {existsSync, mkdirSync} from 'node:fs';
import {createRequire} from 'node:module';

const [root, input, output, countText = '2'] = process.argv.slice(2);
const count = Number(countText);
if (!root || !input || !output || !Number.isInteger(count) || count < 1) {
  throw new Error('usage: benchmark-reference-render.mjs APP_ROOT SCENE OUTPUT_DIR REPETITIONS');
}
const resources = existsSync(path.join(root, 'Contents/Resources'))
  ? path.join(root, 'Contents/Resources') : path.join(root, 'resources');
const cm = createRequire(import.meta.url)(path.join(resources, 'app/node_modules/@cuemol/core/src/index.cjs')).getModule();
cm.initCueMol(path.join(resources, 'cuemol2/share/sysconfig.xml'));
const streams = cm.getService('StreamManager');
const start = performance.now();
const scene = cm.getService('SceneManager').invokeMethod('createScene');
const reader = streams.invokeMethod('createHandler', 'qsc_xml', 3);
reader.invokeMethod('setPath', path.resolve(input));
reader.invokeMethod('attach', scene);
try { reader.invokeMethod('read'); } finally { reader.invokeMethod('detach'); }
const loadMs = performance.now() - start;
const objects = scene.getProp('obj_uids').split(',').filter(Boolean).flatMap(uid => {
  const object = scene.invokeMethod('getObject', Number(uid));
  try {
    return [{name: object.getProp('name'), atoms: object.invokeMethod('getAtomSize'), bonds: object.invokeMethod('getBondSize')}];
  } catch { return []; }
});
const settings = scene.invokeMethod('getAppData', 'render');
if (!settings || !scene.invokeMethod('hasCamera', '__current')) {
  throw new Error('The fixture must contain render settings and the __current camera');
}
mkdirSync(output, {recursive: true});
const samples = [];
for (let repetition = 0; repetition < count; repetition++) {
  const prepareStarted = performance.now();
  const exporter = streams.invokeMethod('createHandler', 'umbreon', 2);
  exporter.invokeMethod('applyRenderSettings', settings, 'umbreon');
  exporter.setProp('camera', '__current');
  const file = path.join(output, `render-${repetition}.png`);
  exporter.invokeMethod('setPath', file);
  exporter.invokeMethod('attach', scene);
  const prepareMs = performance.now() - prepareStarted;
  const renderStarted = performance.now();
  try { exporter.invokeMethod('write'); } finally { exporter.invokeMethod('detach'); }
  samples.push({prepare_ms: prepareMs, render_ms: performance.now() - renderStarted,
    result: {path: file, width: exporter.getProp('width'), height: exporter.getProp('height')}});
}
console.log('RENDER_RESULT ' + JSON.stringify({load_ms: loadMs, objects, samples}));

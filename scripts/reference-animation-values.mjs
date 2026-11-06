import { readFileSync, writeFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { resolve } from 'node:path';

const [reference, destination] = process.argv.slice(2);
const catalog = resolve(reference, 'tritium/react-gui/src/renderer/worker/server/catalog');
const ops = readFileSync(resolve(catalog, 'ops/animOps.ts'), 'utf8');
const args = readFileSync(resolve(catalog, 'argValues.ts'), 'utf8');
function extract(source, name) {
  const found = source.match(new RegExp(`(?:export )?function ${name}\\([\\s\\S]*?\\n}`));
  if (!found) throw new Error(`Missing reference function ${name}`);
  return found[0].replace(/^export /, '');
}
const constants = ops.match(/const BOOL_PROPS[\s\S]*?const SET_PROPS[^\n]+/)[0];
const source = [constants, extract(args, 'parseBoolText'), extract(args, 'parseNumberText'),
  extract(ops, 'readPropValue'), extract(ops, 'genericValue')].join('\n');
const { structured, generic } = Function(`${stripTypeScriptTypes(source)}\nreturn {structured:readPropValue,generic:genericValue}`)();
const groups = [
  ['show', 'hide', ['true', 'TRUE', ' false ', 'ON', ' ON ', 'yes', ' no ', '0', '1', '2', '1.0', '', 'maybe']],
  ['spin', 'angle', ['0', '-0', '0x80', '0B11', '0o10', '.5', '1.e2', '\ufeff 2 \ufeff', '+0x10', '1_000', 'abc', '0x20000000000001', '0x20000000000003', '0x2000000000000301']],
  ['show', 'rend', [' named renderer ', ' r1, obj / r2, , /r3, obj/, x/y/z ', '00123', '1e21', 'true', ' false ', '', '0x10']],
  ['camera', 'endcam', [' main ', '1', 'false']],
  ['spin', 'axis', ['1 0 0', '0 0 0', '0 0.00001 0', '0 0.0001 0', '0x2 0 0', '(0,0,2)', '(0,0,0)', '(0,0,0.00001)', '(0,0,0.0001)', '(1,2,3,4)', 'invalid']],
];
const cases = [];
const lines = [];
for (const [kind, prop, inputs] of groups) {
  for (const tool of ['set_anim_prop', 'set_anim_generic_prop']) {
    for (const input of inputs) {
      const value = tool === 'set_anim_prop' ? structured(prop, input) : generic(input);
      let type = typeof value;
      let text = String(value);
      if (type === 'object') {
        type = 'error' in value ? 'error' : 'vector';
        text = type === 'vector' ? `${value.x} ${value.y} ${value.z}` : '';
      }
      const index = cases.length;
      cases.push({ kind, tool, prop, input });
      const hex = Buffer.from(text).toString('hex') || '-';
      lines.push(`${index} ${kind} ${prop} ${type} ${hex}`);
    }
  }
}
writeFileSync(`${destination}.json`, JSON.stringify(cases));
writeFileSync(`${destination}.txt`, lines.join('\n') + '\n');

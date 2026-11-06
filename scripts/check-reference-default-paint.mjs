#!/usr/bin/env node
// Execute the reference palette algorithm with isolated style and molecule fixtures.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {stripTypeScriptTypes} from 'node:module';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const reference = path.resolve(process.argv[2] ?? process.env.CUEMOL2_REF_DIR ?? path.join(root, '../cuemol2'));
const source = path.join(reference, 'tritium/react-gui/src/renderer/worker/server/services/helpers/defPaintColoring.ts');
let code = stripTypeScriptTypes(fs.readFileSync(source, 'utf8'), {mode:'transform', sourceMap:false});
const dependencies = new Set(['makeSel', 'makeColor', 'ensureWritableSceneSet', 'listSceneObjects']);
code = code.replace(/import\s+\{([^}]+)\}\s+from\s+['"][^'"]+['"];?/g, (_, names) => {
  for (const name of names.split(',').map(x=>x.trim())) {
    if (!dependencies.delete(name)) throw new Error(`Unexpected dependency: ${name}`);
  }
  return '';
});
if (dependencies.size || /(?:from\s+|import\s*)['"]/.test(code)) throw new Error('Reference imports changed');
const fixtureFunctions = `
const makeSel = (_ctx, value) => value;
const makeColor = (_ctx, value) => value;
const listSceneObjects = ctx => ({objects:ctx.objects});
const ensureWritableSceneSet = () => 1;
`;
const {createDefPaintColoring} = await import('data:text/javascript;base64,' + Buffer.from(fixtureFunctions + code).toString('base64'));
const results = JSON.parse(fs.readFileSync(0, 'utf8')).map(item => {
  const colors = new Map(Object.entries(item.colors));
  const local = new Map(Object.entries(item.local ?? {}));
  const created = [];
  const entries = [];
  const ctx = {
    objects:item.objects,
    svc:{createObj:kind=> {
      if (kind !== 'PaintColoring') throw new Error(`Unexpected object: ${kind}`);
      return {append:(selection,color)=>entries.push([selection,color])};
    }},
    styleMgr:{
      getStrData:()=>item.theme,
      getColorDefsJSON:scope=>JSON.stringify([...(scope === 0 ? colors : local).keys()]),
      getColor:name=> {
        const rgb = colors.get(name);
        if (!rgb) throw new Error(`Undefined global color: ${name}`);
        return {r:()=>rgb[0],g:()=>rgb[1],b:()=>rgb[2]};
      },
      setColor:(name,color)=> {local.set(name,color);created.push([name,color]);},
    },
  };
  const result = createDefPaintColoring(ctx, 42, item.target);
  return {paint:result ? entries : null,created};
});
process.stdout.write(JSON.stringify(results));

#!/usr/bin/env node
// Compare pure grammar helpers without executing reference scene operations.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {stripTypeScriptTypes} from 'node:module';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const reference = path.resolve(process.argv[2] ?? process.env.CUEMOL2_REF_DIR ?? path.join(root, '../cuemol2'));
const source = path.join(reference, 'tritium/react-gui/src/plugins/chimerax-console/worker');
const output = path.join(root, '.cache/chimerax-oracle');
fs.mkdirSync(output, {recursive:true});
const visited = new Set();
function compile(relative) {
  const target = path.join(output, relative.replace(/\.ts$/, '.mjs'));
  if (visited.has(relative)) return target;
  visited.add(relative);
  let code = stripTypeScriptTypes(fs.readFileSync(path.join(source, relative), 'utf8'), {mode:'transform',sourceMap:false});
  code = code.replace(/import\s+\{([^}]+)\}\s+from\s+['"](@[^'"]+)['"];?/g, (_, names, module) =>
    names.split(',').map(name => {
      const local = name.trim().split(/\s+as\s+/).at(-1);
      return `const ${local} = () => { throw new Error(${JSON.stringify(`Reference scene operation unavailable: ${module}.${local}`)}); };`;
    }).join('\n'));
  code = code.replace(/(from\s+|import\s*)(['"])(\.[^'"]+)\2/g, (_, prefix, quote, imported) => {
    let dependency = path.normalize(path.join(path.dirname(relative), imported));
    if (!dependency.endsWith('.ts')) dependency += '.ts';
    if (dependency.startsWith('..')) throw new Error(`Unexpected reference dependency: ${dependency}`);
    const compiled = compile(dependency);
    let destination = path.relative(path.dirname(target), compiled).replaceAll(path.sep, '/');
    if (!destination.startsWith('.')) destination = `./${destination}`;
    return `${prefix}${quote}${destination}${quote}`;
  });
  if (/(?:from\s+|import\s*)['"]@/.test(code)) throw new Error(`Unstubbed import in ${relative}`);
  fs.mkdirSync(path.dirname(target), {recursive:true});
  fs.writeFileSync(target, code);
  return target;
}
const load = relative => import(pathToFileURL(compile(relative)).href);
const {CHX_COMMANDS} = await load('commands/registry.ts');
const {bindLine, resolveWords, splitLines} = await load('cli/syntax.ts');
const {readColor} = await load('colors.ts');
const {chainColor,modelColor,readPalette,paletteColors,rainbowRanges} = await load('colorSchemes.ts');
const {g5} = await load('camera.ts');
const {autoStyleOf} = await load('autoStyle.ts');
const {alphaOf} = await load('commands/reprCommands.ts');
const {parseSpec} = await load('spec/parse.ts');
const {emitFor} = await load('spec/emit.ts');
const {withSceneSelectors, isSelector} = await load('spec/selectors.ts');
const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const results = input.map(item => withSceneSelectors(new Set(['site']), () => {
  try {
    switch (item.kind) {
      case 'catalog': return CHX_COMMANDS.map(c=>({name:c.name,summary:c.summary,args:c.args.map(a=>({name:a.name,role:a.role}))}));
      case 'autoStyle': return autoStyleOf(item.size);
      case 'alpha': return alphaOf(Number(item.text));
      case 'split': return splitLines(item.text).map(line => line.text);
      case 'color': return readColor(item.text);
      case 'chain': return chainColor(item.text);
      case 'model': return modelColor(Number(item.text));
      case 'precision': return g5(Number(item.text));
      case 'palette': {
        const result = readPalette(item.text);
        if (!result.ok) return {error:result.error};
        const residues = Array.from({length:130},(_,i)=>String(i+1));
        return {stops:result.palette.stops,colors:paletteColors(result.palette,7),ranges:rainbowRanges(residues,result.palette)};
      }
      case 'bind': {
        const resolved = resolveWords(item.text, CHX_COMMANDS);
        if ('error' in resolved) return {error:resolved.error};
        const bound = bindLine(resolved.spec,item.text);
        return bound.kind === 'error' ? {error:bound.error} : {name:resolved.spec.name,args:bound.args};
      }
      case 'spec': {
        const parsed = parseSpec(item.text,isSelector);
        if (!parsed) return null;
        const context = {
          models:[{model:1,name:'first'},{model:2,name:'second'}],
          currentSelection:()=>'none',namedSelections:new Set(['site']),
          chainsHit:()=>{throw new Error('chainsHit requires a scene oracle');},
          anyHit:()=>{throw new Error('anyHit requires a scene oracle');},
        };
        return {consumed:parsed.consumed,expressions:[1,2].map(model=>emitFor(parsed.node,model,context))};
      }
      default: throw new Error(`Unknown case: ${item.kind}`);
    }
  } catch (error) { return {error:error.message,pos:error.pos??0}; }
}));
process.stdout.write(JSON.stringify(results));

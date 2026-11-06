#!/usr/bin/env node
// Check catalog coverage without invoking reference scene operations.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {stripTypeScriptTypes} from 'node:module';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const referenceRoot = path.resolve(process.argv[2] ?? process.env.CUEMOL2_REF_DIR ?? path.join(root, '../cuemol2'));
const source = path.join(referenceRoot, 'tritium/react-gui/src');
const base = 'renderer/worker/server/catalog';
const output = path.join(root, '.cache/catalog-reference');
const visited = new Set();
function compile(relative) {
  const target = path.join(output, relative.replace(/\.ts$/, '.mjs'));
  if (visited.has(relative)) return target;
  visited.add(relative);
  let code = stripTypeScriptTypes(fs.readFileSync(path.join(source, relative), 'utf8'), {mode:'transform',sourceMap:false});
  if (relative === `${base}/index.ts`) code = code.replace(/^export\s+\{[^}]+\}\s+from\s+['"][^'"]+['"];?\s*$/gm, '');
  code = code.replace(/import\s+\{([^}]+)\}\s+from\s+['"](@[^'"]+)['"];?/g, (whole, names, module) => {
    if (['@renderer/worker/shared/apbsTypes','@renderer/worker/shared/superposeTypes'].includes(module)) return whole;
    return names.split(',').map(name => {
      const local = name.trim().split(/\s+as\s+/).at(-1);
      return `const ${local} = () => { throw new Error(${JSON.stringify(`Reference runtime unavailable: ${module}.${local}`)}); };`;
    }).join('\n');
  });
  code = code.replace(/(from\s+|import\s*)(['"])([.@][^'"]+)\2/g, (_, prefix, quote, imported) => {
    let dependency = imported.startsWith('@') ? imported.slice(1) : path.normalize(path.join(path.dirname(relative), imported));
    if (!dependency.endsWith('.ts')) dependency += '.ts';
    if (dependency.startsWith('..')) throw new Error(`Unexpected dependency: ${dependency}`);
    const compiled = compile(dependency);
    let destination = path.relative(path.dirname(target), compiled).replaceAll(path.sep, '/');
    if (!destination.startsWith('.')) destination = `./${destination}`;
    return `${prefix}${quote}${destination}${quote}`;
  });
  fs.mkdirSync(path.dirname(target), {recursive:true});
  fs.writeFileSync(target, code);
  return target;
}
const {OPS, MCP_OPS} = await import(pathToFileURL(compile(`${base}/index.ts`)).href);
const {toolSchema} = await import(pathToFileURL(compile(`${base}/toolSchema.ts`)).href);
const reference = OPS.map(op => ({name:op.name,params:op.params,expose:op.expose,aliases:op.aliases??[],inputSchema:toolSchema(op)}));
fs.writeFileSync(path.join(output,'reference.json'),JSON.stringify(reference,null,2));
const ours = JSON.parse(fs.readFileSync(path.join(root,'crates/cuemol-command/src/tools.json'),'utf8'));
const consoleCatalog = JSON.parse(fs.readFileSync(path.join(root,'crates/cuemol-command/src/console/catalog.json'),'utf8'));
function includesSchema(actual,expected) {
  const types = schema => [schema.type].flat();
  if (!types(expected).every(type=>types(actual).includes(type))) return false;
  if (actual.enum && !expected.enum?.every(value=>actual.enum.includes(value))) return false;
  if (expected.items && (!actual.items || !includesSchema(actual.items,expected.items))) return false;
  return true;
}
const differences=[];
let parameters=0;
for(const op of MCP_OPS) {
  const local=ours.find(t=>t.name===op.name);
  if(!local) {differences.push(`${op.name}: missing MCP operation`); continue;}
  for(const [key,expected] of Object.entries(toolSchema(op).properties)) {
    parameters++;
    const actual=local.inputSchema.properties[key];
    if(!actual) differences.push(`${op.name}.${key}: missing parameter`);
    else if(!includesSchema(actual,expected)) differences.push(`${op.name}.${key}: incompatible schema`);
  }
}
let consoleNames=0;
for(const op of OPS.filter(op=>op.expose.console)) {
  for(const name of [op.name,...(op.aliases??[]).map(alias=>alias.name)]) {
    consoleNames++;
    if(!consoleCatalog[name]) differences.push(`${name}: missing console command`);
    else if(consoleCatalog[name].tool!==op.name) differences.push(`${name}: expected console operation ${op.name}, got ${consoleCatalog[name].tool}`);
  }
}
console.log(JSON.stringify({referenceOperations:OPS.length,mcpOperations:MCP_OPS.length,localOperations:ours.length,parameters,consoleNames,differences},null,2));
if(differences.length) process.exitCode=1;

#!/usr/bin/env node
// Read-only comparison of the reference time-chain validation.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {stripTypeScriptTypes} from 'node:module';
const root=path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const reference=path.resolve(process.argv[2]??process.env.CUEMOL2_REF_DIR??path.join(root,'../cuemol2'));
const source=path.join(reference,'tritium/react-gui/src/renderer');
const output=path.join(root,'.cache/animation-oracle');
fs.mkdirSync(output,{recursive:true});
for(const [input,name] of [['worker/shared/result.ts','result'],['worker/server/services/anim/timeRefGraph.ts','graph']]) {
  const code=stripTypeScriptTypes(fs.readFileSync(path.join(source,input),'utf8'),{mode:'transform',sourceMap:false}).replaceAll('@renderer/worker/shared/result','./result.mjs');
  if(/from\s*['"]@/.test(code))throw new Error(`Unexpected runtime import in ${input}`);
  fs.writeFileSync(path.join(output,`${name}.mjs`),code);
}
const {buildTimeRefGraph,checkTimeRef,checkName,describeResolveFailure}=await import(pathToFileURL(path.join(output,'graph.mjs')).href);
const normalize=result=>result.ok?result:{error:result.error};
const cases=JSON.parse(fs.readFileSync(0,'utf8'));
process.stdout.write(JSON.stringify(cases.map(item=>{
  const graph=buildTimeRefGraph(item.nodes);
  return {nodes:graph.nodes,error:describeResolveFailure(graph),references:item.nodes.map(node=>item.references.map(reference=>normalize(checkTimeRef(graph,node.uid,reference)))),names:item.nodes.map(node=>item.names.map(name=>normalize(checkName(graph,node.uid,name))))};
})));

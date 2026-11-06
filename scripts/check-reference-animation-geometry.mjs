#!/usr/bin/env node
// Read-only comparison with the reference animation timeline geometry.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {stripTypeScriptTypes} from 'node:module';
const root=path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const reference=path.resolve(process.argv[2]??process.env.CUEMOL2_REF_DIR??path.join(root,'../cuemol2'));
const source=path.join(reference,'tritium/react-gui/src/renderer/features/animation/anim/timelineGeometry.ts');
const output=path.join(root,'.cache/animation-oracle/geometry.mjs');
fs.mkdirSync(path.dirname(output),{recursive:true});
fs.writeFileSync(output,stripTypeScriptTypes(fs.readFileSync(source,'utf8'),{mode:'transform',sourceMap:false}));
const geometry=await import(pathToFileURL(output).href);
const cases=JSON.parse(fs.readFileSync(0,'utf8'));
process.stdout.write(JSON.stringify(cases.map(c=>{
  const scale=geometry.clampPxPerMs(c.scale);
  const tick=geometry.niceTickStepMs(scale);
  return {scale,fit:geometry.fitPxPerMs(c.length,c.width),width:geometry.timelineWidthPx(c.length,scale),tick,label:geometry.formatTimeLabel(tick*c.index,tick),clock:geometry.formatClock(c.time)};
})));

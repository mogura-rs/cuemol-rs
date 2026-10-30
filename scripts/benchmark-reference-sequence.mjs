import path from 'node:path';
import {existsSync} from 'node:fs';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
const [root,fixture,framesText='60']=process.argv.slice(2);
const frames=Number(framesText);
const resources=existsSync(path.join(root,'Contents/Resources'))?path.join(root,'Contents/Resources'):path.join(root,'resources');
const require=createRequire(import.meta.url);
const cm=require(path.join(resources,'app/node_modules/@cuemol/core/src/index.cjs')).getModule();
cm.initCueMol(path.join(resources,'cuemol2/share/sysconfig.xml'));
const manager=cm.getService('StreamManager');
const reader=manager.invokeMethod('createHandler','pdb',0);
reader.invokeMethod('setPath',path.resolve(fixture));
const mol=reader.invokeMethod('createDefaultObj');
reader.invokeMethod('attach',mol);reader.invokeMethod('read');reader.invokeMethod('detach');
const sel=cm.createObj('SelCommand');
sel.invokeMethod('compile','resid 1:10',0);
mol.setProp('sel',sel);
function rows(){
 return JSON.parse(mol.invokeMethod('getChainsJSON')).map(chainName=>({chain:chainName,residues:JSON.parse(mol.invokeMethod('getChain',chainName).invokeMethod('getResidsJSON'))}));
}
let start=performance.now();
const first=rows(),firstMs=performance.now()-start;
const digest=value=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
const firstDigest=digest(first);
start=performance.now();
let result;
for(let i=0;i<frames;i++)result=rows();
const elapsed=performance.now()-start;
if(digest(result)!==firstDigest)throw new Error('Sequence data changed between repeated reads');
let hash=0xcbf29ce484222325n;
for(const row of first)for(const residue of row.residues)for(const field of [row.chain,residue.index,residue.name,residue.single||'*',residue.sel?'1':'0']) {
 for(const byte of [...Buffer.from(field),0])hash=BigInt.asUintN(64,(hash^BigInt(byte))*0x100000001b3n);
}
console.log('SEQUENCE_RESULT '+JSON.stringify({digest:hash.toString(16).padStart(16,'0'),frames,first_ms:firstMs,elapsed_ms:elapsed,ms_per_frame:elapsed/frames,peak_rss_kib:process.resourceUsage().maxRSS,residues:first.reduce((n,r)=>n+r.residues.length,0),selected:first.reduce((n,r)=>n+r.residues.filter(x=>x.sel).length,0),sha256:firstDigest,rows:first.length}));

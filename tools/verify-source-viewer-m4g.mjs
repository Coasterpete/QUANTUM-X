// Isolated headless Edge only. Never attaches to an active browser/desktop.
import {spawnSync} from 'node:child_process';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {pathToFileURL} from 'node:url';
const page=resolve(process.argv[2]),output=dirname(page);
const executable=process.argv[3] || [
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
].find(existsSync);
if(!executable)throw Error('Pass the installed Chromium/Edge executable path.');
function run(mode,query='?selftest=1'){
  const args=['--headless','--disable-gpu','--no-first-run','--no-default-browser-check',
    '--disable-background-networking','--disable-extensions','--hide-scrollbars',
    '--window-size=1600,1400','--force-device-scale-factor=1','--virtual-time-budget=2200',
    '--user-data-dir='+join(output,'headless-m4g-'+mode),
    mode==='dom'?'--dump-dom':'--screenshot='+join(output,mode==='image'?'source-guidance-m4g.png':`source-guidance-m4g-${mode}.png`),pathToFileURL(page).href+query];
  const result=spawnSync(executable,args,{windowsHide:true,timeout:60000,maxBuffer:50_000_000,encoding:'utf8'});
  if(result.error||result.status!==0)throw Error(String(result.error||result.stderr).slice(-3000));
  return result.stdout;
}
const dom=run('dom');
writeFileSync(join(output,'stage-c-viewer-dom.html'),dom,'utf8');
if(!dom.includes('data-self-test="passed"')){
  throw Error('Actual controls: '+(dom.match(/data-self-test="[^"]*"/)?.[0]||'self-test did not complete'));
}
const svg=dom.match(/<svg\b[^>]*id="view"[\s\S]*?<\/svg>/)?.[0]||'';
if([...svg.matchAll(/<polygon /g)].length!==66)throw Error('Expected 66 actual body faces.');
if(!svg.includes('data-body="3"'))throw Error('Shared middle bogie missing.');
// Independent matrix-form quaternion transform, rather than the viewer's
// cross-product transform. Check every rendered face against tick 240 poses.
const data=JSON.parse(readFileSync(page.replace(/\.html$/,'.json'),'utf8'));
const clip=data.clips[0],dims={Car:[2.4,.65,1.1],Bogie:[.6,.16,1.25],Carrier:[.08,.08,1.3]};
function corners(body,id){const [px,py,pz,w,x,y,z]=body,d=dims[clip.scene.roles[id]],out=[];
  const matrix=[[1-2*(y*y+z*z),2*(x*y-w*z),2*(x*z+w*y)],
    [2*(x*y+w*z),1-2*(x*x+z*z),2*(y*z-w*x)],
    [2*(x*z-w*y),2*(y*z+w*x),1-2*(x*x+y*y)]];
  for(const a of [-.5,.5])for(const b of [-.5,.5])for(const c of [-.5,.5]){
    const v=[a*d[0],b*d[1],c*d[2]];out.push(matrix.map((r,i)=>[px,py,pz][i]+r.reduce((s,k,j)=>s+k*v[j],0)));}
  return out;
}
const low=[Infinity,Infinity,Infinity],high=[-Infinity,-Infinity,-Infinity];
for(const frame of clip.frames)frame.bodies.forEach((b,i)=>corners(b,i).forEach(p=>p.forEach((v,j)=>{low[j]=Math.min(low[j],v);high[j]=Math.max(high[j],v);})));
const center=low.map((v,i)=>(v+high[i])/2),scale=440/Math.hypot(...high.map((v,i)=>v-low[i]));
function projected(p){const [x,y,z]=p.map((v,i)=>v-center[i]),a=35*Math.PI/180,b=25*Math.PI/180;
  return [500+scale*(Math.cos(a)*x+Math.sin(a)*z),280-scale*(Math.cos(b)*y-Math.sin(b)*(-Math.sin(a)*x+Math.cos(a)*z))];}
const faces=[[0,1,3,2],[4,6,7,5],[0,4,5,1],[2,3,7,6],[0,2,6,4],[1,5,7,3]];
const recordedFaces=[...svg.matchAll(/<polygon[^>]*data-body="(\d+)"[^>]*points="([^"]+)"/g)];
if(recordedFaces.length!==66)throw Error('Could not inspect all recorded faces.');
for(const match of recordedFaces){
  const id=+match[1],actual=match[2].trim().split(/[ ,]+/).map(Number),vertices=corners(clip.frames[240].bodies[id],id);
  if(!faces.some(face=>face.flatMap(i=>projected(vertices[i])).every((v,i)=>Math.abs(v-actual[i])<.001)))throw Error(`Body ${id} does not match recorded pose/dimensions.`);
}
run('image');
run('foundation-top','?snapshot=foundation-top');
run('shared-bogie','?snapshot=shared-bogie');
const png=readFileSync(join(output,'source-guidance-m4g.png'));
if(png.subarray(1,4).toString()!=='PNG'||png.readUInt32BE(16)!==1600||png.readUInt32BE(20)!==1400)throw Error('Invalid screenshot.');
console.log('PASS: isolated headless controls: two clips, timestamps, play/pause/reset/speed, scrubbing, camera presets/pan/zoom/rotation, body isolation, overlays, histories; 66 faces independently match recorded quaternion poses/dimensions; PNG 1600x1400.');

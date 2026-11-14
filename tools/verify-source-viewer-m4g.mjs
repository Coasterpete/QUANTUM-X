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
run('image');
run('foundation-top','?snapshot=foundation-top');
run('shared-bogie','?snapshot=shared-bogie');
const png=readFileSync(join(output,'source-guidance-m4g.png'));
if(png.subarray(1,4).toString()!=='PNG'||png.readUInt32BE(16)!==1600||png.readUInt32BE(20)!==1400)throw Error('Invalid screenshot.');
console.log('PASS: isolated headless controls: two clips, timestamps, play/pause/reset/speed, scrubbing, camera presets/pan/zoom/rotation, body isolation, overlays, histories; 66 pose-derived faces; PNG 1600x1400.');

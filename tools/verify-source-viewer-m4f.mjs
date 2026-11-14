// Noninteractive headless verification only. Uses an isolated local profile and
// windowsHide; never connects to the owner's active browser or desktop.
import {spawnSync} from 'node:child_process';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {pathToFileURL} from 'node:url';
const page=resolve(process.argv[2]),output=dirname(page);
const executable=process.argv[3] || [
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
].find(existsSync);
if(!executable)throw Error('Pass the path to an installed Chromium/Edge executable as argument 2.');
const url=pathToFileURL(page).href+'?selftest=1';
function run(mode){
  const args=['--headless','--disable-gpu','--no-first-run','--no-default-browser-check',
    '--disable-background-networking','--disable-extensions','--hide-scrollbars',
    '--window-size=1500,1200','--force-device-scale-factor=1','--virtual-time-budget=1500',
    '--user-data-dir='+join(output,'headless-m4f-'+mode),
    mode==='dom'?'--dump-dom':'--screenshot='+join(output,'source-guidance-m4f.png'),url];
  const result=spawnSync(executable,args,{windowsHide:true,timeout:30000,maxBuffer:10_000_000,encoding:'utf8'});
  if(result.error || result.status!==0)throw Error(String(result.error||result.stderr).slice(-3000));
  return result.stdout;
}
const dom=run('dom');
writeFileSync(join(output,'stage-c-viewer-dom.html'),dom,'utf8');
if(!dom.includes('data-self-test="passed"'))throw Error('Actual control self-test did not complete.');
const svg=dom.match(/<svg\b[\s\S]*?<\/svg>/)?.[0] || '';
if([...svg.matchAll(/<polygon /g)].length!==66)throw Error('Expected 66 actual body faces.');
run('image');
const png=readFileSync(join(output,'source-guidance-m4f.png'));
if(png.subarray(1,4).toString()!=='PNG' || png.readUInt32BE(16)!==1500 || png.readUInt32BE(20)!==1200)throw Error('Invalid screenshot.');
console.log('PASS: real headless controls (timeline, previous/next, replay/pause, camera/reset, normals), 11 body rows, 66 faces; PNG 1500x1200.');

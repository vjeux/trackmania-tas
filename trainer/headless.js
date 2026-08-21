/* Headless harness: boots the real page script against a stub DOM built from the
   real HTML's ids, drives frames, and plays the run synthetically. Catches
   wiring typos, exceptions in draw(), and judging behaviour end to end. */
const fs = require('fs');
const html = fs.readFileSync(__dirname + '/index.html','utf8');
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const IDS = new Set([...html.matchAll(/id="([^"]+)"/g)].map(m=>m[1]));

let calls = 0;
function ctxStub(){
  const target = {
    canvas:{width:1200,height:800},
    setTransform(){}, clearRect(){}, fillRect(){}, strokeRect(){}, beginPath(){}, moveTo(){}, lineTo(){},
    stroke(){}, fill(){}, arcTo(){}, closePath(){}, save(){}, restore(){}, arc(){}, rect(){}, clip(){},
    fillText(t){ if(t===undefined) throw new Error('fillText(undefined)'); calls++; },
    strokeText(){}, measureText(){ return {width:20}; },
    createLinearGradient(){ return { addColorStop(o,c){ if(typeof c!=='string') throw new Error('bad gradient stop '+c); } }; },
    createRadialGradient(){ return { addColorStop(){} }; },
  };
  return new Proxy(target,{ get:(o,k)=> k in o ? o[k] : undefined, set:(o,k,v)=>{
    if((k==='fillStyle'||k==='strokeStyle') && typeof v==='string' && /undefined|NaN/.test(v)) throw new Error('bad style '+String(v));
    o[k]=v; return true; } });
}
function el(id){
  const e = {
    id, style:{}, dataset:{}, children:[], textContent:'', innerHTML:'', value:'50', checked:false,
    classList:{ _s:new Set(), add(c){this._s.add(c)}, remove(c){this._s.delete(c)}, contains(c){return this._s.has(c)} },
    appendChild(c){ this.children.push(c); return c; },
    addEventListener(){}, removeEventListener(){},
    querySelectorAll(){ return []; },
    getBoundingClientRect(){ return {width:1200,height:760,left:0,top:0}; },
    setAttribute(){}, focus(){},
  };
  e.parentElement = { getBoundingClientRect:e.getBoundingClientRect, appendChild(){} };
  if (id==='cv') e.getContext = () => ctxStub();
  return e;
}
const cache = new Map();
function byId(id){
  if (!IDS.has(id)) throw new Error('page script asked for missing element id: #'+id);
  if (!cache.has(id)) cache.set(id, el(id));
  return cache.get(id);
}
const speedBtns = [0.15,0.25,0.5,0.75,1].map(v=>{ const e=el('spd'); e.dataset.speed=String(v); return e; });
global.document = {
  querySelector(sel){
    if (sel.startsWith('#')) return byId(sel.slice(1));
    throw new Error('unhandled querySelector '+sel);
  },
  querySelectorAll(sel){ if (sel==='[data-speed]') return speedBtns; return []; },
  getElementById: byId,
  createElement(){ return el('created'); },
  addEventListener(){},
};
const handlers = {};
global.addEventListener = (n,f)=>{ (handlers[n]=handlers[n]||[]).push(f); };
global.window = global;
global.ResizeObserver = class { constructor(cb){this.cb=cb;} observe(){} };
let rafCb = null;
global.requestAnimationFrame = cb => { rafCb = cb; return 1; };
global.devicePixelRatio = 2;

// boot
new Function(script)();
console.log('boot ok — no exception');

const key = (type, code, ts) => handlers[type].forEach(f=>f({ code, timeStamp:ts, repeat:false, preventDefault(){} }));
function frames(n, ms){ for(let i=0;i<n;i++){ t += ms; const cb=rafCb; rafCb=null; cb(t); } }
let t = 1000;


function laneRuns(){
  const rows = fs.readFileSync(__dirname + '/kb6323.csv','utf8').trim().split('\n').slice(1)
    .map(l=>l.split(',').map(Number)).map(([t,s,a,b])=>({t,s,a,b}));
  const on=[r=>r.s<0, r=>r.a===1, r=>r.b===1, r=>r.s>0];
  const notes=[];
  on.forEach((f,li)=>{ let st=null;
    for(const r of rows){ const v=f(r);
      if(v&&st===null) st=r.t;
      if(!v&&st!==null){ notes.push({lane:li,start:st,end:r.t,open:false}); st=null; } }
    if(st!==null) notes.push({lane:li,start:st,end:rows[rows.length-1].t+10,open:true}); });
  return notes;
}
const CODES=['ArrowLeft','ArrowUp','ArrowDown','ArrowRight'];
function playRun(offsetMs, label, demo){
  cache.forEach(e=>{ e.innerHTML=''; e.classList.remove('show'); });
  byId('cbDemo').onchange({target:{checked:!!demo}});
  byId('rgSpeed').oninput({target:{value:100}});           // 1.00x
  const edges=[];
  if(!demo) for(const n of laneRuns()){
    if(n.end<=0) continue;
    edges.push({t:n.start+offsetMs, code:CODES[n.lane], type:'keydown'});
    if(!n.open) edges.push({t:n.end+offsetMs, code:CODES[n.lane], type:'keyup'});
  }
  edges.sort((a,b)=>a.t-b.t);
  key('keydown','KeyR',t);                                  // (re)start
  frames(1,4);
  let tape=-2.3*1000, i=0;
  const STEP=4;
  while(tape < 7600 && i<100000){
    tape += STEP;                                           // speed 1.0 → tape ms == wall ms
    while(edges.length && edges[0].t<=tape){ const e=edges.shift(); key(e.type,e.code,t+STEP); }
    frames(1,STEP); i++;
  }
  const rh=byId('results').innerHTML;
  const grade=(rh.match(/gradebig[^>]*>\s*([A-Z+]+)/)||[])[1];
  const acc=(rh.match(/([\d.]+)%/)||[])[1];
  const perfect=(rh.match(/<b>(\d+\/\d+)<\/b><span>perfect/)||[])[1];
  const miss=(rh.match(/<b>(\d+)<\/b><span>missed/)||[])[1];
  const extra=(rh.match(/<b>(\d+)<\/b><span>inputs the tape never makes/)||[])[1];
  const mean=(rh.match(/<b>([^<]*)<\/b><span>mean abs error/)||[])[1];
  console.log(label.padEnd(26)+' grade='+grade+' acc='+acc+'% perfect='+perfect+' miss='+miss+' extra='+extra+' mean='+mean+' rows='+((rh.match(/<tr>/g)||[]).length-1));
  return rh;
}
console.log('\n— simulated runs (speed 1.00x) —');
playRun(0,'demo (the TAS itself)',true);
playRun(0,'player: exactly on tape');
playRun(12,'player: 12 ms late');
playRun(-35,'player: 35 ms early');
playRun(70,'player: 70 ms late');
playRun(400,'player: hopeless (400 ms)');
console.log('\nfillText calls total:',calls);

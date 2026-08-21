try{
document.title='D boot';
const CODES=['ArrowLeft','ArrowDown','ArrowRight'];
const csv=document.documentElement.innerHTML.split('const RAW_TAPE = String.raw`')[1].split('`;')[0].trim().split('\n');
const R=csv.slice(1).map(l=>l.split(',').map(Number)).map(([t,s,a,b])=>({t,s,a,b}));
function laneRuns(){const on=[r=>r.s<0,r=>r.b===1,r=>r.s>0],N=[];
 on.forEach((f,li)=>{let st=null;for(const r of R){const v=f(r);
  if(v&&st===null)st=r.t; if(!v&&st!==null){N.push({lane:li,start:st,end:r.t,open:false});st=null;}}
  if(st!==null)N.push({lane:li,start:st,end:R[R.length-1].t+10,open:true});});return N;}
const notes=laneRuns().filter(n=>n.end>0);
const K=(c,d)=>window.dispatchEvent(new KeyboardEvent(d?'keydown':'keyup',{code:c,bubbles:true,cancelable:true}));
const out=[];
function res(){const r=document.getElementById('results');if(!r.classList.contains('show'))return null;const h=r.innerHTML;
 return{g:(h.match(/gradebig[^>]*>\s*([A-Z+]+)/)||[])[1],acc:(h.match(/([\d.]+)%/)||[])[1],
  miss:(h.match(/<b>(\d+)<\/b><span>missed/)||[])[1],extra:(h.match(/<b>(\d+)<\/b><span>inputs the tape never makes/)||[])[1],
  perfect:(h.match(/<b>(\d+\/\d+)<\/b><span>perfect/)||[])[1],gas:(h.match(/<b>(held|LIFTED)<\/b>/)||[])[1]};}
function play(off,label){
 const sp=document.getElementById('rgSpeed');sp.value=100;sp.oninput({target:sp});
 document.getElementById('btnStart').click();
 window.__pump(1); K('ArrowUp',true);
 const q=[];for(const n of notes){q.push({t:n.start+off,c:CODES[n.lane],d:true});if(!n.open)q.push({t:n.end+off,c:CODES[n.lane],d:false});}
 q.sort((a,b)=>a.t-b.t);
 for(let i=0;i<4000;i++){
  const now=parseFloat(document.getElementById('hTime').textContent)*1000;
  while(q.length&&q[0].t<=now){const e=q.shift();K(e.c,e.d);}
  window.__pump(1);
  const r=res(); if(r){out.push(label+' g='+r.g+' acc='+r.acc+'% perfect='+r.perfect+' miss='+r.miss+' extra='+r.extra+' gas='+r.gas);return;}
 }
 out.push(label+' DID-NOT-FINISH');
}
window.__pump(2);
play(0,'on-tape'); play(15,'15ms-late'); play(60,'60ms-late'); play(-40,'40ms-early');
document.title='RESULT '+out.join(' | ');
}catch(e){document.title='ERR '+e.message;}

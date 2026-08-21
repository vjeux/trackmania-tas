// Tape analysis: derive state transitions + note structure straight from the CSV.
const fs = require('fs');
const raw = fs.readFileSync(__dirname + '/kb6323.csv', 'utf8');
const lines = raw.trim().split('\n');
const header = lines[0];
const rows = lines.slice(1).map(l => l.split(',').map(Number));
console.log('header:', header);
console.log('data rows:', rows.length, 'first', rows[0].join(','), 'last', rows[rows.length-1].join(','));

// tick spacing sanity
let gaps = new Set();
for (let i=1;i<rows.length;i++) gaps.add(rows[i][0]-rows[i-1][0]);
console.log('tick gaps present:', [...gaps]);
console.log('distinct steer values:', [...new Set(rows.map(r=>r[1]))]);
console.log('distinct accel:', [...new Set(rows.map(r=>r[2]))], 'distinct brake:', [...new Set(rows.map(r=>r[3]))]);

// transitions (any change in the (steer,accel,brake) triple)
const tr = [];
let prev = null;
for (const [t,s,a,b] of rows) {
  const key = s+'|'+a+'|'+b;
  if (key !== prev) { tr.push({t,s,a,b}); prev = key; }
}
console.log('\nALL transitions (including countdown):', tr.length);
for (const e of tr) console.log((e.t/1000).toFixed(3).padStart(8), 'steer='+String(e.s).padStart(4), 'accel='+e.a, 'brake='+e.b);

const fromZero = tr.filter(e=>e.t>=0);
console.log('\ntransitions at race_ms >= 0:', fromZero.length);

// state at race 0 (carry-in)
const atZero = rows.find(r=>r[0]===0);
console.log('state at 0.000:', atZero && atZero.join(','));

// vjeux's table, for a machine diff
const table = `0.000 -127 1 0
0.750 -127 1 1
0.880 -127 1 0
1.450 127 1 0
1.970 0 1 0
2.090 -127 1 0
2.250 0 1 0
2.300 -127 1 0
2.330 0 1 0
2.360 -127 1 0
2.370 0 1 0
2.380 -127 1 0
2.510 0 1 0
2.710 127 1 0
2.790 -127 1 0
3.110 0 1 0
3.130 127 1 0
3.340 0 1 0
3.350 -127 1 0
3.480 0 1 0
3.500 -127 1 0
3.570 0 1 0
3.590 -127 1 0
5.110 0 1 0
5.230 127 1 0`.split('\n').map(l=>{const [t,s,a,b]=l.trim().split(/\s+/).map(Number);return {t:Math.round(t*1000),s,a,b};});

console.log('\nDIFF vs vjeux table (parser rows:', fromZero.length, 'table rows:', table.length, ')');
let bad=0;
const n=Math.max(table.length, fromZero.length);
for (let i=0;i<n;i++){
  const p=fromZero[i], q=table[i];
  const ps=p?`${(p.t/1000).toFixed(3)} ${p.s} ${p.a} ${p.b}`:'(none)';
  const qs=q?`${(q.t/1000).toFixed(3)} ${q.s} ${q.a} ${q.b}`:'(none)';
  if (ps!==qs){ bad++; console.log(`  row ${i}: parser[${ps}] table[${qs}]`); }
}
console.log(bad? `MISMATCHES: ${bad}` : 'EXACT MATCH on all rows');

// Note extraction: contiguous runs where a control is engaged
function notes(pred){
  const out=[]; let start=null;
  for (const [t,s,a,b] of rows){ const on=pred(s,a,b);
    if(on && start===null) start=t; if(!on && start!==null){ out.push([start,t]); start=null; } }
  if (start!==null) out.push([start, rows[rows.length-1][0]+10]);
  return out;
}
const lanes = {
  LEFT: notes((s)=>s<0), RIGHT: notes((s)=>s>0),
  ACCEL: notes((s,a)=>a===1), BRAKE: notes((s,a,b)=>b===1),
};
console.log('\nNOTES (lane: start -> end, duration s)');
let total=0;
for (const [k,v] of Object.entries(lanes)){
  console.log(` ${k} (${v.length})`);
  for (const [a,b] of v) console.log(`   ${(a/1000).toFixed(3)} -> ${(b/1000).toFixed(3)}  dur ${((b-a)/1000).toFixed(3)}`);
  total+=v.length;
}
console.log('total notes:', total);

// burst analysis 2.2 - 3.6
console.log('\nBURST 2.090-3.600 steer segments (value, start, dur):');
let segs=[]; let cs=null;
for (const [t,s] of rows){ if(!cs||cs.s!==s){ if(cs) cs.end=t, segs.push(cs); cs={s,start:t}; } }
cs.end=rows[rows.length-1][0]+10; segs.push(cs);
for (const g of segs) if (g.end>2000 && g.start<3700) console.log(`   steer=${String(g.s).padStart(4)}  ${(g.start/1000).toFixed(3)} -> ${(g.end/1000).toFixed(3)}  dur ${((g.end-g.start)/1000).toFixed(3)}`);

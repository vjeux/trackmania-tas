// Verify the page's own logic, by executing the script block out of index.html.
const fs = require('fs');
const html = fs.readFileSync(__dirname + '/index.html','utf8');
const m = html.match(/<script>([\s\S]*?)<\/script>/);
if(!m) throw new Error('no script block');
const body = m[1] + '\n;return {parseTape,buildNotes,notesToEvents,transitions,steerSegments,matchEdge,gradeOf,scoreOf,WIN,LANES,ROWS,NOTES,TRANS,RACE_SEGS,TAPE_START,TAPE_END,FINISH_MS,SECTIONS,RAW_TAPE,GAS,GAS_NOTES,GAS_DOWN_AT,GAS_LIFTED_IN_RACE};';
const api = new Function(body)();
const fail = [];
const ok = (c,msg)=>{ console.log((c?'  ok   ':'  FAIL ')+msg); if(!c) fail.push(msg); };

console.log('— tape —');
ok(api.ROWS.length===793, 'parsed 793 data ticks (got '+api.ROWS.length+')');
ok(api.RAW_TAPE.trim().split('\n').length===794, 'embedded CSV is 794 lines incl. header');
ok(api.ROWS[0].t===-1560 && api.ROWS[api.ROWS.length-1].t===6360, 'spans -1.560 → 6.360');
ok([...new Set(api.ROWS.map(r=>r.steer))].sort((a,b)=>a-b).join()==='-127,0,127','three steer values only');
let gaps=new Set(); for(let i=1;i<api.ROWS.length;i++) gaps.add(api.ROWS[i].t-api.ROWS[i-1].t);
ok([...gaps].join()==='10','uniform 10 ms ticks');

console.log('— transitions vs vjeux table —');
const table=`0.000 -127 1 0
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
5.230 127 1 0`.split('\n').map(l=>l.trim().split(/\s+/).map(Number));
// the page's transition list, restricted to the race, with the carried-in state at 0.000 prepended
const atZero = api.ROWS.find(r=>r.t===0);
const mine = [[0,atZero.steer,atZero.accel,atZero.brake]].concat(
  api.TRANS.filter(r=>r.t>0).map(r=>[r.t/1000,r.steer,r.accel,r.brake]));
ok(mine.length===table.length, 'same row count ('+mine.length+' vs '+table.length+')');
let diffs=0;
for(let i=0;i<Math.max(mine.length,table.length);i++){
  const a=(mine[i]||[]).join(' '), b=(table[i]||[]).join(' ');
  if(a!==b){ diffs++; console.log('    row '+i+': parser['+a+'] table['+b+']'); }
}
ok(diffs===0,'every race transition matches the table exactly');
ok(api.RACE_SEGS.length===23,'23 steer segments after 0.000 (got '+api.RACE_SEGS.length+') — the "23 events"');

console.log('— notes —');
const N=api.NOTES, L=api.LANES;
const byLane = i => N.filter(n=>n.lane===i);
console.log('  lanes: left '+byLane(0).length+', brake '+byLane(1).length+', right '+byLane(2).length+', gas(state) '+api.GAS_NOTES.length);
const race = N.filter(n=>n.end>0);
ok(race.length===14,'14 notes touch the race (got '+race.length+')');
ok(N.filter(n=>n.lane===1 && n.start>=0).length===1,'exactly one brake note after the line');
const flick = N.find(n=>n.dur===10);
ok(!!flick && flick.start===2360 && flick.lane===0,'the 10 ms left flick at 2.360 survives note-building');
const longleft = N.find(n=>n.lane===0 && n.dur===1520);
ok(!!longleft && longleft.start===3590,'the 1.520 s left hold starts at 3.590');
ok(api.GAS_NOTES.some(n=>n.open && n.start===-270),'gas is one open state from -0.270 to the end');
ok(N.some(n=>n.lane===2 && n.open && n.start===5230),'final right is open to the finish');
ok(!api.ROWS.some(r=>r.t>=0 && r.t<=6323 && r.accel===0),'gas is never released between the line and 6.323');

console.log('— events / judging —');
const evRace = api.notesToEvents(N,false);
const evFull = api.notesToEvents(N,true);
console.log('  race-mode edges: '+evRace.length+'  full-tape edges: '+evFull.length);
ok(evRace.every(e=>e.t>=-60),'race mode keeps only the carry-in steer and later');
ok(!evRace.some(e=>api.LANES[e.lane].id==='gas'),'no gas edge is ever judged');
ok(evRace.filter(e=>e.type==='press').length===evRace.filter(e=>e.type==='release').length+1,
   'one note (the final right) is open-ended, so 1 more press than releases');
// judging behaviour
const evs = api.notesToEvents(N,false).map(e=>Object.assign({},e,{judged:false}));
const target = evs.find(e=>e.t===2360 && e.type==='press');
ok(api.matchEdge(evs,0,'press',2372)===target,'a press 12 ms late is credited to the 2.360 flick, not the nearer 2.380');
target.judged = true;
const nxt = evs.find(e=>e.t===2380 && e.type==='press');
ok(api.matchEdge(evs,0,'press',2385)===nxt,'the following press then lands on 2.380');
target.judged = false;
const p2300 = evs.find(e=>e.t===2300 && e.type==='press');
ok(api.matchEdge(evs,0,'press',2312)===p2300,'a press 12 ms late in sequence takes the earliest open edge (2.300)');
p2300.judged = true;
ok(api.matchEdge(evs,0,'press',2402)===target,'42 ms overdue still belongs to the open 2.360 edge');
target.judged = true;
ok(api.matchEdge(evs,0,'press',2402)===nxt,'with 2.360 played, 2.380 takes the next press');
ok(api.gradeOf(12).name==='PERFECT' && api.gradeOf(30).name==='GREAT' && api.gradeOf(200)===null,'window tiers');
ok(api.matchEdge(evs,1,'press',4000)===null,'a brake press at 4.000 matches nothing (spurious)');

console.log('— page shell —');
ok(html.includes('<canvas id="cv">'),'canvas present');
ok(!/\bsrc=|\bhref="http/.test(html),'no external resources — opens with no network');
ok(html.includes('6.323')&&html.includes('6.343')&&html.includes('6.346'),'the three times are on the page');
ok(!/\b\d{4} ms\b/.test(html.replace(/[+−]\d+ ms/g,'')),'no bare 4-digit millisecond times in the copy');

console.log(fail.length? '\nFAILURES: '+fail.length : '\nALL CHECKS PASS');
process.exit(fail.length?1:0);

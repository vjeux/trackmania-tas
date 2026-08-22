// pagecheck.js -- run a built tmsite page's own JavaScript, headless.
//
// The point is stronger than "the HTML is well formed": it extracts the page's
// <script>, runs it against a stub DOM/canvas, and reports what the page's own
// code computed from its own payload. If the data payload were invalid JSON
// (CPython's bare NaN/Infinity being the classic trap), or the geometry were
// malformed, this throws instead of printing numbers.
//
//   node pagecheck.js <page.html>

const fs = require('fs');
const vm = require('vm');

const file = process.argv[2];
const html = fs.readFileSync(file, 'utf8');
const m = html.match(/<script>([\s\S]*?)<\/script>/);
if (!m) { console.error('no <script> in ' + file); process.exit(1); }
const src = m[1];

const calls = {};
const bump = (k) => { calls[k] = (calls[k] || 0) + 1; };

function makeCtx() {
  return new Proxy({}, {
    get(t, p) {
      if (p === 'canvas') return {};
      return (...a) => { bump(String(p)); if (p === 'measureText') return { width: 8 }; };
    },
    set() { return true; },
  });
}

function makeEl(tag) {
  const el = {
    tagName: tag, style: {}, className: '', innerHTML: '', textContent: '',
    value: 0, children: [], dataset: {},
    getContext: () => makeCtx(),
    getBoundingClientRect: () => ({ width: 1280, height: 800, left: 0, top: 0 }),
    addEventListener: () => {},
    appendChild(c) { this.children.push(c); return c; },
  };
  return el;
}

const els = {};
const document = {
  getElementById(id) { if (!els[id]) els[id] = makeEl('div'); return els[id]; },
  createElement: (t) => makeEl(t),
};

const sandbox = {
  document,
  devicePixelRatio: 2,
  addEventListener: () => {},
  requestAnimationFrame: () => 0,
  atob: (s) => Buffer.from(s, 'base64').toString('binary'),
  console,
  Math, JSON, Array, Object, Number, String, Set, Map, Uint8Array, DataView, Buffer,
};
sandbox.window = sandbox;
sandbox.globalThis = sandbox;

vm.createContext(sandbox);
vm.runInContext(src + "\n;this.__RUNS=RUNS; this.__VMAX=VMAX; this.__CPS=CPS;", sandbox, { filename: file });

const RUNS = sandbox.__RUNS;
if (!Array.isArray(RUNS)) { console.error('page defined no RUNS array'); process.exit(1); }
let n = 0, xmn = 1e18, xmx = -1e18, ymn = 1e18, ymx = -1e18, zmn = 1e18, zmx = -1e18, vmx = -1e18;
let bad = 0;
for (const r of RUNS) {
  for (const p of r.p) {
    n++;
    for (const c of p) if (!Number.isFinite(c)) bad++;
    xmn = Math.min(xmn, p[0]); xmx = Math.max(xmx, p[0]);
    ymn = Math.min(ymn, p[1]); ymx = Math.max(ymx, p[1]);
    zmn = Math.min(zmn, p[2]); zmx = Math.max(zmx, p[2]);
    vmx = Math.max(vmx, p[3]);
  }
}
const fmt = (v) => v.toFixed(1);
console.log(file);
console.log('  bytes            ' + html.length);
console.log('  RUNS.length      ' + RUNS.length);
console.log('  samples          ' + n);
console.log('  non-finite       ' + bad);
console.log('  x range          ' + fmt(xmn) + ' .. ' + fmt(xmx));
console.log('  y range          ' + fmt(ymn) + ' .. ' + fmt(ymx));
console.log('  z range          ' + fmt(zmn) + ' .. ' + fmt(zmx));
console.log('  max speed        ' + fmt(vmx) + '  (legend VMAX ' + sandbox.__VMAX + ')');
console.log('  checkpoints      ' + Object.keys(sandbox.__CPS).join(', '));
console.log('  table rows built ' + (els['list'] || els['lst'] || { children: [] }).children.length);
console.log('  canvas ops       moveTo=' + (calls.moveTo | 0) + ' lineTo=' + (calls.lineTo | 0) +
            ' stroke=' + (calls.stroke | 0) + ' fillText=' + (calls.fillText | 0) +
            ' arc=' + (calls.arc | 0));
if (bad > 0) { console.error('FAILED: non-finite numbers in payload'); process.exit(1); }
if ((calls.stroke | 0) < 100) { console.error('FAILED: page drew almost nothing'); process.exit(1); }

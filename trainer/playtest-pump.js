// Deterministic frame pump: real canvas, real events, real DOM — but a clock we
// control, so a headless playtest is reproducible.
(function(){
  let t = 0;
  const q = [];
  window.requestAnimationFrame = cb => { q.push(cb); return q.length; };
  window.__pump = n => { for (let i=0;i<n;i++){ t += 16; const c = q.splice(0); c.forEach(f=>f(t)); } };
  const P = performance.now.bind(performance);
  performance.now = () => t;
})();

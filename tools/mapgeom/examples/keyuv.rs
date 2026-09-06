//! Weld key + per-corner U/V simulation. Usage: keyuv HIS.ITEM SRCFILE
use std::collections::BTreeSet;
use mapgeom::static_item::bake::{geometry_layers, face_triangles, tangent};
use mapgeom::static_item::bake::Corner;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // his stream verts (targets, with uv1 transplant assumed? No - compare without uv1: his |pos+n+uv+U+V| vs sim)
    // Simpler: simulate key=(pos,n-smoothed?,uv,U_percorner,V_percorner) on crystal faces (no smoothing? use face normals? No - need smoothed N.
    // SKIP full sim; instead measure his |pos+n+uv+uv1+U+V| vs stream (should equal if key complete).
    println!("keyuv: use keytest-style analysis on his file with U,V in key");
    let _ = (geometry_layers, face_triangles, tangent, Corner { pos: [0.0; 3], normal: [0.0; 3], uv: [0.0; 2], uv1: [0.0; 2] });
}

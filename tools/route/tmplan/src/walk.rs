//! Surface-FOLLOWING paths for the legs the up-facing grid cannot carry: wallrides, loops, inverted roads, the pool
//! surface, terrain hills (the converter's W / S / T verdicts on the tiny maps, 2026-09-08). The road grid is a
//! heightfield — one surface per XZ cell, gravity down — so a loop or a wall has no cell of its own. Here the graph is
//! the SURFACE SHELL itself, voxelised: every drivable collision triangle is sampled at ~0.6 m and each sample marks a
//! `cell`-metre voxel; two occupied voxels within one step (26-neighbourhood) are connected; Dijkstra from the voxels
//! around the from-gate to any voxel around the to-gate. "Up" rotates with the surface, so a loop is a road that
//! curls; items that only touch (no shared vertices) still connect through neighbouring voxels.
use mapgeom::local::LocalScene;
use std::collections::{BinaryHeap, HashMap};

pub struct SurfaceWalk {
    pub cell: f32,
    /// Occupied voxel → index into `centre`.
    pub index: HashMap<(i32, i32, i32), u32>,
    pub key: Vec<(i32, i32, i32)>,
}

impl SurfaceWalk {
    /// `drivable(physics id)` selects the triangles (road / deck / terrain / pool physics — walls of Metal or Concrete
    /// are drivable too: a wallride IS a wall).
    pub fn build(scene: &LocalScene, cell: f32, drivable: &dyn Fn(u8) -> bool) -> SurfaceWalk {
        let mut index: HashMap<(i32, i32, i32), u32> = HashMap::new();
        let mut key: Vec<(i32, i32, i32)> = Vec::new();
        let mut mark = |p: [f32; 3]| {
            let k = ((p[0] / cell).floor() as i32, (p[1] / cell).floor() as i32, (p[2] / cell).floor() as i32);
            if !index.contains_key(&k) {
                index.insert(k, key.len() as u32);
                key.push(k);
            }
        };
        let step = cell * 0.6;
        for t in &scene.tris {
            if !drivable(t.mat) {
                continue;
            }
            let (a, b, c) = (t.v[0], t.v[1], t.v[2]);
            let lab = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            let lac = ((c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2) + (c[2] - a[2]).powi(2)).sqrt();
            let n = ((lab.max(lac) / step).ceil() as usize).clamp(1, 64);
            // barycentric lattice over the triangle
            for i in 0..=n {
                for j in 0..=(n - i) {
                    let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                    let w = 1.0 - u - v;
                    mark([w * a[0] + u * b[0] + v * c[0], w * a[1] + u * b[1] + v * c[1], w * a[2] + u * b[2] + v * c[2]]);
                }
            }
        }
        SurfaceWalk { cell, index, key }
    }

    pub fn centre(&self, k: (i32, i32, i32)) -> [f32; 3] {
        [(k.0 as f32 + 0.5) * self.cell, (k.1 as f32 + 0.5) * self.cell, (k.2 as f32 + 0.5) * self.cell]
    }

    /// Occupied voxels within `r` m (XZ) and `dy` m (height) of `p`.
    pub fn near(&self, p: [f32; 3], r: f32, dy: f32) -> Vec<u32> {
        let mut out = Vec::new();
        let (rx, ry) = ((r / self.cell).ceil() as i32, (dy / self.cell).ceil() as i32);
        let c = ((p[0] / self.cell).floor() as i32, (p[1] / self.cell).floor() as i32, (p[2] / self.cell).floor() as i32);
        for dx in -rx..=rx {
            for dz in -rx..=rx {
                for dyy in -ry..=ry {
                    if let Some(&i) = self.index.get(&(c.0 + dx, c.1 + dyy, c.2 + dz)) {
                        let q = self.centre(self.key[i as usize]);
                        if ((q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2)).sqrt() <= r && (q[1] - p[1]).abs() <= dy {
                            out.push(i);
                        }
                    }
                }
            }
        }
        out
    }

    /// Shortest walk over the shell from any of `from` to any of `to`; None when disconnected or longer than
    /// `max_len`. Returns the voxel-centre polyline and its length.
    pub fn walk(&self, from: &[u32], to: &[u32], max_len: f32) -> Option<(Vec<[f32; 3]>, f32)> {
        if from.is_empty() || to.is_empty() {
            return None;
        }
        let mut dist: HashMap<u32, f32> = HashMap::new();
        let mut prev: HashMap<u32, u32> = HashMap::new();
        let mut is_to = vec![false; self.key.len()];
        for &t in to {
            is_to[t as usize] = true;
        }
        let mut heap: BinaryHeap<(std::cmp::Reverse<u32>, u32)> = BinaryHeap::new();
        for &f in from {
            dist.insert(f, 0.0);
            heap.push((std::cmp::Reverse(0u32), f));
        }
        let mut goal = None;
        while let Some((std::cmp::Reverse(db), u)) = heap.pop() {
            let d = f32::from_bits(db);
            if d > *dist.get(&u).unwrap_or(&f32::INFINITY) {
                continue;
            }
            if d > max_len {
                break;
            }
            if is_to[u as usize] {
                goal = Some(u);
                break;
            }
            let k = self.key[u as usize];
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        if let Some(&v) = self.index.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                            let w = self.cell * ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                            let nd = d + w;
                            if nd < *dist.get(&v).unwrap_or(&f32::INFINITY) {
                                dist.insert(v, nd);
                                prev.insert(v, u);
                                heap.push((std::cmp::Reverse(nd.to_bits()), v));
                            }
                        }
                    }
                }
            }
        }
        let g = goal?;
        let len = dist[&g];
        let mut path = vec![self.centre(self.key[g as usize])];
        let mut cur = g;
        while let Some(&p) = prev.get(&cur) {
            cur = p;
            path.push(self.centre(self.key[cur as usize]));
        }
        path.reverse();
        Some((path, len))
    }
}

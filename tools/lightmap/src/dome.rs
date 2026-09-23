//! The game's precomputed sphere point sets (`Techno\Media\PointsInSphere\Std.PointsInSphere.Gbx`,
//! class 0x09066000; RE child 2026-09-23): 135 sets of unit vectors, n = 4…132, 256, 512, 1032,
//! 2040, 4112, 8192. The lightmapper's sky cone = the points of a set inside the cone of
//! half-angle A around +y; using the same points reproduces its sampling pattern.

pub struct PointSets {
    pub sets: Vec<Vec<[f32; 3]>>,
}

impl PointSets {
    /// Parse the uncompressed .Gbx (header, then the 0x09066000 chunk: u32 count, count × {u32 n,
    /// u32 offset}, u32 total, total × float3).
    pub fn load(path: &str) -> Result<PointSets, String> {
        let d = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let u32_at = |o: usize| -> Option<u32> { d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
        // the body starts with the chunk id; find it after the header
        let mut o = 0;
        while o + 4 <= d.len() && u32_at(o) != Some(0x0906_6000) {
            o += 1;
        }
        // the first occurrence is the header's class id; the body chunk id follows the header
        let mut oc = o + 4;
        while oc + 4 <= d.len() && u32_at(oc) != Some(0x0906_6000) {
            oc += 1;
        }
        if oc + 8 > d.len() {
            return Err(format!("{path}: no 0x09066000 body chunk"));
        }
        let count = u32_at(oc + 4).ok_or("count")? as usize;
        let mut ranges = Vec::with_capacity(count);
        let mut p = oc + 8;
        for _ in 0..count {
            let n = u32_at(p).ok_or("n")? as usize;
            let off = u32_at(p + 4).ok_or("offset")? as usize;
            ranges.push((n, off));
            p += 8;
        }
        let total = u32_at(p).ok_or("total")? as usize;
        p += 4;
        if p + total * 12 > d.len() {
            return Err(format!("{path}: {total} vectors do not fit"));
        }
        let f32_at = |o: usize| f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        let pts: Vec<[f32; 3]> = (0..total).map(|i| [f32_at(p + i * 12), f32_at(p + i * 12 + 4), f32_at(p + i * 12 + 8)]).collect();
        let sets = ranges.iter().map(|&(n, off)| pts[off.min(total)..(off + n).min(total)].to_vec()).collect();
        Ok(PointSets { sets })
    }

    /// The set with exactly `n` points, else the smallest with at least `n` (else the largest).
    pub fn set(&self, n: usize) -> Option<&Vec<[f32; 3]>> {
        self.sets.iter().find(|s| s.len() == n).or_else(|| self.sets.iter().filter(|s| s.len() >= n).min_by_key(|s| s.len())).or_else(|| self.sets.iter().max_by_key(|s| s.len()))
    }

    /// The points of the `n`-set inside the cone of half-angle `deg` around +y.
    pub fn cone(&self, n: usize, deg: f32) -> Vec<[f32; 3]> {
        let c = deg.to_radians().cos();
        self.set(n).map(|s| s.iter().copied().filter(|p| p[1] >= c).collect()).unwrap_or_default()
    }
}

/// The banked copy's default location.
pub fn default_path() -> String {
    format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/client-re/Std.PointsInSphere.Gbx", std::env::var("HOME").unwrap_or_default())
}

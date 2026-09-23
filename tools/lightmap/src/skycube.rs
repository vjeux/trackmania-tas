//! The mood's HDR sky cube (`EnvCubicHdr.dds` / `AmbCubeP.dds`, BC6H) as a
//! radiance lookup for the tracer: D3D cube-face convention, nearest texel.

pub struct CubeMap {
    pub n: usize,
    /// Six faces (+X, −X, +Y, −Y, +Z, −Z), row-major, RGB.
    pub faces: Vec<Vec<[f32; 3]>>,
}

impl std::fmt::Debug for CubeMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CubeMap({}²)", self.n)
    }
}

impl CubeMap {
    pub fn load(path: &str) -> Result<CubeMap, String> {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let dds = crate::bc6h::parse_dds(&data)?;
        if !dds.cubemap || dds.w != dds.h {
            return Err(format!("{path}: not a cubemap ({}x{}, format {})", dds.w, dds.h, dds.format));
        }
        let faces = match dds.format {
            95 | 96 => {
                let mut face_bytes = 0usize;
                let (mut mw, mut mh) = (dds.w, dds.h);
                for _ in 0..dds.mips {
                    face_bytes += crate::bc6h::bc_mip_bytes(mw, mh);
                    mw = (mw / 2).max(1);
                    mh = (mh / 2).max(1);
                }
                (0..6).map(|f| crate::bc6h::decode_image(&dds.data[f * face_bytes..], dds.w, dds.h, dds.format == 96)).collect()
            }
            // D3DFMT_A16B16G16R16F (fourcc 113, the Stadium AmbCubeP): RGBA half floats, 8 B per texel
            113 => {
                let mut face_bytes = 0usize;
                let (mut mw, mut mh) = (dds.w, dds.h);
                for _ in 0..dds.mips {
                    face_bytes += mw * mh * 8;
                    mw = (mw / 2).max(1);
                    mh = (mh / 2).max(1);
                }
                (0..6)
                    .map(|f| {
                        let o = f * face_bytes;
                        (0..dds.w * dds.h)
                            .map(|i| {
                                let p = o + i * 8;
                                let h = |k: usize| u16::from_le_bytes([dds.data[p + 2 * k], dds.data[p + 2 * k + 1]]);
                                [crate::bc6h::half_to_f32(h(0)), crate::bc6h::half_to_f32(h(1)), crate::bc6h::half_to_f32(h(2))]
                            })
                            .collect()
                    })
                    .collect()
            }
            other => return Err(format!("{path}: unsupported cubemap format {other}")),
        };
        Ok(CubeMap { n: dds.w, faces })
    }

    /// Radiance in direction `d` (unit or not).
    pub fn sample(&self, d: [f32; 3]) -> [f32; 3] {
        let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
        // face, u, v (u right, v down in the face image), per the D3D convention
        let (face, u, v) = if ax >= ay && ax >= az {
            if d[0] > 0.0 { (0, -d[2] / ax, -d[1] / ax) } else { (1, d[2] / ax, -d[1] / ax) }
        } else if ay >= az {
            if d[1] > 0.0 { (2, d[0] / ay, d[2] / ay) } else { (3, d[0] / ay, -d[2] / ay) }
        } else if d[2] > 0.0 {
            (4, d[0] / az, -d[1] / az)
        } else {
            (5, -d[0] / az, -d[1] / az)
        };
        let n = self.n as f32;
        let x = (((u + 1.0) * 0.5 * n) as isize).clamp(0, self.n as isize - 1) as usize;
        let y = (((v + 1.0) * 0.5 * n) as isize).clamp(0, self.n as isize - 1) as usize;
        self.faces[face][y * self.n + x]
    }

    /// Cosine-weighted irradiance for normal `nrm`, integrated over the texels (for checks).
    pub fn irradiance(&self, nrm: [f32; 3]) -> [f64; 3] {
        let mut e = [0f64; 3];
        let n = self.n;
        for face in 0..6 {
            for y in 0..n {
                for x in 0..n {
                    let (u, v) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                    let d = match face { 0 => [1.0, -v, -u], 1 => [-1.0, -v, u], 2 => [u, 1.0, v], 3 => [u, -1.0, -v], 4 => [u, -v, 1.0], _ => [-u, -v, -1.0] };
                    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                    let inv = 1.0 / len2.sqrt();
                    let c = (d[0] * nrm[0] + d[1] * nrm[1] + d[2] * nrm[2]) * inv;
                    if c > 0.0 {
                        let dw = (4.0 / (n * n) as f64) / (len2 as f64).powf(1.5);
                        let p = self.faces[face][y * n + x];
                        for k in 0..3 {
                            e[k] += p[k] as f64 * c as f64 * dw;
                        }
                    }
                }
            }
        }
        e
    }
}

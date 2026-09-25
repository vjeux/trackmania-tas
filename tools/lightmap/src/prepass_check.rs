//! `lmtool prepass-check` — ROW 1's harness: the attribute pre-pass of a captured frame emulated draw by draw
//! (`prepass.rs`) and compared with every banked snapshot: the atlas after the material draws (`atlas_attr_K`),
//! the water-id map (`atlas_ids`), the atlas after the water tint (`setup_ps17018`), the accumulation target
//! after each run (`setup_ps1109`), and — with `--all-runs` — the nine runs (the six uncaptured ones derived
//! from the captured run by their raster offsets) summed against the frame's final 16963.

use crate::gpucmp::{compare_where, Fmt, Report};
use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::{load_entry, read_manifest, Buf};
use crate::passdump::{Entry, Manifest};
use crate::prepass::{self, DrawRec, Meshes, RunOpts, Target, Textures, WaterData, WaterDraw, H, W};
use crate::texsample::{self, Address, Bc1Decode, Sampler};
use std::path::{Path, PathBuf};

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
}

fn flag(a: &[String], k: &str) -> bool {
    a.iter().any(|x| x == k)
}

/// The class ids `prepass::class_of` assigns, with their names for the tables.
const CLASSES: [(u8, &str); 6] = [(1, "tiles (PS 8401)"), (4, "wall 24 idx (PS 8401 slices 0)"), (5, "leaves 2376 idx (PS 17022, ATC)"), (6, "trunk 159 idx (PS 17023)"), (7, "leaves 3216 idx (PS 17022, ATC)"), (3, "pad 12 idx (PS 17025)")];

/// Everything one run's emulation needs, loaded once.
struct Ctx<'a> {
    root: PathBuf,
    frame: u32,
    m: &'a Manifest,
    meshes: Meshes,
    tex: &'a Textures,
    sampler: Sampler,
    lm_meshes: Vec<crate::sunpass::LmMesh>,
    instances: Vec<crate::sunpass::LmInstance>,
    st_table: Vec<[f32; 4]>,
    top_by_plane: Vec<[f32; 4]>,
    depth_by_id: Vec<[f32; 4]>,
    id_verts: Vec<prepass::IdVertex>,
    id_indices: Vec<u32>,
    id_instances: Vec<prepass::IdInstance>,
    atc_threshold: f32,
    coverage_only: bool,
}

impl<'a> Ctx<'a> {
    fn entries(&self, pass: &str) -> Vec<&'a Entry> {
        let mut v: Vec<&Entry> = self.m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(self.frame)).collect();
        v.sort_by_key(|e| e.eid_last.unwrap_or(0));
        v
    }
    fn load(&self, e: &Entry) -> Buf {
        load_entry(&self.root, e).unwrap_or_else(|err| panic!("{err}"))
    }

    /// The material draws of a run → the atlas (before the water tint).
    fn attr(&self, run: &[DrawRec]) -> Target {
        let terrain = self.terrain();
        let pad = self.pad();
        let eye = [0.0f32; 3];
        let tile_rgb: Option<Box<dyn Fn(&DrawRec) -> [f32; 3] + '_>> = terrain.as_ref().map(|t| {
            // VS 8400 with GbxVisualToWorld = 0: o1 (the normal) = 0, o2 (eye-relative position) = 0 − EyeInWorld
            Box::new(move |d: &DrawRec| prepass::ps_8401(t, [0.0; 3], [-eye[0], -eye[1], -eye[2]], eye, d.i_py, d.i_pxz, d.i_pyx2, d.i_pyh2, [0.0; 3], [0.0; 3])) as Box<dyn Fn(&DrawRec) -> [f32; 3]>
        });
        let pad_rgb: Option<Box<dyn Fn(&DrawRec) -> [f32; 3] + '_>> = pad.as_ref().map(|p| {
            // VS 17024 with GbxVisualToWorld = 0: world position 0, normal 0, o3 = the maps' translations (0)
            Box::new(move |_d: &DrawRec| prepass::ps_17025(p, [0.0; 3], [0.0; 3], [0.0; 4])) as Box<dyn Fn(&DrawRec) -> [f32; 3]>
        });
        let opts = RunOpts { sampler: self.sampler, coverage_only: self.coverage_only, tile_rgb, pad_rgb, atc_threshold: self.atc_threshold };
        let mut tgt = Target::new();
        prepass::run_attr_draws(run, &self.meshes, self.tex, &opts, &mut tgt).unwrap_or_else(|e| panic!("{e}"));
        tgt
    }

    fn terrain(&self) -> Option<prepass::TerrainMaterial<'_>> {
        let bufs = self.root.join(format!("env/frame{}/bufs", self.frame));
        match (self.tex.get("5354"), self.tex.get("5363"), self.tex.get("5367")) {
            (Some(base), Some(x2), Some(h2)) => Some(prepass::TerrainMaterial {
                py_pxz: prepass::read_float4s(&bufs.join("e000032_Pixel_srv3_5352.bin")).unwrap_or_default(),
                py_x2: prepass::read_float4s(&bufs.join("e000032_Pixel_srv4_5361.bin")).unwrap_or_default(),
                py_h2: prepass::read_float4s(&bufs.join("e000032_Pixel_srv5_5365.bin")).unwrap_or_default(),
                base,
                x2,
                h2,
                sampler: self.sampler,
            }),
            _ => None,
        }
    }

    fn pad(&self) -> Option<prepass::PadMaterial<'_>> {
        let mut s_clamp = self.sampler;
        s_clamp.address_u = Address::Clamp;
        s_clamp.address_v = Address::Clamp;
        match (self.tex.get("5457"), self.tex.get("5459"), self.tex.get("14627"), self.tex.get("5468")) {
            (Some(acos), Some(acos_py), Some(base), Some(x2)) => Some(prepass::PadMaterial { acos, acos_py, py_base: base, py_x2: x2, pxz_base: base, s_acos: s_clamp, s_acos_py: s_clamp, s_py: self.sampler, s_pxz: self.sampler, tc_scale_trans_pxz: [0.03125, 0.03125, 0.0, 0.0] }),
            _ => None,
        }
    }

    /// The water-id pass of a run.
    fn ids(&self, run: &[DrawRec]) -> Option<Buf> {
        let d = run.iter().find(|d| d.ps == "17012")?;
        let w2h = prepass::regs2(&d.shader_v["g_CBufferV"]["WorldToHPos"]);
        Some(prepass::run_id_pass(&self.id_verts, &self.id_indices, &self.id_instances[..(d.inst as usize).min(self.id_instances.len())], &w2h, W, H))
    }

    /// The water tint of a run applied to `tgt`, with the id map given.
    fn tint(&self, run: &[DrawRec], ids: &Buf, tgt: &mut Target) -> bool {
        let wd: Vec<WaterDraw> = prepass::water_draws(run);
        let (Some(fog), Some(tr)) = (self.tex.get("15075"), self.tex.get("15078")) else { return false };
        if wd.is_empty() {
            return false;
        }
        let mut ws = Sampler::bilinear_no_mip(Address::Clamp);
        ws.weight_bits = self.sampler.weight_bits;
        let water = WaterData { ids, top_by_plane: self.top_by_plane.clone(), depth_by_id: self.depth_by_id.clone(), fog, transmittance: tr, sampler: ws };
        prepass::run_water_draws(&wd, &self.lm_meshes, &self.instances, &self.st_table, &water, tgt);
        true
    }
}

fn per_class(tgt: &Target, captured: &Buf) -> Vec<String> {
    let mut out = Vec::new();
    for (cls, name) in CLASSES {
        let mask = |x: u32, y: u32| tgt.class[(y * W + x) as usize] == cls;
        let r = compare_where(&tgt.buf, captured, 4, Fmt::F16, &mask);
        if r.texels == 0 {
            continue;
        }
        let mut oa = Buf::new(W, H, 1);
        let mut ca = Buf::new(W, H, 1);
        for y in 0..H {
            for x in 0..W {
                oa.set(x, y, 0, tgt.buf.get(x, y, 3));
                ca.set(x, y, 0, captured.get(x, y, 3));
            }
        }
        let alpha = compare_where(&oa, &ca, 1, Fmt::F16, &mask);
        out.push(format!("  {name}: {} texels — alpha {} exact / {} beyond; rgba {}", r.texels, alpha.exact, alpha.beyond, r.line()));
    }
    out
}

fn coverage_line(tgt: &Target, captured: &Buf) -> String {
    let (mut ours, mut cap, mut only_ours, mut only_cap, mut alpha_off) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for y in 0..H {
        for x in 0..W {
            let (o, c) = (tgt.buf.get(x, y, 3), captured.get(x, y, 3));
            if o > 0.0 {
                ours += 1;
            }
            if c > 0.0 {
                cap += 1;
            }
            if o > 0.0 && c == 0.0 {
                only_ours += 1;
            } else if o == 0.0 && c > 0.0 {
                only_cap += 1;
            } else if o != c {
                alpha_off += 1;
            }
        }
    }
    format!("  coverage: ours {ours} texels, captured {cap}; only ours {only_ours}, only captured {only_cap}, both but alpha differs {alpha_off}")
}

/// The One/One accumulation of a run's atlas into 16963 (PS 1109, ScaleSrc 1): f16_rtne(acc + f16_rtz(src)).
fn accumulate(acc: &mut Buf, src: &Buf) {
    for y in 0..H {
        for x in 0..W {
            for c in 0..4 {
                let s = quantise_f16(src.get(x, y, c), Rounding::Truncate);
                acc.set(x, y, c, quantise_f16(acc.get(x, y, c) + s, Rounding::NearestEven));
            }
        }
    }
}

pub fn run(a: Vec<String>) {
    let root = PathBuf::from(&a[1]);
    let frame: u32 = arg(&a, "--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127447);
    let env_frame: u32 = arg(&a, "--env-frame").map(|v| v.parse().expect("--env-frame")).unwrap_or(127448);
    let show: usize = arg(&a, "--show").map(|v| v.parse().expect("--show")).unwrap_or(0);
    let only_run: Option<usize> = arg(&a, "--run").map(|v| v.parse().expect("--run"));
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
    let m = read_manifest(&txt).expect("manifest");
    let env = root.join(format!("env/frame{env_frame}"));
    let env_pre = root.join(format!("env/frame{frame}"));
    let t0 = std::time::Instant::now();
    let mut draws = prepass::read_draws(&root.join(format!("logs/draws-frame{frame}.json.gz"))).unwrap_or_else(|e| panic!("{e}"));
    match prepass::apply_state(&mut draws, &env_pre.join("state.json")) {
        Ok(n) => println!("state.json: {n} draws with alpha-to-coverage"),
        Err(e) => println!("state.json: {e} (no alpha-to-coverage applied)"),
    }
    let runs = prepass::split_runs(&draws);
    println!("frame {frame}: {} actions, {} runs of {:?} draws ({:.1} s)", draws.len(), runs.len(), runs.iter().map(|r| r.len()).collect::<Vec<_>>(), t0.elapsed().as_secs_f32());
    let meshes = Meshes::load(&env_pre).or_else(|_| Meshes::load(&env)).unwrap_or_else(|e| panic!("{e}"));

    // the textures the runs bind, from both frames' exports; the *_TYPELESS ones through their _UNORM_SRGB views
    let bc1 = match arg(&a, "--bc1").as_deref() {
        Some("ideal") => Bc1Decode::Ideal,
        Some("expand8-trunc") => Bc1Decode::Expand8Trunc,
        _ => Bc1Decode::Expand8Round,
    };
    let mut needed: std::collections::HashSet<String> = runs.iter().flatten().flat_map(|d| d.psrv.iter().map(|(_, id)| id.clone())).collect();
    for id in ["15075", "15078"] {
        needed.insert(id.to_string());
    }
    let srgb_ids = ["14585", "14579", "14609", "14627", "5354", "16796", "14508", "15075", "15078"];
    let mut tex = Textures { by_id: Default::default() };
    for dir in [env.join("textures"), env_pre.join("textures")] {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.contains(".dds") {
                continue;
            }
            let stem = name.split(".dds").next().unwrap_or("");
            let id = stem.rsplit('_').next().unwrap_or("").to_string();
            if id.is_empty() || tex.by_id.contains_key(&id) || !needed.contains(&id) {
                continue;
            }
            match texsample::load_dds(&e.path(), bc1) {
                Ok(mut t) => {
                    if srgb_ids.contains(&id.as_str()) && !flag(&a, "--no-srgb-textures") {
                        t.decode_srgb();
                    }
                    if show > 0 {
                        println!("  texture {id}: {:?} {}×{} × {} slices, {} mips in the file{}", t.fmt, t.w, t.h, t.slices, t.levels[0].len(), if t.complete { "" } else { " (top level only)" });
                    }
                    tex.by_id.insert(id, t);
                }
                Err(err) => println!("  texture {name}: {err}"),
            }
        }
    }
    // the material samplers of the frame (env/frame127447/samplers.json): anisotropic 16×, wrap, no bias / clamp
    let mut sampler = Sampler::trilinear(Address::Wrap);
    sampler.max_aniso = arg(&a, "--aniso").map(|v| v.parse().expect("--aniso")).unwrap_or(16);
    sampler.lod_bias = arg(&a, "--lod-bias").map(|v| v.parse().expect("--lod-bias")).unwrap_or(0.0);
    sampler.weight_bits = arg(&a, "--weight-bits").map(|v| if v == "none" { None } else { Some(v.parse().expect("--weight-bits")) }).unwrap_or(Some(8));

    let mesh_dir = env_pre.join("mesh");
    let bufs = env_pre.join("bufs");
    let rd = |p: &Path| prepass::read_maybe_gz(p).unwrap_or_else(|e| panic!("{e}"));
    let lm_meshes: Vec<crate::sunpass::LmMesh> = ["16955", "16957", "16950", "16948"]
        .iter()
        .zip(["e012448", "e012451", "e012454", "e012457"])
        .map(|(vb, eid)| {
            let verts = crate::sunpass::parse_lm_vertices(&rd(&mesh_dir.join(format!("vb_{vb}.bin"))));
            let indices: Vec<u16> = rd(&mesh_dir.join(format!("{eid}_vsout_indices.bin"))).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            crate::sunpass::LmMesh { verts, indices }
        })
        .collect();
    let top = rd(&bufs.join("e012448_Pixel_srv3_17007.bin"));
    let dep = rd(&bufs.join("e012448_Pixel_srv4_17009.bin"));
    let ctx = Ctx {
        root: root.clone(),
        frame,
        m: &m,
        meshes,
        tex: &tex,
        sampler,
        lm_meshes,
        instances: crate::sunpass::parse_instances(&rd(&mesh_dir.join("vb_17033.bin"))),
        st_table: prepass::read_float4s(&bufs.join("e012448_Vertex_srv0_16959.bin")).unwrap_or_default(),
        top_by_plane: top.chunks_exact(4).map(|c| [f32::from_le_bytes(c.try_into().unwrap()), 0.0, 0.0, 1.0]).collect(),
        depth_by_id: dep.chunks_exact(8).map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap()), 0.0, 1.0]).collect(),
        id_verts: prepass::parse_id_vertices(&rd(&mesh_dir.join("vb_5392.bin"))),
        id_indices: rd(&mesh_dir.join("e012420_vsout_indices.bin")).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as u32).collect(),
        id_instances: prepass::parse_id_instances(&rd(&mesh_dir.join("vb_17032.bin"))),
        atc_threshold: arg(&a, "--atc-threshold").map(|v| v.parse().expect("--atc-threshold")).unwrap_or(0.5),
        coverage_only: flag(&a, "--coverage"),
    };
    if let Some(t) = ctx.terrain() {
        let d0 = runs[0].iter().find(|d| d.ps == "8401").unwrap();
        let c = prepass::ps_8401(&t, [0.0; 3], [0.0; 3], [0.0; 3], d0.i_py, d0.i_pxz, d0.i_pyx2, d0.i_pyh2, [0.0; 3], [0.0; 3]);
        println!("PS 8401 tile colour (slices py {} pxz {} x2 {}): [{:.6}, {:.6}, {:.6}] → × 1/9 = [{:.6}, {:.6}, {:.6}]", d0.i_py, d0.i_pxz, d0.i_pyx2, c[0], c[1], c[2], c[0] / 9.0, c[1] / 9.0, c[2] / 9.0);
    } else {
        println!("(terrain material textures 5354 / 5363 / 5367 not all loaded: tile colours 0)");
    }
    if let Some(p) = ctx.pad() {
        let c = prepass::ps_17025(&p, [0.0; 3], [0.0; 3], [0.0; 4]);
        println!("PS 17025 pad colour: [{:.6}, {:.6}, {:.6}] → × 1/9 = [{:.6}, {:.6}, {:.6}]", c[0], c[1], c[2], c[0] / 9.0, c[1] / 9.0, c[2] / 9.0);
    } else {
        println!("(pad material textures 5457 / 5459 / 14627 / 5468 not all loaded: pad colour 0)");
    }

    if let Some(t) = arg(&a, "--edge") {
        let (x, y) = t.split_once(',').expect("--edge X,Y");
        edge_debug(&runs, &ctx.instances, &ctx.meshes, x.parse().unwrap(), y.parse().unwrap());
        return;
    }
    let attr_entries = (0..runs.len()).map(|k| m.passes.iter().find(|e| e.pass == format!("atlas_attr_{k}") && e.frame == Some(frame)).unwrap_or_else(|| panic!("no atlas_attr_{k} entry"))).collect::<Vec<_>>();
    let ids_entries = ctx.entries("atlas_ids");
    let tint_entries = ctx.entries("setup_ps17018");
    let acc_entries = ctx.entries("setup_ps1109");

    // --- the captured runs, step by step
    let mut tinted: Vec<Buf> = Vec::new();
    for (k, run) in runs.iter().enumerate() {
        if let Some(o) = only_run {
            if o != k {
                continue;
            }
        }
        let captured = ctx.load(attr_entries[k]);
        let t1 = std::time::Instant::now();
        let mut tgt = ctx.attr(run);
        let first = run.iter().find(|d| d.ps == "8401").unwrap();
        println!("run {k} (eids {}..{}, RasterLM translation of the first tile ({:.6}, {:.6}), raster offset {:?}/9 texels) — material draws in {:.1} s:", run.first().unwrap().eid, run.last().unwrap().eid, first.rlm[0][3], first.rlm[1][3], prepass::OFFSETS[6 + k], t1.elapsed().as_secs_f32());
        println!("{}", coverage_line(&tgt, &captured));
        for l in per_class(&tgt, &captured) {
            println!("{l}");
        }
        let ra = compare_where(&tgt.buf, &captured, 4, Fmt::F16, &|_, _| true);
        println!("  whole atlas vs atlas_attr_{k}: {}", ra.line());
        if flag(&a, "--alpha-report") {
            for d in run.iter().filter(|d| d.ps == "17022") {
                if let (Some(mesh), Some((_, id))) = (ctx.meshes.for_draw(d), d.psrv.iter().find(|(s, _)| *s == 0)) {
                    println!("  {}", prepass::uv_report(d, mesh));
                    if let Some(t) = tex.get(id) {
                        println!("  {}", prepass::alpha_test_report(d, mesh, t, &ctx.sampler));
                    }
                }
            }
        }
        // the id map
        let ids_cap = ids_entries.get(k).map(|e| ctx.load(e));
        let ids_ours = ctx.ids(run);
        if let (Some(ours), Some(cap)) = (&ids_ours, &ids_cap) {
            let r = compare_where(ours, cap, 2, Fmt::Exact, &|_, _| true);
            let cov = (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).filter(|&(x, y)| cap.get(x, y, 0) > 0.0).count();
            println!("  water ids (VS 17011 / PS 17012, {} tile instances): {} — {cov} texels carry an id in the capture", ctx.id_instances.len(), r.line());
        }
        // the water tint, with the captured id map (or ours with --our-ids)
        if let Some(e) = tint_entries.get(k) {
            let ids = if flag(&a, "--our-ids") { ids_ours.as_ref().or(ids_cap.as_ref()) } else { ids_cap.as_ref().or(ids_ours.as_ref()) };
            if let Some(ids) = ids {
                let before = tgt.buf.clone();
                let t2 = std::time::Instant::now();
                if ctx.tint(run, ids, &mut tgt) {
                    let cap = ctx.load(e);
                    let r = compare_where(&tgt.buf, &cap, 4, Fmt::F16, &|_, _| true);
                    let (mut changed_cap, mut changed_ours) = (0usize, 0usize);
                    for y in 0..H {
                        for x in 0..W {
                            if captured.get(x, y, 0) != cap.get(x, y, 0) || captured.get(x, y, 2) != cap.get(x, y, 2) {
                                changed_cap += 1;
                            }
                            if before.get(x, y, 0) != tgt.buf.get(x, y, 0) || before.get(x, y, 2) != tgt.buf.get(x, y, 2) {
                                changed_ours += 1;
                            }
                        }
                    }
                    println!("  water tint (VS 17017 / PS 17018, {:.1} s; texels changed: captured {changed_cap}, ours {changed_ours}) vs setup_ps17018: {}", t2.elapsed().as_secs_f32(), r.line());
                    for l in per_class(&tgt, &cap) {
                        println!("  {l}");
                    }
                    if show > 0 {
                        crate::gpucmp::print_diffs(&tgt.buf, &cap, 4, show);
                    }
                }
            }
        }
        tinted.push(tgt.buf.clone());
    }

    // --- the accumulation: run k's tinted atlas added into 16963 — from the captured prior (the frame's first run
    //     has none banked) and chained from our own
    if only_run.is_none() && acc_entries.len() == runs.len() {
        println!("accumulation (PS 1109, ScaleSrc 1, blend One/One into 16963):");
        for k in 1..runs.len() {
            let prior = ctx.load(acc_entries[k - 1]);
            let after = ctx.load(acc_entries[k]);
            let mut ours = prior.clone();
            accumulate(&mut ours, &tinted[k]);
            let r = compare_where(&ours, &after, 4, Fmt::F16, &|_, _| true);
            println!("  16963 after run {k} = f16_rtne(captured 16963 after run {} + f16_rtz(our tinted run {k})): {}", k - 1, r.line());
            // and with the captured tinted run (the accumulation step alone)
            let mut step = prior.clone();
            accumulate(&mut step, &ctx.load(tint_entries[k]));
            let rs = compare_where(&step, &after, 4, Fmt::F16, &|_, _| true);
            println!("    the step alone (captured tinted run {k}): {}", rs.line());
        }
        if flag(&a, "--all-runs") {
            for l in shift_check(&runs) {
                println!("{l}");
            }
            for l in rebuild_check(&runs, &ctx.instances) {
                println!("{l}");
            }
            // the nine runs: the six before this frame are the captured first run shifted by the raster offsets
            let base = &runs[0];
            let mut acc = Buf::new(W, H, 4);
            for k in 0..9usize {
                let (run, note) = if k >= 6 { (runs[k - 6].clone(), "captured") } else { (prepass::rebuild_run(base, k, &ctx.instances), "rebuilt from the STs") };
                // the water draws' LM01_Trans_RasterSS shift with the run too
                let t1 = std::time::Instant::now();
                let mut tgt = ctx.attr(&run);
                let ids = ids_entries.first().map(|e| ctx.load(e)).or_else(|| ctx.ids(&run)).unwrap();
                ctx.tint(&run, &ids, &mut tgt);
                accumulate(&mut acc, &tgt.buf);
                let line = if k >= 6 {
                    let after = ctx.load(acc_entries[k - 6]);
                    let r = compare_where(&acc, &after, 4, Fmt::F16, &|_, _| true);
                    let cov = (0..H).flat_map(|y| (0..W).map(move |x| (x, y))).filter(|&(x, y)| after.get(x, y, 3) > 0.0).count();
                    let mut alpha_exact = 0usize;
                    for y in 0..H { for x in 0..W { if acc.get(x, y, 3) == after.get(x, y, 3) { alpha_exact += 1; } } }
                    format!(" vs captured 16963 after this run: {} — alpha bit-identical on {alpha_exact} of {} texels ({cov} covered)", r.line(), W * H)
                } else {
                    String::new()
                };
                println!("  run {k} ({note}, offset {:?}/9, {:.1} s){line}", prepass::OFFSETS[k], t1.elapsed().as_secs_f32());
                if k >= 6 {
                    // per class of the run's last raster: the relative error of the accumulated colour
                    let after = ctx.load(acc_entries[k - 6]);
                    for (cls, name) in CLASSES {
                        let mask = |x: u32, y: u32| tgt.class[(y * W + x) as usize] == cls;
                        let r = compare_where(&acc, &after, 3, Fmt::F16, &mask);
                        if r.texels == 0 { continue; }
                        let (mut sum_o, mut sum_c, mut sum_abs) = (0f64, 0f64, 0f64);
                        for y in 0..H { for x in 0..W { if mask(x, y) { for c in 0..3 { let (o, t) = (acc.get(x, y, c) as f64, after.get(x, y, c) as f64); sum_o += o; sum_c += t; sum_abs += (o - t).abs(); } } } }
                        println!("    {name}: rgb {} exact / {} within 1 quantum / {} beyond of {}; mean ours/captured {:.5}, mean |Δ| / mean {:.3e}", r.exact, r.ulp1, r.beyond, r.values, sum_o / sum_c.max(1e-30), sum_abs / sum_c.max(1e-30));
                    }
                    if k == 8 {
                        // the end of ROW 1: the resolve into the MDiffuse 16969 (PS 17043 + the sRGB store, ilightin.rs) from our sum
                        let res = crate::ilightin::resolve_ps17043(&acc, false);
                        if let Some(e) = m.passes.iter().find(|e| e.pass == "setup_ps17043" && e.frame == Some(frame + 1)) {
                            let cap = ctx.load(e);
                            let mut q = Buf::new(W, H, 4);
                            for y in 0..H { for x in 0..W { for c in 0..4 { let v = res.get(x, y, c); let v = if c < 3 { crate::gpufmt::linear_to_srgb(v) } else { v }; q.set(x, y, c, crate::ilightin::unorm8_rt(v, crate::gpuenc::UnormRounding::NearestEven)); } } }
                            let r = compare_where(&q, &cap, 4, Fmt::Unorm8, &|_, _| true);
                            println!("  our nine-run sum → PS 17043 → sRGB UNORM8 (the MDiffuse 16969 of frame {}): {}", frame + 1, r.line());
                        }
                    }
                }
            }
            if flag(&a, "--dump-sum") {
                let mut bytes = Vec::with_capacity((W * H * 8) as usize);
                for y in 0..H { for x in 0..W { for c in 0..4 { bytes.extend_from_slice(&crate::gpufmt::encode_f16(acc.get(x, y, c), Rounding::NearestEven).to_le_bytes()); } } }
                std::fs::write("prepass-sum-16963.rgba16f", &bytes).expect("write");
                println!("  wrote prepass-sum-16963.rgba16f (raw RGBA16F {W}×{H})");
            }
        }
    }
    let _ = Report::default();
}

/// How exactly `shift_run` reproduces a captured run's translations from another captured run of the same
/// frame: (draws compared, bit-identical translations, the largest |Δ| in NDC units).
pub fn shift_check(runs: &[Vec<DrawRec>]) -> Vec<String> {
    let mut out = Vec::new();
    for k in 1..runs.len() {
        let derived = prepass::shift_run(&runs[0], 6, 6 + k);
        let (mut n, mut same, mut maxd) = (0usize, 0usize, 0f32);
        for (d, c) in derived.iter().zip(runs[k].iter()) {
            if d.rlm[0] == [0.0; 4] {
                continue;
            }
            n += 1;
            if d.rlm[0][3].to_bits() == c.rlm[0][3].to_bits() && d.rlm[1][3].to_bits() == c.rlm[1][3].to_bits() {
                same += 1;
            }
            maxd = maxd.max((d.rlm[0][3] - c.rlm[0][3]).abs()).max((d.rlm[1][3] - c.rlm[1][3]).abs());
        }
        out.push(format!("  run {} derived from run 0 by the offset shift: {same} of {n} translations bit-identical, max |Δ| {maxd:.3e} NDC ({:.2e} px)", 6 + k, maxd * 1024.0));
    }
    out
}

/// Which f32 expression the game evaluates for `GbxVTexCoordToRasterLM`'s translation: candidates built from
/// the tile's chart ST (the instance stream's TEXCOORD7.zw) and the run's raster offset, scored against the
/// captured translations of every tile draw of every captured run.
pub fn trans_fit(runs: &[Vec<DrawRec>], instances: &[crate::sunpass::LmInstance]) -> Vec<String> {
    const Q: f32 = 2.0 / W as f32; // one texel in NDC
    type F = Box<dyn Fn(f32, i32, f32) -> f32>; // (st.z or st.w, offset, sign) → translation
    let cands: Vec<(&str, F)> = vec![
        ("2·t − 1 + off/9·q", Box::new(|t: f32, o: i32, s: f32| s * (2.0 * t - 1.0) + (o as f32 / 9.0) * Q * s.signum())),
        ("(2·t − 1) + off·(q/9)", Box::new(|t, o, s| s * (2.0 * t - 1.0) + (o as f32) * (Q / 9.0))),
        ("2·t + (off/9·q − 1)", Box::new(|t, o, s| s * (2.0 * t) + ((o as f32 / 9.0) * Q - s))),
        ("2·(t + off/9/2048) − 1", Box::new(|t, o, s| s * (2.0 * (t + (o as f32 / 9.0) / W as f32) - 1.0))),
        ("2·(t + off/(9·2048)) − 1", Box::new(|t, o, s| s * (2.0 * (t + o as f32 / (9.0 * W as f32)) - 1.0))),
        ("(t + off/9/2048)·2 − 1 (mad)", Box::new(|t, o, s| s * (t + (o as f32 / 9.0) / W as f32).mul_add(2.0, -1.0))),
        ("2·t − 1 + off·(2/18432)", Box::new(|t, o, s| s * (2.0 * t - 1.0) + o as f32 * (2.0 / 18432.0))),
        ("t·2 + (off·(2/18432) − 1)", Box::new(|t, o, s| s * (t * 2.0) + (o as f32 * (2.0 / 18432.0) - s))),
    ];
    let mut out = Vec::new();
    for (name, f) in &cands {
        let (mut n, mut same) = (0usize, 0usize);
        for (k, run) in runs.iter().enumerate() {
            let (ox, oy) = prepass::OFFSETS[6 + k];
            // the tile draws in order = instances 3.. of the stream (the water pass's mesh 3 starts at instance 3)
            for (i, d) in run.iter().filter(|d| d.ps == "8401" && d.idx == 24 && d.i_pxz != 0).enumerate() {
                let Some(inst) = instances.get(3 + i) else { break };
                let tx = f(inst.st[2], ox, 1.0);
                let ty = f(inst.st[3], -oy, -1.0);
                n += 1;
                if tx.to_bits() == d.rlm[0][3].to_bits() && ty.to_bits() == d.rlm[1][3].to_bits() {
                    same += 1;
                }
            }
        }
        out.push(format!("  translation candidate {name}: {same} of {n} tile draws bit-identical"));
    }
    // and the scale: 2·st.x / −2·st.y
    let (mut n, mut same) = (0usize, 0usize);
    for run in runs {
        for (i, d) in run.iter().filter(|d| d.ps == "8401" && d.idx == 24 && d.i_pxz != 0).enumerate() {
            let Some(inst) = instances.get(3 + i) else { break };
            n += 1;
            if (2.0 * inst.st[0]).to_bits() == d.rlm[0][0].to_bits() && (-2.0 * inst.st[1]).to_bits() == d.rlm[1][1].to_bits() {
                same += 1;
            }
        }
    }
    out.push(format!("  scale = (2·st.x, −2·st.y): {same} of {n} bit-identical"));
    if let Some(inst) = instances.get(3) {
        out.push(format!("  instance 3 st {:?}; run 0 first tile rlm {:?}", inst.st, runs[0].iter().find(|d| d.ps == "8401").map(|d| d.rlm)));
    }
    out
}

/// `rebuild_run` against the captured runs: every draw's matrix and the water draws' translation, bit for bit.
pub fn rebuild_check(runs: &[Vec<DrawRec>], instances: &[crate::sunpass::LmInstance]) -> Vec<String> {
    let mut out = Vec::new();
    for (k, run) in runs.iter().enumerate() {
        let rb = prepass::rebuild_run(run, 6 + k, instances);
        let (mut n, mut same, mut wn, mut wsame) = (0usize, 0usize, 0usize, 0usize);
        let mut bad: Vec<String> = Vec::new();
        for (d, c) in rb.iter().zip(run.iter()) {
            if c.rlm[0] != [0.0; 4] {
                n += 1;
                let eq = (0..2).all(|r| (0..4).all(|j| d.rlm[r][j].to_bits() == c.rlm[r][j].to_bits() || (d.rlm[r][j] == 0.0 && c.rlm[r][j] == 0.0)));
                if eq { same += 1; } else if bad.len() < 3 { bad.push(format!("eid {} PS {} idx {}: rebuilt {:?} captured {:?}", c.eid, c.ps, c.idx, d.rlm, c.rlm)); }
            }
            if c.ps == "17018" {
                wn += 1;
                if d.shader_v["g_CBufferV"]["LM01_Trans_RasterSS"] == c.shader_v["g_CBufferV"]["LM01_Trans_RasterSS"] { wsame += 1; }
            }
        }
        out.push(format!("  run {} rebuilt from the instance STs and offset {:?}: {same} of {n} raster matrices bit-identical, {wsame} of {wn} water translations", 6 + k, prepass::OFFSETS[6 + k]));
        out.extend(bad.into_iter().map(|b| format!("    {b}")));
    }
    out
}

/// `--edge X,Y`: the tie cases at one texel across the nine runs.
pub fn edge_debug(runs: &[Vec<DrawRec>], instances: &[crate::sunpass::LmInstance], meshes: &Meshes, x: u32, y: u32) {
    for k in 0..9usize {
        let run = if k >= 6 { runs[k - 6].clone() } else { prepass::rebuild_run(&runs[0], k, instances) };
        let lines = prepass::edge_report(&run, meshes, x, y, 2);
        println!("  run {k} offset {:?}: {} triangles inside or within 2/256 px of the centre of ({x}, {y})", prepass::OFFSETS[k], lines.len());
        for l in lines { println!("{l}"); }
    }
}

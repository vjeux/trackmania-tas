//! Kart Silverstone's Grand Prix layout (1377 m, 18 corners; opened 2025 in
//! the old Bridge–Priory infield) as its own map, driven in the SnowCar,
//! with the F1 circuit and the venue kept as scenery.
//!
//! The layout was read off three sources that agree (2026-09-11): the
//! venue's own outline (kart.silverstone.co.uk/circuit), Kart Directory's
//! track map (GP in blue, the cut-throughs of the other layouts in grey,
//! start/finish tick on the main straight) and motorsport-timing.co.uk's
//! sector map (Sector 1 leaves the start/finish towards the bottom-right
//! corner = NORTH-EAST on the main straight). Google's 2026 zoom-20 tiles
//! show the finished track: the grid boxes on the main straight, the
//! timing gantry at their north-east end, the pit lane between the straight
//! and the tents, the movable-barrier plaza where the layouts fork.
//!
//! OSM (11 `sport=karting` ways) is only the topology: its centreline is a
//! few metres off in places and its nodes are sparse through the hairpins,
//! so the centreline is re-centred between the tarmac edges read off the
//! imagery (`imagery.rs`) and the widths come from there too. The LIDAR
//! (2019/2022) predates the track and only gives the ground height.

use crate::edges::Edges;
use crate::geo::Bng;
use crate::imagery::Imagery;
use crate::osm::{self, Loop, Ways};
use crate::tiff::Mosaic;
use crate::track::Track;

/// The GP lap as OSM junction nodes in travel order, starting at the
/// Bridge hairpin's inner end and running the main straight north-east.
pub const GP_NODES: &[i64] = &[
    13912520210, // A: west end of the Bridge hairpin (magenta straight's start)
    13148241480, // B: Bridge link -> the lower arm
    13912520205, // C: 3-way junction, lower arm / "e" exit / hairpin arm
    13912520265, // Q: onto the hairpin arm
    13912520384, // N: out of the hairpin
    13912520256, // M
    13912520266, // L
    13912520201, // K: into the "e" (the double hairpin)
    13912515879, // J: out of the "e", onto the main straight
    13912515876, // I: grid
    13912520208, // H: the plaza (start/finish gantry just before it)
    13912520388, // O
    13912520433, // P: into the loop
    13912520257, // G: out of the loop
    13912520262, // F: Priory
    13148241490, // E: top of Priory, onto the return leg
    13912520222, // D: the kink before the long straight
];

/// A label per leg (what the checkpoints and the lap TSV say).
pub const GP_LABELS: &[&str] = &[
    "Bridge hairpin", "Lower arm", "Arm curve", "Hairpin", "Hairpin exit", "Link", "Wiggle", "Double hairpin", "Straight entry", "Grid", "Plaza", "Loop entry", "Loop", "Loop exit", "Priory", "Return leg", "Kink straight",
];

fn karting(w: &osm::Way) -> bool {
    w.tags.get("highway").map(|s| s.as_str()) == Some("raceway") && w.tags.get("sport").map(|s| s.as_str()) == Some("karting")
}

pub fn gp_loop(ways: &Ways) -> Loop {
    osm::loop_from_nodes(ways, GP_NODES, GP_LABELS, &karting)
}

/// The karting ways' runs the lap does NOT use (the cut-throughs of the
/// other layouts, the pit lane), each extended by one node to the lap
/// junction it leaves from, as open polylines with a name.
pub fn unused_karting_runs(ways: &Ways, lap: &Loop) -> Vec<(String, Vec<Bng>)> {
    let on_lap = lap.edge_set();
    let mut out = Vec::new();
    for way in ways.ways.iter().filter(|w| karting(w)) {
        let n = way.nodes.len();
        let mut i = 0;
        while i + 1 < n {
            if on_lap.contains(&(way.nodes[i], way.nodes[i + 1])) {
                i += 1;
                continue;
            }
            let start = i;
            while i + 1 < n && !on_lap.contains(&(way.nodes[i], way.nodes[i + 1])) {
                i += 1;
            }
            let run: Vec<Bng> = way.nodes[start..=i].iter().filter_map(|id| ways.nodes.get(id).copied()).collect();
            if run.len() >= 2 {
                let name = if way.name.is_empty() { format!("kart way {} [{}..{}]", way.id, start, i) } else { format!("{} [{}..{}]", way.name, start, i) };
                out.push((name, run));
            }
        }
    }
    out
}

/// Bounding box of the karting ways (BNG).
pub fn kart_bbox(ways: &Ways) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for way in ways.ways.iter().filter(|w| karting(w)) {
        for p in way.nodes.iter().filter_map(|id| ways.nodes.get(id)) {
            b.0 = b.0.min(p.e);
            b.1 = b.1.min(p.n);
            b.2 = b.2.max(p.e);
            b.3 = b.3.max(p.n);
        }
    }
    b
}

pub const MIN_HALF: f64 = 2.4;
pub const MAX_HALF: f64 = 9.0;
/// Widths a single-lane kart track can have (m); wider readings are the
/// plaza or a junction and are interpolated across.
pub const WIDTH_OK: (f64, f64) = (5.2, 12.5);

/// The centreline re-centred between the imagery edges: build, read both
/// edges, move each trusted station to their midpoint, smooth the shift,
/// rebuild; `rounds` times. Returns the final track, its edges and the
/// per-station trust flags of the last read.
pub fn refine(lp: &Loop, dtm: &Mosaic, img: &Imagery, rounds: usize, sigma_xy: f64, sigma_z: f64) -> (Track, Edges, Vec<bool>) {
    let mut points = lp.points.clone();
    let mut labels = lp.labels.clone();
    let mut tr = Track::build_points_pub(&points, &labels, true, dtm, 1.0, sigma_xy, sigma_z, 3.5);
    let (mut ed, mut both) = Edges::from_imagery(&tr, img, MIN_HALF, MAX_HALF, WIDTH_OK, 2.0);
    for round in 0..rounds {
        let n = tr.len();
        // lateral shift towards the midpoint of the two edges, trusted stations only
        let mut shift: Vec<f64> = (0..n).map(|i| if both[i] { (ed.left[i] - ed.right[i]) / 2.0 } else { f64::NAN }).collect();
        let trusted = both.iter().filter(|b| **b).count();
        // gaps: interpolate; then smooth
        {
            let valid: Vec<usize> = (0..n).filter(|&i| !shift[i].is_nan()).collect();
            if valid.len() * 3 < n {
                println!("refine round {round}: only {trusted}/{n} stations read on both sides; stopping");
                break;
            }
            let src = shift.clone();
            for i in 0..n {
                if !src[i].is_nan() {
                    continue;
                }
                let mut a = i;
                let mut da = 0;
                while src[a].is_nan() {
                    a = (a + n - 1) % n;
                    da += 1;
                }
                let mut b = i;
                let mut db = 0;
                while src[b].is_nan() {
                    b = (b + 1) % n;
                    db += 1;
                }
                let t = da as f64 / (da + db) as f64;
                shift[i] = src[a] * (1.0 - t) + src[b] * t;
            }
        }
        let shift = crate::track::smooth(&shift, 3.0 / tr.ds, true);
        let max_shift = shift.iter().fold(0.0f64, |a, s| a.max(s.abs()));
        let rms = (shift.iter().map(|s| s * s).sum::<f64>() / n as f64).sqrt();
        points = (0..n)
            .map(|i| {
                let p = tr.offset(i, shift[i]);
                Bng { e: p[0], n: p[1] }
            })
            .collect();
        labels = tr.stations.iter().map(|s| s.label.clone()).collect();
        tr = Track::build_points_pub(&points, &labels, true, dtm, 1.0, sigma_xy.min(1.5), sigma_z, 3.5);
        let (e2, b2) = Edges::from_imagery(&tr, img, MIN_HALF, MAX_HALF, WIDTH_OK, 2.0);
        ed = e2;
        both = b2;
        let widths: Vec<f64> = (0..tr.len()).map(|i| ed.left[i] + ed.right[i]).collect();
        let (wmin, wmax) = widths.iter().fold((f64::MAX, f64::MIN), |(a, b), w| (a.min(*w), b.max(*w)));
        let wmean = widths.iter().sum::<f64>() / widths.len() as f64;
        println!("refine round {round}: {trusted}/{n} stations trusted, shift rms {rms:.2} m max {max_shift:.2} m; lap now {:.1} m, width {wmin:.1}..{wmax:.1} (mean {wmean:.1}), {} interpolated", tr.len() as f64 * tr.ds, ed.guessed);
    }
    (tr, ed, both)
}

/// Map names of the host's name length (24 for TMX 135841).
pub const KART_NAMES: &[&str] = &["Kart Silverstone GP 1:1 ", "Kart Silverstone GP, 1:1", "Kart Silverstone GP", "Kart Silverstone", "Kart Silverstone Grand Prix 1:1"];

/// Where the start/finish gantry crosses the main straight (BNG), read off
/// Google's 2026 imagery: the north-east end of the grid boxes.
pub const GANTRY: (f64, f64) = (467395.0, 242268.0);

pub struct Params {
    pub osm: std::path::PathBuf,
    pub dtm_dir: std::path::PathBuf,
    pub tif: std::path::PathBuf,
    pub host: std::path::PathBuf,
    pub out: std::path::PathBuf,
    pub surroundings: Option<std::path::PathBuf>,
    pub seg: f64,
    pub cp: f64,
    pub start_en: (f64, f64),
    pub author_ms: Option<u32>,
    pub zoom: u32,
    pub rounds: usize,
    pub car: Option<String>,
}

/// The whole pipeline for the kart map: the kart GP lap from OSM topology
/// + imagery, the F1 lap and every other road of the venue as scenery,
/// terrain and buildings as the F1 map has them, SnowCar.
pub fn build(p: &Params) {
    use crate::mapbuild::{self, Frame, Placement, RoadStyle};
    use crate::roads::{self, Road};
    use crate::terrain::{self, Ribbon};
    use crate::{check, tiff};
    let ways = osm::load(&p.osm);
    let around = p.surroundings.as_ref().map(|s| osm::load(s));
    let dtm = Mosaic::load(&crate::tiles(&p.dtm_dir, "dtm_"));
    let inten = tiff::Raster::load(&p.tif);
    // The F1 lap: the frame is its box (the venue), as in the F1 map.
    let f1 = osm::gp_loop(&ways);
    let tr1 = Track::build(&f1, &dtm, 1.0, 3.0, 8.0, 6.0);
    let ed1 = Edges::from_intensity(&tr1, &inten);
    println!("F1 lap {:.1} m, {} stations", tr1.len() as f64 * tr1.ds, tr1.len());
    let (e0, n0, e1, n1) = tr1.bbox();
    let host_size = tmmaps::map::MapFile::load(&p.host).size;
    let margin = 200.0;
    let need_x = (((e1 - e0 + 2.0 * margin) / 32.0).ceil() as i32).max(48);
    let need_z = (((n1 - n0 + 2.0 * margin) / 32.0).ceil() as i32).max(48);
    let (sx, sz) = (host_size[0], host_size[2]);
    assert!(sx >= need_x && sz >= need_z, "host grid {host_size:?} is too small for the venue ({need_x} x {need_z} blocks needed)");
    // The kart lap: OSM topology, imagery geometry.
    let lp = gp_loop(&ways);
    println!("kart GP lap from OSM: {} nodes, {:.1} m", lp.points.len(), lp.length());
    let kb = kart_bbox(&ways);
    let img = Imagery::fetch(kb, 60.0, 0.1, p.zoom, crate::aerial::Source::Google).unwrap_or_else(|e| panic!("imagery: {e}"));
    let (tr, ed, both) = refine(&lp, &dtm, &img, p.rounds, 2.0, 15.0);
    let n = tr.len();
    println!("kart lap {:.1} m, {} stations, {} trusted on both sides, {} edge readings interpolated", n as f64 * tr.ds, n, both.iter().filter(|b| **b).count(), ed.guessed);
    let zmin = tr.stations.iter().chain(tr1.stations.iter()).map(|s| s.z).fold(f64::MAX, f64::min);
    let (ce, cn) = ((e0 + e1) / 2.0, (n0 + n1) / 2.0);
    let fr = Frame { e0: ce - (sx as f64) * 16.0, n0: cn + (sz as f64) * 16.0, z_ref: zmin - 12.0 };
    println!("map size {sx} x {} x {sz} blocks; origin E{:.0} N{:.0}; kart track y {:.1}..{:.1}", host_size[1], fr.e0, fr.n0, tr.stations.iter().map(|s| s.z).fold(f64::MAX, f64::min) - fr.z_ref, tr.stations.iter().map(|s| s.z).fold(f64::MIN, f64::max) - fr.z_ref);
    // Start and finish: the finish trigger on the gantry line, the car
    // spawning 30 m back on the grid.
    let nearest = |e: f64, nn: f64| tr.stations.iter().enumerate().map(|(i, s)| (i, (s.e - e).powi(2) + (s.n - nn).powi(2))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|(i, d)| (i, d.sqrt())).unwrap();
    let (gantry, off) = nearest(p.start_en.0, p.start_en.1);
    let start = (gantry + n - 32) % n;
    let mut plan = mapbuild::plan_waypoints(&tr, start, p.cp);
    plan.finish = gantry;
    println!("gantry -> station {gantry} ({}), {off:.1} m off the centreline; spawn station {start}; {} checkpoints at {:?}", tr.stations[gantry].label, plan.checkpoints.len(), plan.checkpoints.iter().map(|&c| format!("{}:{}", tr.stations[c].s as i64, tr.stations[c].label)).collect::<Vec<_>>());
    // Roads: the kart lap first, then the F1 lap, then every other piece of
    // tarmac (the venue from the LIDAR, the kart cut-throughs and pit lane
    // from the imagery), each clipped against what came before.
    let (de0, dn0, de1, dn1) = dtm.bounds();
    let map_box = ((fr.e0).max(de0), (fr.n0 - sz as f64 * 32.0).max(dn0), (fr.e0 + sx as f64 * 32.0).min(de1), (fr.n0).min(dn1));
    let lap_ribbon = Ribbon::build(&tr, &ed, map_box, 40.0);
    let mut union = Ribbon::build(&tr, &ed, map_box, 40.0);
    let n1s = tr1.len();
    let f1_road = Road { name: "F1 Grand Prix circuit".into(), from_imagery: false, track: tr1, edges: ed1, skip: vec![false; n1s], shoulders: vec![(true, true); n1s], caps: vec![(false, false); n1s], drawn: std::cell::RefCell::new(Vec::new()) };
    union.add(&f1_road.track, &f1_road.edges, 40.0);
    let mut both_laps = Ribbon::build(&tr, &ed, map_box, 40.0);
    both_laps.add(&f1_road.track, &f1_road.edges, 40.0);
    let keep = |w: &osm::Way| !karting(w);
    let extra: Vec<roads::Candidate> = unused_karting_runs(&ways, &lp).into_iter().map(|(name, pts)| (name, pts, true)).collect();
    println!("{} unused karting runs (cut-throughs, pit lane)", extra.len());
    let mut extra_roads = if crate::a_has("--no-extra-roads") { Vec::new() } else { roads::extra_roads_with(&ways, &both_laps, &mut union, &dtm, &inten, &keep, extra, Some(&img)) };
    extra_roads.insert(0, f1_road);
    println!("{} extra roads (the F1 lap, pit lanes, other layouts, link roads)", extra_roads.len());
    let others = {
        let mut r = Ribbon::new(map_box);
        for road in &extra_roads {
            r.add(&road.track, &road.edges, 10.0);
        }
        r
    };
    let shoulder_asphalt = |k: usize, left: bool| -> bool {
        let k1 = (k + 1) % n;
        let (edge, kerb) = if left { (ed.left[k], terrain::kerb_width(ed.kerb_left[k], ed.kerb_left[k1])) } else { (ed.right[k], terrain::kerb_width(ed.kerb_right[k], ed.kerb_right[k1])) };
        let off = edge + kerb + 0.75;
        let w = tr.offset(k, if left { off } else { -off });
        img.asphalt_majority(w[0], w[1], 1.0).unwrap_or(false)
    };
    let shoulder_sides = |k: usize| -> (bool, bool) {
        let k1 = (k + 1) % n;
        let side = |left: bool| -> bool {
            let (edge, kerb) = if left { (ed.left[k], terrain::kerb_width(ed.kerb_left[k], ed.kerb_left[k1])) } else { (ed.right[k], terrain::kerb_width(ed.kerb_right[k], ed.kerb_right[k1])) };
            let off = edge + kerb + 0.75;
            let w = tr.offset(k, if left { off } else { -off });
            others.beyond_kerb(w[0], w[1]) >= 0.0
        };
        (side(true), side(false))
    };
    let own_height = |_k: usize, _off: f64, own: f64| own;
    let style = RoadStyle { shoulder_asphalt: &shoulder_asphalt, shoulder_sides: &shoulder_sides, height: &own_height };
    let (mut items, skip) = mapbuild::waypoint_items(&tr, &ed, &fr, &plan, 16.0, &style);
    let mut road_items = mapbuild::road_items(&tr, &ed, &fr, p.seg, &skip, &style, "Road");
    println!("{} waypoint items, {} road items", items.len(), road_items.len());
    {
        let mut before = Ribbon::build(&tr, &ed, map_box, 40.0);
        for (idx, road) in extra_roads.iter().enumerate() {
            road_items.extend(roads::road_items_for(road, idx, &fr, &before, &inten, &dtm, Some(&img)));
            before.add(&road.track, &road.edges, 40.0);
        }
    }
    // The venue: the F1 lap's box plus 220 m, as the F1 map has it. The
    // ground under and around the kart track is classified off the imagery
    // (the 2019 intensity still shows the old Bridge tarmac and grass there).
    let venue = (e0 - 220.0, n0 - 220.0, e1 + 220.0, n1 + 220.0);
    let kart_zone = (kb.0 - 50.0, kb.1 - 50.0, kb.2 + 50.0, kb.3 + 50.0);
    let asphalt_override = |e: f64, nn: f64, step: f64| -> Option<bool> {
        if e < kart_zone.0 || e > kart_zone.2 || nn < kart_zone.1 || nn > kart_zone.3 {
            return None;
        }
        img.asphalt_majority(e, nn, step)
    };
    let mut extras: Vec<Placement> = Vec::new();
    if !crate::a_has("--no-terrain") {
        let spec = terrain::TerrainSpec { bbox: venue, coarse_bbox: map_box, coarse: 32.0, fine: 4.0, near: 30.0, tile: 256.0 };
        extras.extend(terrain::terrain_items(&union, &dtm, &inten, &fr, &spec, Some(&asphalt_override)));
    }
    if let Some(w) = around.as_ref() {
        if !crate::a_has("--no-buildings") {
            let dsm = Mosaic::load(&crate::tiles(&p.dtm_dir, "dsm_"));
            let structs = crate::buildings::structures(w, &dtm, &dsm, venue, &tr, &ed, &union);
            let n_stand = structs.iter().filter(|s| s.kind == "grandstand").count();
            println!("{} structures ({n_stand} grandstands); tallest {:.0} m", structs.len(), structs.iter().map(|s| s.height).fold(0.0, f64::max));
            extras.extend(crate::buildings::building_items(&structs, &fr));
            extras.extend(crate::buildings::linear_items(w, &dtm, &dsm, &ed, &fr, venue, &tr, 60.0, &union));
        }
    }
    println!("{} terrain/surroundings items", extras.len());
    items.extend(road_items);
    items.extend(extras);
    println!("{} items, {:.1} MB", items.len(), items.iter().map(|p| p.bytes.len()).sum::<usize>() as f64 / 1e6);
    let mut per: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for it in &items {
        let stem = it.ident.rsplit('\\').next().unwrap_or(&it.ident);
        let key: String = stem.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let e = per.entry(key).or_default();
        e.0 += 1;
        e.1 += it.bytes.len();
    }
    for (k, (cnt, b)) in &per {
        println!("  {k:<12} {cnt:>4} items {:>6.2} MB", *b as f64 / 1e6);
    }
    // The self-check on the kart lap (what the car drives) and on every
    // other road's drawn slices.
    let report = check::run(&items, &tr, &ed, &fr, 45.0);
    check::print(&report);
    for road in &extra_roads {
        let r = check::run_road(&items, road, &fr);
        if !r.offenders.is_empty() {
            println!("check: on {}:", road.name);
            check::print(&r);
        }
    }
    if report.fatal() && !crate::a_has("--allow-defects") {
        eprintln!("circuit: the kart lap has surface defects; not written (pass --allow-defects to write it anyway)");
        std::process::exit(2);
    }
    let uid = mapbuild::map_uid_prefixed("SilverstoneKart", &items);
    println!("uid {uid}");
    let name = |len: usize| mapbuild::name_of_len_for(len, KART_NAMES, "Kart Silverstone Grand Prix layout 1:1, F1 circuit as scenery");
    mapbuild::assemble_named(&p.host, &p.out, &items, None, &uid, p.author_ms, &name, p.car.as_deref());
    // The lap for whoever drives it, in TM world coordinates.
    let line_path = p.out.with_extension("line.tsv");
    let mut w = String::from("# Kart Silverstone GP lap in Trackmania world coordinates (x east, y up, z south); heading = atan2(dx, dz) of travel; curvature >0 turns left\n");
    w.push_str(&format!("# start_station\t{}\tfinish_station\t{}\tcheckpoints\t{}\n", plan.start, plan.finish, plan.checkpoints.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")));
    w.push_str("station\ts_m\tx\ty\tz\theading_rad\tcurvature\tleft_m\tright_m\tcorner\n");
    for (i, st) in tr.stations.iter().enumerate() {
        let q = fr.to_tm(st.e, st.n, st.z);
        let (sh, ch) = st.heading.sin_cos();
        let heading_tm = mapbuild::yaw_for(ch as f32, -sh as f32);
        w.push_str(&format!("{i}\t{:.1}\t{:.3}\t{:.3}\t{:.3}\t{:.5}\t{:.5}\t{:.2}\t{:.2}\t{}\n", st.s, q[0], q[1], q[2], heading_tm, st.curvature, ed.left[i], ed.right[i], st.label));
    }
    std::fs::write(&line_path, w).expect("line tsv");
    println!("lap written to {}", line_path.display());
}

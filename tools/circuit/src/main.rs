//! `circuit` — a real-world race circuit as a 1:1 Trackmania 2020 map.
//!
//! Data in: an OpenStreetMap Overpass dump of the raceway (the outline),
//! Environment Agency LIDAR rasters (the ground), OSM buildings (the
//! surroundings). Data out: custom items (road, terrain, buildings) and a
//! `.Map.Gbx` that places them, with waypoints along the lap.

mod aerial;
mod buildings;
mod check;
mod edges;
mod geo;
mod imagery;
mod kart;
mod item_probe;
mod mapbuild;
mod mesh;
mod osm;
mod png;
mod roads;
mod terrain;
mod tiff;
mod track;

use std::collections::HashMap;
use std::path::Path;

fn usage() -> ! {
    eprintln!(
        "circuit — real-world circuit -> TM2020 map\n\
         \n\
         circuit probe-item FILE...          what a crystal item carries: layers, materials, waypoint chunk\n\
         circuit osm-loop OVERPASS.json      find the GP loop in a raceway dump and print it as TSV (BNG metres)\n\
         circuit dtm-stat TILE.tif...        georeference + value range of LIDAR GeoTIFF tiles\n\
         circuit preview OVERPASS.json DTM_DIR OUT.png [--ds 1] [--width 15]\n\
                                             build the 3D centreline and draw it over the LIDAR ground\n"
    );
    std::process::exit(2)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(|s| s.as_str()) {
        Some("probe-item") => {
            for f in &a[2..] {
                item_probe::probe(Path::new(f));
            }
        }
        Some("osm-loop") => {
            let path = a.get(2).unwrap_or_else(|| usage());
            let ways = osm::load(Path::new(path));
            let lp = osm::gp_loop(&ways);
            println!("# loop: {} ways, {} nodes, length {:.1} m", lp.way_names.len(), lp.points.len(), lp.length());
            for (i, p) in lp.points.iter().enumerate() {
                println!("{i}\t{:.2}\t{:.2}\t{}", p.e, p.n, lp.labels[i]);
            }
        }
        Some("dtm-stat") => {
            for f in &a[2..] {
                let t = tiff::Raster::load(Path::new(f));
                println!("{}: {}x{} origin E{:.1} N{:.1} px {:.2} m; z {:.2}..{:.2} (nodata {})", f, t.width, t.height, t.x0, t.y0, t.px, t.min, t.max, t.nodata_count);
            }
        }
        Some("preview") => {
            // circuit preview OVERPASS.json DTM_DIR OUT.png [--intensity TIF] [--ds 1]
            let (osm_path, dtm_dir, out) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()));
            let ds = flag(&a, "--ds").unwrap_or(1.0);
            let ways = osm::load(Path::new(osm_path));
            let lp = osm::gp_loop(&ways);
            let dtm = tiff::Mosaic::load(&tiles(Path::new(dtm_dir), "dtm_"));
            let tr = track::Track::build(&lp, &dtm, ds, 3.0, 8.0, 6.0);
            let inten_path = a.iter().position(|x| x == "--intensity").and_then(|i| a.get(i + 1));
            let ed = inten_path.map(|p| edges::Edges::from_intensity(&tr, &tiff::Raster::load(Path::new(p))));
            preview(&tr, &dtm, Path::new(out), ed.as_ref());
        }
        Some("raster-png") => {
            // circuit raster-png TIF OUT.png E0,N0,E1,N1 [--scale 2] [--osm dump.json]
            let (tif, out, bb) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()));
            let v: Vec<f64> = bb.split(',').map(|x| x.parse().expect("bbox number")).collect();
            assert_eq!(v.len(), 4, "bbox E0,N0,E1,N1");
            let scale = flag(&a, "--scale").unwrap_or(2.0);
            let lap = a.iter().position(|x| x == "--osm").and_then(|i| a.get(i + 1)).map(|p| osm::gp_loop(&osm::load(Path::new(p))));
            // --dtm DIR: also build the track + intensity edges and draw them
            let dtm_dir = a.iter().position(|x| x == "--dtm").and_then(|i| a.get(i + 1));
            let tr_edges = match (&lap, dtm_dir) {
                (Some(lp), Some(d)) => {
                    let dtm = tiff::Mosaic::load(&tiles(Path::new(d), "dtm_"));
                    let tr = track::Track::build(lp, &dtm, 1.0, 3.0, 8.0, 6.0);
                    let ed = edges::Edges::from_intensity(&tr, &tiff::Raster::load(Path::new(tif)));
                    Some((tr, ed))
                }
                _ => None,
            };
            raster_png(Path::new(tif), Path::new(out), (v[0], v[1], v[2], v[3]), scale, lap.as_ref(), tr_edges.as_ref());
        }
        Some("profiles") => {
            // circuit profiles OVERPASS.json DTM_DIR INTENSITY.tif s1,s2,...
            let (osm_path, dtm_dir, tif, ss) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()), a.get(5).unwrap_or_else(|| usage()));
            let ways = osm::load(Path::new(osm_path));
            let lp = osm::gp_loop(&ways);
            let dtm = tiff::Mosaic::load(&tiles(Path::new(dtm_dir), "dtm_"));
            let tr = track::Track::build(&lp, &dtm, 1.0, 3.0, 8.0, 7.5);
            let inten = tiff::Raster::load(Path::new(tif));
            let at: Vec<f64> = ss.split(',').map(|x| x.parse().expect("station")).collect();
            profiles(&tr, &inten, &dtm, &at);
        }
        Some("build") => {
            // circuit build OVERPASS.json DTM_DIR INTENSITY.tif HOST.Map.Gbx OUT.Map.Gbx [--seg 100] [--cp 450] [--half 16]
            let (osm_path, dtm_dir, tif, host, out) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()), a.get(5).unwrap_or_else(|| usage()), a.get(6).unwrap_or_else(|| usage()));
            let seg = flag(&a, "--seg").unwrap_or(100.0);
            let cp = flag(&a, "--cp").unwrap_or(450.0);
            let half = flag(&a, "--half").unwrap_or(16.0);
            let surroundings = a.iter().position(|x| x == "--surroundings").and_then(|i| a.get(i + 1)).map(|p| Path::new(p).to_path_buf());
            // --variant all|size|items|nowp|roads: bisection aids for the game
            let variant = a.iter().position(|x| x == "--variant").and_then(|i| a.get(i + 1)).cloned().unwrap_or("all".into());
            let limit = flag(&a, "--limit").map(|v| v as usize);
            build(Path::new(osm_path), Path::new(dtm_dir), Path::new(tif), Path::new(host), Path::new(out), seg, cp, half, surroundings.as_deref(), &variant, limit);
        }
        Some("aerial") => {
            // circuit aerial OSM DTM_DIR INTENSITY.tif OUT_DIR s1,s2,... [--span 80] [--px 0.2]
            let (osm_path, dtm_dir, tif, outdir, ss) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()), a.get(5).unwrap_or_else(|| usage()), a.get(6).unwrap_or_else(|| usage()));
            let ways = osm::load(Path::new(osm_path));
            let lp = osm::gp_loop(&ways);
            let dtm = tiff::Mosaic::load(&tiles(Path::new(dtm_dir), "dtm_"));
            let tr = track::Track::build(&lp, &dtm, 1.0, 3.0, 8.0, 6.0);
            let inten = tiff::Raster::load(Path::new(tif));
            let ed = edges::Edges::from_intensity(&tr, &inten);
            let span = flag(&a, "--span").unwrap_or(80.0);
            let px = flag(&a, "--px").unwrap_or(0.3); // the service refuses anything finer than its 0.3 m tiles
            std::fs::create_dir_all(outdir).ok();
            for s in ss.split(',') {
                let at: usize = s.trim().parse().expect("station");
                let out = Path::new(outdir).join(format!("aerial_{at:04}.png"));
                if let Err(e) = aerial::overlay(&tr, &ed, at, span, px, &out) {
                    eprintln!("station {at}: {e}");
                }
            }
        }
        Some("osm-ways") => {
            // circuit osm-ways OVERPASS.json --tag sport=karting
            // every way carrying the tag: id, name, length, its nodes in BNG,
            // and which of its nodes are junctions (shared with another such way)
            let ways = osm::load(Path::new(a.get(2).unwrap_or_else(|| usage())));
            let tag = a.iter().position(|x| x == "--tag").and_then(|i| a.get(i + 1)).cloned().unwrap_or("highway=raceway".into());
            let (k, v) = tag.split_once('=').expect("--tag key=value");
            let sel: Vec<&osm::Way> = ways.ways.iter().filter(|w| w.tags.get(k).map(|x| x == v) == Some(true)).collect();
            let mut uses: HashMap<i64, usize> = HashMap::new();
            for w in &sel {
                for &n in &w.nodes {
                    *uses.entry(n).or_default() += 1;
                }
            }
            let mut total = 0.0;
            for w in &sel {
                let pts: Vec<_> = w.nodes.iter().filter_map(|id| ways.nodes.get(id)).collect();
                let len: f64 = pts.windows(2).map(|q| ((q[0].e - q[1].e).powi(2) + (q[0].n - q[1].n).powi(2)).sqrt()).sum();
                total += len;
                println!("way {} {:?} {:.1} m {} nodes tags {:?}", w.id, w.name, len, pts.len(), w.tags);
                for (i, id) in w.nodes.iter().enumerate() {
                    let p = ways.nodes[id];
                    let j = if uses[id] > 1 || i == 0 || i + 1 == w.nodes.len() { format!(" junction x{}", uses[id]) } else { String::new() };
                    println!("   {i:>3} node {id} E{:.1} N{:.1}{j}", p.e, p.n);
                }
            }
            println!("# {} ways, {:.1} m of centreline", sel.len(), total);
        }
        Some("build-kart") => {
            // circuit build-kart OVERPASS.json DTM_DIR INTENSITY.tif HOST.Map.Gbx OUT.Map.Gbx
            //     [--surroundings s.json] [--seg 100] [--cp 150] [--start-en E,N] [--zoom 20] [--rounds 3]
            //     [--car CarSnow|none] [--author-ms N] [--no-terrain] [--no-buildings] [--no-extra-roads] [--allow-defects]
            let g = |i: usize| a.get(i).map(|s| Path::new(s).to_path_buf()).unwrap_or_else(|| usage());
            let start_en = a.iter().position(|x| x == "--start-en").and_then(|i| a.get(i + 1)).map(|s| {
                let v: Vec<f64> = s.split(',').map(|x| x.parse().expect("E,N")).collect();
                (v[0], v[1])
            }).unwrap_or(kart::GANTRY);
            let car = a.iter().position(|x| x == "--car").and_then(|i| a.get(i + 1)).cloned().unwrap_or("CarSnow".into());
            let params = kart::Params {
                osm: g(2), dtm_dir: g(3), tif: g(4), host: g(5), out: g(6),
                surroundings: a.iter().position(|x| x == "--surroundings").and_then(|i| a.get(i + 1)).map(|p| Path::new(p).to_path_buf()),
                seg: flag(&a, "--seg").unwrap_or(100.0),
                cp: flag(&a, "--cp").unwrap_or(150.0),
                start_en,
                author_ms: flag(&a, "--author-ms").map(|v| v as u32),
                zoom: flag(&a, "--zoom").unwrap_or(20.0) as u32,
                rounds: flag(&a, "--rounds").unwrap_or(3.0) as usize,
                car: if car == "none" { None } else { Some(car) },
            };
            kart::build(&params);
        }
        Some("kart-trace") => {
            // circuit kart-trace OVERPASS.json DTM_DIR OUT.png [--rounds 3] [--zoom 20] [--crop E0,N0,E1,N1]
            //   the GP lap: OSM topology, centreline re-centred between the imagery edges;
            //   drawn on the imagery (centre white, edges green/red, kerbs yellow, untrusted stations magenta)
            let (osm_path, dtm_dir, out) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()));
            let rounds = flag(&a, "--rounds").unwrap_or(3.0) as usize;
            let zoom = flag(&a, "--zoom").unwrap_or(20.0) as u32;
            let px = flag(&a, "--px").unwrap_or(0.1);
            let ways = osm::load(Path::new(osm_path));
            let lp = kart::gp_loop(&ways);
            println!("kart GP lap from OSM: {} nodes, {:.1} m", lp.points.len(), lp.length());
            let dtm = tiff::Mosaic::load(&tiles(Path::new(dtm_dir), "dtm_"));
            let bb = kart::kart_bbox(&ways);
            let img = imagery::Imagery::fetch(bb, 40.0, px, zoom, aerial::Source::Google).unwrap_or_else(|e| panic!("{e}"));
            println!("imagery {}x{} px at {} m over E{:.0}..{:.0} N{:.0}..{:.0}", img.w, img.h, img.px, img.e0, img.e0 + img.w as f64 * img.px, img.n0, img.n0 + img.h as f64 * img.px);
            let (tr, ed, both) = kart::refine(&lp, &dtm, &img, rounds, 2.0, 15.0);
            let n = tr.len();
            let mut lines: Vec<(Vec<geo::Bng>, [u8; 3])> = Vec::new();
            let poly = |f: &dyn Fn(usize) -> [f64; 3]| -> Vec<geo::Bng> {
                let mut v: Vec<geo::Bng> = (0..n).map(|i| {
                    let p = f(i);
                    geo::Bng { e: p[0], n: p[1] }
                }).collect();
                v.push(v[0]);
                v
            };
            lines.push((poly(&|i| tr.offset(i, ed.left[i])), [0, 255, 0]));
            lines.push((poly(&|i| tr.offset(i, -ed.right[i])), [255, 40, 40]));
            lines.push((poly(&|i| tr.offset(i, ed.left[i] + ed.kerb_left[i])), [255, 230, 0]));
            lines.push((poly(&|i| tr.offset(i, -(ed.right[i] + ed.kerb_right[i]))), [255, 230, 0]));
            lines.push((poly(&|i| tr.offset(i, 0.0)), [255, 255, 255]));
            for i in 0..n {
                if !both[i] {
                    let p = tr.offset(i, 0.0);
                    let q = tr.offset(i, 0.6);
                    lines.push((vec![geo::Bng { e: p[0], n: p[1] }, geo::Bng { e: q[0], n: q[1] }], [255, 0, 255]));
                }
            }
            // OSM's own polyline in blue for comparison
            let mut osm_pts = lp.points.clone();
            osm_pts.push(lp.points[0]);
            lines.push((osm_pts, [60, 120, 255]));
            if let Some(c) = a.iter().position(|x| x == "--crop").and_then(|i| a.get(i + 1)) {
                let v: Vec<f64> = c.split(',').map(|x| x.parse().expect("crop")).collect();
                let sub = img.crop((v[0], v[1], v[2], v[3]));
                sub.save_png(Path::new(out), &lines);
            } else {
                img.save_png(Path::new(out), &lines);
            }
            // the lap as TSV next to the picture
            let mut w = String::from("station\ts_m\te\tn\tz\theading\tcurvature\tleft\tright\tkerb_l\tkerb_r\ttrusted\tlabel\n");
            for (i, st) in tr.stations.iter().enumerate() {
                w.push_str(&format!("{i}\t{:.1}\t{:.2}\t{:.2}\t{:.2}\t{:.4}\t{:.5}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\n", st.s, st.e, st.n, st.z, st.heading, st.curvature, ed.left[i], ed.right[i], ed.kerb_left[i], ed.kerb_right[i], both[i] as u8, st.label));
            }
            std::fs::write(Path::new(out).with_extension("tsv"), w).expect("tsv");
            println!("{out}: {} stations, {:.1} m; z {:.2}..{:.2}", n, n as f64 * tr.ds, tr.stations.iter().map(|s| s.z).fold(f64::MAX, f64::min), tr.stations.iter().map(|s| s.z).fold(f64::MIN, f64::max));
        }
        Some("img-profile") => {
            // circuit img-profile OVERPASS.json DTM_DIR s1,s2,... : the imagery classes across the lap at stations
            let (osm_path, dtm_dir, ss) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()));
            let ways = osm::load(Path::new(osm_path));
            let lp = kart::gp_loop(&ways);
            let dtm = tiff::Mosaic::load(&tiles(Path::new(dtm_dir), "dtm_"));
            let img = imagery::Imagery::fetch(kart::kart_bbox(&ways), 40.0, 0.1, 20, aerial::Source::Google).unwrap_or_else(|e| panic!("{e}"));
            let tr = track::Track::build_points_pub(&lp.points, &lp.labels, true, &dtm, 1.0, 2.0, 15.0, 3.5);
            for s in ss.split(',') {
                let i: usize = s.trim().parse().expect("station");
                let st = &tr.stations[i];
                println!("# station {i} {} E{:.1} N{:.1}", st.label, st.e, st.n);
                let mut row = String::new();
                let mut k = -90i32;
                while k <= 90 {
                    let off = k as f64 * 0.1;
                    let p = tr.offset(i, off);
                    let c = img.rgb(p[0], p[1]);
                    let ch = match c.map(imagery::classify) {
                        Some(imagery::Class::Asphalt) => '#',
                        Some(imagery::Class::Grass) => '.',
                        Some(imagery::Class::White) => 'W',
                        Some(imagery::Class::Dark) => 'd',
                        Some(imagery::Class::Dirt) => 'o',
                        Some(imagery::Class::Other) => '?',
                        None => ' ',
                    };
                    row.push(ch);
                    if k % 10 == 0 {
                        if let Some(c) = c {
                            row.push_str(&format!("({},{},{})", c[0], c[1], c[2]));
                        }
                    }
                    k += 1;
                }
                println!("{row}");
            }
        }
        Some("aerial-box") => {
            // circuit aerial-box OUT.png E0,N0,E1,N1 [--px 0.3] [--src esri|google|bing] [--zoom 19]
            //         [--osm dump.json --tag sport=karting] [--ways ID,ID,...  the chosen ways drawn thick white]
            let out = a.get(2).unwrap_or_else(|| usage());
            let bb: Vec<f64> = a.get(3).unwrap_or_else(|| usage()).split(',').map(|v| v.parse().expect("bbox")).collect();
            let px = flag(&a, "--px").unwrap_or(0.3);
            let src = a.iter().position(|x| x == "--src").and_then(|i| a.get(i + 1)).map(|s| aerial::Source::parse(s).unwrap_or_else(|| panic!("--src esri|google|bing, not {s:?}"))).unwrap_or(aerial::Source::Esri);
            let zoom = flag(&a, "--zoom").unwrap_or(19.0) as u32;
            let osm = a.iter().position(|x| x == "--osm").and_then(|i| a.get(i + 1)).map(|p| osm::load(Path::new(p)));
            let tag = a.iter().position(|x| x == "--tag").and_then(|i| a.get(i + 1)).cloned();
            let mut extra: Vec<(Vec<geo::Bng>, [u8; 3])> = Vec::new();
            if let (Some(ways), Some(list)) = (osm.as_ref(), a.iter().position(|x| x == "--ways").and_then(|i| a.get(i + 1))) {
                for id in list.split(',') {
                    let id: i64 = id.trim().parse().expect("way id");
                    let way = ways.ways.iter().find(|w| w.id == id).unwrap_or_else(|| panic!("no way {id} in the dump"));
                    extra.push((way.nodes.iter().filter_map(|n| ways.nodes.get(n).copied()).collect(), [255, 255, 255]));
                }
            }
            aerial::box_overlay((bb[0], bb[1], bb[2], bb[3]), px, osm.as_ref(), tag.as_deref(), Path::new(out), src, zoom, &extra).unwrap_or_else(|e| panic!("{e}"));
        }
        Some("map-head") => {
            for f in &a[2..] {
                map_head(Path::new(f));
            }
        }
        Some("chunk-hex") => {
            let f = a.get(2).unwrap_or_else(|| usage());
            let cid = u32::from_str_radix(a.get(3).unwrap_or_else(|| usage()).trim_start_matches("0x"), 16).expect("chunk id hex");
            let n = flag(&a, "--bytes").unwrap_or(256.0) as usize;
            chunk_hex(Path::new(f), cid, n);
        }
        Some("header-hex") => {
            let f = a.get(2).unwrap_or_else(|| usage());
            let n = flag(&a, "--bytes").unwrap_or(512.0) as usize;
            header_hex(Path::new(f), n);
        }
        Some("set-car") => {
            // circuit set-car IN.Map.Gbx OUT.Map.Gbx CarSnow|CarRally|CarDesert|CarSport
            let (inp, out, car) = (a.get(2).unwrap_or_else(|| usage()), a.get(3).unwrap_or_else(|| usage()), a.get(4).unwrap_or_else(|| usage()));
            let mut m = tmmaps::map::MapFile::load(Path::new(inp));
            println!("{inp}: player model {:?}", m.player_model());
            if a_has("--park-blocks") {
                for i in 0..m.blocks.len() {
                    m.move_block_cell(i, (0, 0, 0));
                    m.set_block_name(i, "RoadTechStraight");
                }
            }
            if car != "keep" {
                m.set_player_model(car, 10003, "Nadeo");
            }
            m.write_to(Path::new(out)).expect("write");
            let back = tmmaps::map::MapFile::load(Path::new(out));
            println!("{out}: player model {:?}, {} blocks, {} items, decoration {}", back.player_model(), back.blocks.len(), back.items.len(), back.decoration_id);
        }
        Some("body-hex") => {
            // circuit body-hex MAP [--bytes 256] [--at OFFSET]: the decompressed body's first bytes
            let f = a.get(2).unwrap_or_else(|| usage());
            let n = flag(&a, "--bytes").unwrap_or(256.0) as usize;
            let at = flag(&a, "--at").unwrap_or(0.0) as usize;
            let g = tmmaps::gbx::Gbx::parse(&std::fs::read(f).unwrap());
            let bytes = &g.body[at.min(g.body.len())..(at + n).min(g.body.len())];
            for (i, row) in bytes.chunks(16).enumerate() {
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                let asc: String = row.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                println!("{:06x}  {:<48} {}", at + i * 16, hex.join(" "), asc);
            }
            println!("(body {} bytes)", g.body.len());
        }
        Some("near") => {
            // circuit near surroundings.json E0 N0 X Z [R]
            let f = |i: usize| a.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or_else(|| usage());
            near(Path::new(a.get(2).unwrap_or_else(|| usage())), f(3), f(4), f(5), f(6), a.get(7).and_then(|v| v.parse().ok()).unwrap_or(15.0));
        }
        _ => usage(),
    }
}

fn a_has(name: &str) -> bool {
    std::env::args().any(|x| x == name)
}

fn a_all() -> Vec<String> {
    std::env::args().collect()
}

fn flag(a: &[String], name: &str) -> Option<f64> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok())
}

fn tiles(dir: &Path, prefix: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.starts_with(prefix) && n.ends_with(".tif")))
        .collect();
    v.sort();
    assert!(!v.is_empty(), "no {prefix}*.tif in {}", dir.display());
    v
}

/// Top-down picture: LIDAR ground as grey relief, the lap coloured by
/// height, both edges drawn, one tick every 100 m, the corner labels' first
/// stations marked.
fn preview(tr: &track::Track, dtm: &tiff::Mosaic, out: &Path, edges: Option<&edges::Edges>) {
    let (e0, n0, e1, n1) = tr.bbox();
    let margin = 120.0;
    let scale = 0.5; // px per metre
    let (e0, n0, e1, n1) = (e0 - margin, n0 - margin, e1 + margin, n1 + margin);
    let w = ((e1 - e0) * scale) as usize;
    let h = ((n1 - n0) * scale) as usize;
    let mut img = png::Image::new(w, h, [0, 0, 0]);
    // relief: shade by slope towards the north-west
    let (mut zmin, mut zmax) = (f64::MAX, f64::MIN);
    let mut heights = vec![f64::NAN; w * h];
    for y in 0..h {
        for x in 0..w {
            let e = e0 + (x as f64 + 0.5) / scale;
            let n = n1 - (y as f64 + 0.5) / scale;
            if let Some(z) = dtm.sample(e, n) {
                heights[y * w + x] = z;
                zmin = zmin.min(z);
                zmax = zmax.max(z);
            }
        }
    }
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let z = heights[y * w + x];
            if z.is_nan() {
                continue;
            }
            let dzdx = heights[y * w + x + 1] - heights[y * w + x - 1];
            let dzdy = heights[(y + 1) * w + x] - heights[(y - 1) * w + x];
            let shade = (0.55 + 0.35 * (dzdx - dzdy).clamp(-2.0, 2.0) / 2.0) * 255.0;
            let t = ((z - zmin) / (zmax - zmin).max(1e-6)) as f64;
            let base = [shade * (0.6 + 0.4 * t), shade * (0.7 + 0.2 * t), shade * (0.6 + 0.2 * (1.0 - t))];
            img.put(x as i64, y as i64, [base[0].clamp(0.0, 255.0) as u8, base[1].clamp(0.0, 255.0) as u8, base[2].clamp(0.0, 255.0) as u8]);
        }
    }
    let to_px = |e: f64, n: f64| ((e - e0) * scale, (n1 - n) * scale);
    let (tzmin, tzmax) = tr.stations.iter().fold((f64::MAX, f64::MIN), |(a, b), s| (a.min(s.z), b.max(s.z)));
    let n = tr.len();
    for i in 0..n {
        let st = &tr.stations[i];
        let (wl, wr) = match edges {
            Some(e) => (e.left[i], e.right[i]),
            None => (6.5, 6.5),
        };
        let l = tr.offset(i, wl);
        let r = tr.offset(i, -wr);
        let (lx, ly) = to_px(l[0], l[1]);
        let (rx, ry) = to_px(r[0], r[1]);
        img.line(lx, ly, rx, ry, png::heat((st.z - tzmin) / (tzmax - tzmin).max(1e-6)));
        if i % 100 == 0 {
            let (cx, cy) = to_px(st.e, st.n);
            img.disc(cx, cy, 2.5, [255, 255, 255]);
        }
        if i == 0 || tr.stations[i - 1].label != st.label {
            let (cx, cy) = to_px(st.e, st.n);
            img.disc(cx, cy, 4.0, [0, 0, 0]);
            img.disc(cx, cy, 2.5, [255, 0, 255]);
        }
    }
    img.save(out);
    // stats
    let len = n as f64 * tr.ds;
    println!("lap {len:.1} m at ds {} ({} stations); track height {tzmin:.2}..{tzmax:.2} m; ground in view {zmin:.1}..{zmax:.1} m", tr.ds, n);
    let (kmin, kmax) = tr.stations.iter().fold((0.0f64, 0.0f64), |(a, b), s| (a.min(s.curvature), b.max(s.curvature)));
    println!("curvature {kmin:.4}..{kmax:.4} 1/m (radius {:.0} m tightest)", 1.0 / kmin.abs().max(kmax.abs()));
    let (smin, smax) = tr.stations.iter().fold((0.0f64, 0.0f64), |(a, b), s| (a.min(s.cross_slope), b.max(s.cross_slope)));
    println!("cross-slope {:.1}%..{:.1}%", smin * 100.0, smax * 100.0);
    if let Some(e) = edges {
        let widths: Vec<f64> = (0..n).map(|i| e.left[i] + e.right[i]).collect();
        let (wmin, wmax) = widths.iter().fold((f64::MAX, f64::MIN), |(a, b), w| (a.min(*w), b.max(*w)));
        let wmean = widths.iter().sum::<f64>() / n as f64;
        println!("track width {wmin:.1}..{wmax:.1} m (mean {wmean:.1}); {} edge readings interpolated", e.guessed);
        let (kl, kr) = (e.kerb_left.iter().sum::<f64>() / n as f64, e.kerb_right.iter().sum::<f64>() / n as f64);
        println!("mean bright band beyond the edge: left {kl:.2} m, right {kr:.2} m");
    }
    let mut i = 0;
    while i < n {
        let label = &tr.stations[i].label;
        let mut j = i;
        let (mut zlo, mut zhi, mut kabs) = (f64::MAX, f64::MIN, 0.0f64);
        while j < n && &tr.stations[j].label == label {
            zlo = zlo.min(tr.stations[j].z);
            zhi = zhi.max(tr.stations[j].z);
            kabs = kabs.max(tr.stations[j].curvature.abs());
            j += 1;
        }
        println!("  {:>7.0} m  {:<19} {:>5} m  z {:.1}..{:.1}  r_min {:.0} m", tr.stations[i].s, label, ((j - i) as f64 * tr.ds) as i64, zlo, zhi, if kabs > 1e-6 { 1.0 / kabs } else { f64::INFINITY });
        i = j;
    }
}

/// Grey picture of one raster over a BNG box (percentile-stretched), with
/// the lap centreline overlaid when an Overpass dump is given.
fn raster_png(tif: &Path, out: &Path, bbox: (f64, f64, f64, f64), scale: f64, lap: Option<&osm::Loop>, tr_edges: Option<&(track::Track, edges::Edges)>) {
    let r = tiff::Raster::load(tif);
    let (e0, n0, e1, n1) = bbox;
    let w = ((e1 - e0) * scale) as usize;
    let h = ((n1 - n0) * scale) as usize;
    let mut vals: Vec<f32> = Vec::new();
    let mut grid = vec![f32::NAN; w * h];
    for y in 0..h {
        for x in 0..w {
            let e = e0 + (x as f64 + 0.5) / scale;
            let n = n1 - (y as f64 + 0.5) / scale;
            if let Some(v) = r.at(e, n) {
                grid[y * w + x] = v;
                vals.push(v);
            }
        }
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lo = vals[vals.len() * 2 / 100];
    let hi = vals[vals.len() * 98 / 100];
    let mut img = png::Image::new(w, h, [40, 0, 40]);
    let classify = a_has("--classify");
    for y in 0..h {
        for x in 0..w {
            let v = grid[y * w + x];
            if v.is_nan() {
                continue;
            }
            if classify {
                // asphalt < 190 dark grey; 190..300 "gravel?" orange; grass green; >600 bright white (roofs/paint)
                let c = if (v as f64) < terrain::ASPHALT_MAX { [70, 70, 75] } else if v < 300.0 { [220, 140, 40] } else if v < 600.0 { [60, 150, 50] } else { [240, 240, 240] };
                img.put(x as i64, y as i64, c);
                continue;
            }
            let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
            let g = (t * 255.0) as u8;
            img.put(x as i64, y as i64, [g, g, g]);
        }
    }
    if let Some(lp) = lap {
        for (i, p) in lp.points.iter().enumerate() {
            let q = lp.points[(i + 1) % lp.points.len()];
            img.line((p.e - e0) * scale, (n1 - p.n) * scale, (q.e - e0) * scale, (n1 - q.n) * scale, [255, 40, 40]);
        }
    }
    if let Some((tr, ed)) = tr_edges {
        let n = tr.len();
        for i in 0..n {
            let j = (i + 1) % n;
            for (side, col) in [(1.0, [40, 255, 40]), (-1.0, [40, 160, 255])] {
                let (wi, wj) = if side > 0.0 { (ed.left[i], ed.left[j]) } else { (ed.right[i], ed.right[j]) };
                let p = tr.offset(i, side * wi);
                let q = tr.offset(j, side * wj);
                img.line((p[0] - e0) * scale, (n1 - p[1]) * scale, (q[0] - e0) * scale, (n1 - q[1]) * scale, col);
                // kerb band outer limit in yellow
                let (ki, kj) = if side > 0.0 { (ed.kerb_left[i], ed.kerb_left[j]) } else { (ed.kerb_right[i], ed.kerb_right[j]) };
                if ki > 0.3 {
                    let p = tr.offset(i, side * (wi + ki));
                    let q = tr.offset(j, side * (wj + kj));
                    img.line((p[0] - e0) * scale, (n1 - p[1]) * scale, (q[0] - e0) * scale, (n1 - q[1]) * scale, [255, 220, 40]);
                }
            }
        }
    }
    img.save(out);
    println!("{}: {}x{} px, stretch {lo}..{hi}", out.display(), w, h);
}

/// Transverse intensity profiles at chosen stations, for calibrating the
/// edge finder: one row per 0.5 m lateral offset (left positive).
fn profiles(tr: &track::Track, inten: &tiff::Raster, dtm: &tiff::Mosaic, at: &[f64]) {
    for &s in at {
        let i = ((s / tr.ds).round() as usize).min(tr.len() - 1);
        let st = &tr.stations[i];
        println!("# station {i} s={:.0} {} heading {:.1} deg  (offset:intensity/ground cm relative to the centreline)", st.s, st.label, st.heading.to_degrees());
        let z0 = dtm.sample(st.e, st.n).unwrap_or(0.0);
        let mut row = String::new();
        let mut k = -60;
        while k <= 60 {
            let off = k as f64 * 0.5;
            let p = tr.offset(i, off);
            let v = inten.sample(p[0], p[1]).map(|v| format!("{v:.0}")).unwrap_or("-".into());
            let h = dtm.sample(p[0], p[1]).map(|z| format!("{:+.0}", (z - z0) * 100.0)).unwrap_or("-".into());
            row.push_str(&format!("{off:+.1}:{v}/{h} "));
            k += 1;
        }
        println!("{row}");
    }
}

/// The whole pipeline: data -> track -> items -> map.
#[allow(clippy::too_many_arguments)]
fn build(osm_path: &Path, dtm_dir: &Path, tif: &Path, host: &Path, out: &Path, seg: f64, cp_spacing: f64, half: f64, surroundings: Option<&Path>, variant: &str, limit: Option<usize>) {
    let ways = osm::load(osm_path);
    let around = surroundings.map(osm::load);
    let lp = osm::gp_loop(&ways);
    let dtm = tiff::Mosaic::load(&tiles(dtm_dir, "dtm_"));
    let tr = track::Track::build(&lp, &dtm, 1.0, 3.0, 8.0, 6.0);
    let inten = tiff::Raster::load(tif);
    let ed = edges::Edges::from_intensity(&tr, &inten);
    println!("lap {:.1} m, {} stations, {} edge readings interpolated", tr.len() as f64 * tr.ds, tr.len(), ed.guessed);
    // Frame: the lap centred in the host's grid (or in a grid grown to fit
    // it, --resize), heights 4 m above the stadium floor (y 8).
    let (e0, n0, e1, n1) = tr.bbox();
    let host_size = tmmaps::map::MapFile::load(host).size;
    let margin = 200.0;
    let need_x = (((e1 - e0 + 2.0 * margin) / 32.0).ceil() as i32).max(48);
    let need_z = (((n1 - n0 + 2.0 * margin) / 32.0).ceil() as i32).max(48);
    let resize = a_has("--resize");
    let (sx, sz) = if resize { (need_x, need_z) } else { (host_size[0], host_size[2]) };
    assert!(sx >= need_x && sz >= need_z, "host grid {host_size:?} is too small for the lap ({need_x} x {need_z} blocks needed); pass --resize");
    let zmin = tr.stations.iter().map(|s| s.z).fold(f64::MAX, f64::min);
    let (ce, cn) = ((e0 + e1) / 2.0, (n0 + n1) / 2.0);
    let fr = mapbuild::Frame { e0: ce - (sx as f64) * 16.0, n0: cn + (sz as f64) * 16.0, z_ref: zmin - 12.0 };
    println!("map size {sx} x {} x {sz} blocks ({} x {} m); origin E{:.0} N{:.0}; track y {:.1}..{:.1}", host_size[1], sx * 32, sz * 32, fr.e0, fr.n0, 12.0, tr.stations.iter().map(|s| s.z).fold(f64::MIN, f64::max) - fr.z_ref);
    // Start line: in front of the Wing (OSM building), else station 0.
    // The start and finish lines: OSM's `raceway=start` / `raceway=finish`
    // nodes when the dump has them (Silverstone's are ~150 m apart, the
    // finish line downstream of the start line), else the Wing's centroid.
    let nearest = |b: geo::Bng| tr.stations.iter().enumerate().map(|(i, s)| (i, (s.e - b.e).powi(2) + (s.n - b.n).powi(2))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|(i, d)| (i, d.sqrt())).unwrap();
    let osm_start = ways.tagged_node("raceway", "start").map(nearest);
    let osm_finish = ways.tagged_node("raceway", "finish").map(nearest);
    if let Some((i, d)) = osm_start {
        println!("OSM start line -> station {i} ({}), {d:.1} m off the centreline", tr.stations[i].label);
    }
    if let Some((i, d)) = osm_finish {
        println!("OSM finish line -> station {i} ({}), {d:.1} m off the centreline", tr.stations[i].label);
    }
    let start = osm_start.map(|(i, _)| i).or_else(|| around.as_ref().and_then(|w| start_station(&tr, w))).unwrap_or(0);
    let mut plan = mapbuild::plan_waypoints(&tr, start, cp_spacing);
    if let Some((f, _)) = osm_finish {
        plan.finish = f;
    }
    println!("start at s={} ({}), finish at s={}, {} checkpoints at {:?}", start, tr.stations[start].label, plan.finish, plan.checkpoints.len(), plan.checkpoints.iter().map(|&c| format!("{}:{}", tr.stations[c].s as i64, tr.stations[c].label.split(' ').next().unwrap_or(""))).collect::<Vec<_>>());
    // which shoulders are run-off asphalt: the intensity 0.75 m beyond the kerb
    let shoulder_asphalt = |k: usize, left: bool| -> bool {
        let k1 = (k + 1) % tr.len();
        let (edge, kerb) = if left { (ed.left[k], terrain::kerb_width(ed.kerb_left[k], ed.kerb_left[k1])) } else { (ed.right[k], terrain::kerb_width(ed.kerb_right[k], ed.kerb_right[k1])) };
        let off = edge + kerb + 0.75;
        let w = tr.offset(k, if left { off } else { -off });
        inten.at(w[0], w[1]).map(|v| (v as f64) < terrain::ASPHALT_MAX).unwrap_or(false)
    };
    // The roads' footprint: the lap first, then every other piece of tarmac
    // at the venue clipped against what came before (roads.rs). The map
    // box is where terrain lives; the ribbon spans it.
    let (de0, dn0, de1, dn1) = dtm.bounds();
    let map_box = ((fr.e0).max(de0), (fr.n0 - sz as f64 * 32.0).max(dn0), (fr.e0 + sx as f64 * 32.0).min(de1), (fr.n0).min(dn1));
    let lap_ribbon = terrain::Ribbon::build(&tr, &ed, map_box, 40.0);
    let mut union = terrain::Ribbon::build(&tr, &ed, map_box, 40.0);
    let extra_roads = if a_has("--no-extra-roads") { Vec::new() } else { roads::extra_roads(&ways, &lap_ribbon, &mut union, &dtm, &inten) };
    println!("{} extra roads (pit lanes, other layouts, link roads)", extra_roads.len());
    // the lap's shoulders: none where another road meets the lap flush
    let others = {
        let mut r = terrain::Ribbon::new(map_box);
        for road in &extra_roads {
            r.add(&road.track, &road.edges, 10.0);
        }
        r
    };
    let shoulder_sides = |k: usize| -> (bool, bool) {
        let k1 = (k + 1) % tr.len();
        let side = |left: bool| -> bool {
            let (edge, kerb) = if left { (ed.left[k], terrain::kerb_width(ed.kerb_left[k], ed.kerb_left[k1])) } else { (ed.right[k], terrain::kerb_width(ed.kerb_right[k], ed.kerb_right[k1])) };
            let off = edge + kerb + 0.75;
            let w = tr.offset(k, if left { off } else { -off });
            others.beyond_kerb(w[0], w[1]) >= 0.0
        };
        (side(true), side(false))
    };
    let own_height = |_k: usize, _off: f64, own: f64| own;
    let style = mapbuild::RoadStyle { shoulder_asphalt: &shoulder_asphalt, shoulder_sides: &shoulder_sides, height: &own_height };
    let (mut items, skip) = mapbuild::waypoint_items(&tr, &ed, &fr, &plan, half, &style);
    let mut roads = mapbuild::road_items(&tr, &ed, &fr, seg, &skip, &style, "Road");
    println!("{} waypoint items, {} road items", items.len(), roads.len());
    // the extra roads, each blended onto everything built before it
    {
        let mut before = terrain::Ribbon::build(&tr, &ed, map_box, 40.0);
        for (idx, road) in extra_roads.iter().enumerate() {
            roads.extend(roads::road_items_for(road, idx, &fr, &before, &inten, &dtm, None));
            before.add(&road.track, &road.edges, 40.0);
        }
    }
    // the venue: the lap's box plus 220 m, where terrain and surroundings live
    let venue = (e0 - 220.0, n0 - 220.0, e1 + 220.0, n1 + 220.0);
    let mut extras: Vec<mapbuild::Placement> = Vec::new();
    if !a_has("--no-terrain") {
        let spec = terrain::TerrainSpec { bbox: venue, coarse_bbox: map_box, coarse: flag(&a_all(), "--coarse").unwrap_or(32.0), fine: flag(&a_all(), "--fine").unwrap_or(4.0), near: flag(&a_all(), "--near").unwrap_or(30.0), tile: 256.0 };
        extras.extend(terrain::terrain_items(&union, &dtm, &inten, &fr, &spec, None));
    }
    if let Some(w) = around.as_ref() {
        if !a_has("--no-buildings") {
            let dsm = tiff::Mosaic::load(&tiles(dtm_dir, "dsm_"));
            let structs = buildings::structures(w, &dtm, &dsm, venue, &tr, &ed, &union);
            let n_stand = structs.iter().filter(|s| s.kind == "grandstand").count();
            println!("{} structures ({n_stand} grandstands); tallest {:.0} m", structs.len(), structs.iter().map(|s| s.height).fold(0.0, f64::max));
            extras.extend(buildings::building_items(&structs, &fr));
            extras.extend(buildings::linear_items(w, &dtm, &dsm, &ed, &fr, venue, &tr, 60.0, &union));
        }
    }
    println!("{} terrain/surroundings items", extras.len());
    match variant {
        "size" => items.clear(),
        "roads" => items = roads.clone(),
        "nowp" => {
            for it in &mut items {
                it.tag = None;
            }
            items.extend(roads);
        }
        _ => {
            items.extend(roads);
            items.extend(extras);
        }
    }
    if let Some(l) = limit {
        items.truncate(l);
    }
    let size = if resize { Some([sx, host_size[1], sz]) } else { None };
    println!("variant {variant}: {} items", items.len());
    let total_bytes: usize = items.iter().map(|p| p.bytes.len()).sum();
    println!("items total {:.1} MB", total_bytes as f64 / 1e6);
    let mut per: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for it in &items {
        let stem = it.ident.rsplit('\\').next().unwrap_or(&it.ident);
        let key: String = stem.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let e = per.entry(key).or_default();
        e.0 += 1;
        e.1 += it.bytes.len();
    }
    for (k, (n, b)) in &per {
        println!("  {k:<12} {n:>4} items {:>6.2} MB", *b as f64 / 1e6);
    }
    // The self-check: nothing above the road, no holes, no launching crests.
    let report = check::run(&items, &tr, &ed, &fr, 60.0);
    check::print(&report);
    for road in &extra_roads {
        let r = check::run_road(&items, road, &fr);
        if !r.offenders.is_empty() {
            println!("check: on {}:", road.name);
            check::print(&r);
        }
    }
    if report.fatal() && !a_has("--allow-defects") {
        eprintln!("circuit: the map has surface defects on the lap; not written (pass --allow-defects to write it anyway)");
        std::process::exit(2);
    }
    let uid = mapbuild::map_uid(&items);
    println!("uid {uid} (hash of every placement's collision + waypoint physics)");
    // --author-ms N: the validated lap time -> medals + validated flag
    let author_ms = flag(&a_all(), "--author-ms").map(|v| v as u32);
    mapbuild::assemble(host, out, &items, size, &uid, author_ms);
    // The lap as the map has it, for whoever drives it: one row per metre
    // in TM world coordinates, plus the waypoint plan.
    let line_path = out.with_extension("line.tsv");
    let mut w = String::from("# Silverstone lap in Trackmania world coordinates (x east, y up, z south); heading = atan2(dx, dz) of travel; curvature >0 turns left\n");
    w.push_str(&format!("# start_station\t{}\tfinish_station\t{}\tcheckpoints\t{}\n", plan.start, plan.finish, plan.checkpoints.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")));
    w.push_str("station\ts_m\tx\ty\tz\theading_rad\tcurvature\tleft_m\tright_m\tcorner\n");
    for (i, st) in tr.stations.iter().enumerate() {
        let p = fr.to_tm(st.e, st.n, st.z);
        let (sh, ch) = st.heading.sin_cos();
        let heading_tm = mapbuild::yaw_for(ch as f32, -sh as f32);
        w.push_str(&format!("{i}\t{:.1}\t{:.3}\t{:.3}\t{:.3}\t{:.5}\t{:.5}\t{:.2}\t{:.2}\t{}\n", st.s, p[0], p[1], p[2], heading_tm, st.curvature, ed.left[i], ed.right[i], st.label));
    }
    std::fs::write(&line_path, w).expect("line tsv");
    println!("lap written to {}", line_path.display());
}

/// The station nearest the centroid of the OSM building named "Silverstone
/// Wing" (the pit building): the start/finish line sits in front of it.
fn start_station(tr: &track::Track, ways: &osm::Ways) -> Option<usize> {
    let wing = ways.ways.iter().find(|w| w.name == "Silverstone Wing")?;
    let pts: Vec<_> = wing.nodes.iter().filter_map(|id| ways.nodes.get(id)).collect();
    if pts.is_empty() {
        return None;
    }
    let ce = pts.iter().map(|p| p.e).sum::<f64>() / pts.len() as f64;
    let cn = pts.iter().map(|p| p.n).sum::<f64>() / pts.len() as f64;
    let (i, d) = tr.stations.iter().enumerate().map(|(i, s)| (i, (s.e - ce).powi(2) + (s.n - cn).powi(2))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())?;
    println!("Wing centroid E{ce:.0} N{cn:.0} -> station {i} ({}), {:.0} m away", tr.stations[i].label, d.sqrt());
    Some(i)
}

/// The blocks chunk's head (uid, name, decoration, size) read raw, for maps
/// tmmaps refuses (e.g. free blocks without their chunk).
fn map_head(path: &Path) {
    let g = tmmaps::gbx::Gbx::parse(&std::fs::read(path).unwrap());
    let body = &g.body;
    let needle = 0x0304301Fu32.to_le_bytes();
    let mut at = None;
    for i in 0..body.len() - 4 {
        if body[i..i + 4] == needle {
            at = Some(i);
            break;
        }
    }
    let Some(at) = at else {
        println!("{}: no blocks chunk", path.display());
        return;
    };
    let mut o = at + 4;
    let rd = |o: &mut usize| -> u32 {
        let v = u32::from_le_bytes(body[*o..*o + 4].try_into().unwrap());
        *o += 4;
        v
    };
    let mut table: Vec<String> = Vec::new();
    let mut first = true;
    let mut id = |o: &mut usize, table: &mut Vec<String>| -> String {
        if first {
            let v = u32::from_le_bytes(body[*o..*o + 4].try_into().unwrap());
            if v == 3 {
                *o += 4;
            }
            first = false;
        }
        let idx = rd(o);
        if idx == 0xFFFF_FFFF {
            return "<null>".into();
        }
        if idx & 0xC000_0000 == 0 {
            return format!("#{idx}");
        }
        if idx & 0x3FFF_FFFF == 0 {
            let n = rd(o) as usize;
            let s = String::from_utf8_lossy(&body[*o..*o + n]).to_string();
            *o += n;
            table.push(s.clone());
            return s;
        }
        table.get((idx & 0x3FFF_FFFF) as usize - 1).cloned().unwrap_or_else(|| format!("<ref {}>", idx & 0x3FFF_FFFF))
    };
    let uid = id(&mut o, &mut table);
    let coll = id(&mut o, &mut table);
    let author = id(&mut o, &mut table);
    let n = rd(&mut o) as usize;
    let name = String::from_utf8_lossy(&body[o..o + n]).to_string();
    o += n;
    let deco = id(&mut o, &mut table);
    let deco_coll = id(&mut o, &mut table);
    let deco_author = id(&mut o, &mut table);
    let size = [rd(&mut o), rd(&mut o), rd(&mut o)];
    let need_unlock = rd(&mut o);
    let version = rd(&mut o);
    let nb = rd(&mut o);
    println!("{}: uid {uid} coll {coll} author {author} name {name:?}; deco {deco} / {deco_coll} / {deco_author}; size {size:?} needUnlock {need_unlock} v{version} blocks {nb}", path.display());
    for (cid, off, payload, size) in tmmaps::gbx::all_skip_chunks(body) {
        let _ = (off, payload);
        println!("   chunk {cid:08X} {size} bytes");
    }
}

/// Hex of the first bytes of a skippable chunk's payload.
fn chunk_hex(path: &Path, cid: u32, n: usize) {
    let g = tmmaps::gbx::Gbx::parse(&std::fs::read(path).unwrap());
    for (c, _off, payload, size) in tmmaps::gbx::all_skip_chunks(&g.body) {
        if c == cid {
            let end = (payload + n).min(payload + size);
            let bytes = &g.body[payload..end];
            for (i, row) in bytes.chunks(16).enumerate() {
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                let asc: String = row.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                println!("{:06x}  {:<48} {}", i * 16, hex.join(" "), asc);
            }
            println!("(chunk {cid:08X}: {size} bytes)");
        }
    }
}

/// Hex of the header user data (all header chunks).
fn header_hex(path: &Path, n: usize) {
    let g = tmmaps::gbx::Gbx::parse(&std::fs::read(path).unwrap());
    let bytes = &g.user_data[..n.min(g.user_data.len())];
    for (i, row) in bytes.chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
        let asc: String = row.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
        println!("{:06x}  {:<48} {}", i * 16, hex.join(" "), asc);
    }
    println!("(user data {} bytes)", g.user_data.len());
}

/// OSM ways of the surroundings dump within `r` metres of a TM world point
/// (x, z) of the last build's frame — what is standing where the car stopped.
fn near(osm: &Path, e0: f64, n0: f64, x: f64, z: f64, r: f64) {
    let w = osm::load(osm);
    let (e, n) = (x + e0, n0 - z);
    println!("world ({x:.1}, {z:.1}) = E{e:.1} N{n:.1}");
    for way in &w.ways {
        let pts: Vec<_> = way.nodes.iter().filter_map(|id| w.nodes.get(id)).collect();
        let mut best = f64::MAX;
        for k in 1..pts.len() {
            let (a, b) = (pts[k - 1], pts[k]);
            let (dx, dy) = (b.e - a.e, b.n - a.n);
            let l2 = dx * dx + dy * dy;
            let t = if l2 > 0.0 { (((e - a.e) * dx + (n - a.n) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let (px, py) = (a.e + t * dx, a.n + t * dy);
            best = best.min(((e - px).powi(2) + (n - py).powi(2)).sqrt());
        }
        if best <= r {
            println!("  {:6.1} m  way {} {:?} {} nodes  tags {:?}", best, way.id, way.name, pts.len(), way.tags);
        }
    }
}

//! A test map for LOOSE items (files under Documents/Trackmania/Items, not
//! embedded): every item record of the host map parked far away, then one
//! record per placement spec re-pointed at an item of the set — the way a
//! mapper's saved map references a set item, and the in-game check that a
//! `TinyBlocks\…\X.Item.Gbx` file resolves by its path, draws where its
//! pivot says, and tiles against its neighbours.
//!
//! usage: item_set_map HOST.Map.Gbx OUT.Map.Gbx AUTHOR SPEC...
//!   SPEC = [AUTHOR::]IDENT@x,y,z[/yaw_deg[,pitch_deg[,roll_deg]]][/px,py,pz][/scale][#Spawn|Checkpoint|Goal][*N[:dx,dy,dz]]
//!          (pivot default 8,0,8: a 1x1 tiny block's centre;
//!          x,y,z is the record position P; the game puts the item at P + R(p + V) — at yaw 0 the
//!          piece's corner is P + pivot;
//!          the 4th slash field is the placement scale (`/0/0,0,0/0.5`);
//!          the #tag is the placement's waypoint property — a start/CP/finish item needs one;
//!          `*N:dx,dy,dz` repeats the placement N times, each copy offset by (dx,dy,dz) from the
//!          previous one — a PILE of overlapping checkpoints is `…#Checkpoint*50:0.05,0,0.04`;
//!          `Nadeo::GateCheckpointCenter8mv2` is a STOCK pack item, not a file under Items/)
//!   e.g. 'TinyBlocks\Roads\RoadTech\Main\Main\RoadTechStraight.Item.Gbx@408,8,408'
//! The host must have at least as many item records as placements.
use std::{env, path::Path};
use tmmaps::map::MapFile;

/// One parsed spec, before the `*N` repeat is expanded.
struct Spec {
    ident: String,
    author: String,
    pos: [f32; 3],
    angles: [f32; 3],
    pivot: [f32; 3],
    scale: f32,
    tag: Option<String>,
    copies: usize,
    step: [f32; 3],
}

fn parse_spec(raw: &str, default_author: &str) -> Spec {
    // the `*N[:dx,dy,dz]` repeat rides at the very end, after any #tag
    let (body, repeat) = match raw.rsplit_once('*') {
        Some((b, r)) if !r.is_empty() && r.chars().next().is_some_and(|c| c.is_ascii_digit()) => (b, Some(r)),
        _ => (raw, None),
    };
    let (copies, step) = match repeat {
        Some(r) => {
            let (n, off) = r.split_once(':').unwrap_or((r, "0,0,0"));
            let n: usize = n.parse().unwrap_or_else(|_| {
                eprintln!("spec {raw}: *N wants a count");
                std::process::exit(2);
            });
            let v: Vec<f32> = off.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() != 3 {
                eprintln!("spec {raw}: *N:dx,dy,dz wants three offsets");
                std::process::exit(2);
            }
            (n.max(1), [v[0], v[1], v[2]])
        }
        None => (1, [0.0; 3]),
    };
    let (body, tag) = match body.split_once('#') {
        Some((s, t)) => (s, Some(t.to_string())),
        None => (body, None),
    };
    let (ident, rest) = body.split_once('@').unwrap_or_else(|| {
        eprintln!("spec {raw}: no '@'");
        std::process::exit(2);
    });
    // `AUTHOR::IDENT` overrides the map-wide author for ONE record — a STOCK
    // pack item is (name without `.Item.Gbx`, collection 26, author `Nadeo`),
    // so a map can hold the game's own 8 m gate next to our loose copy of it
    // (2026-10-17, the Editor++ find/replace test).
    let (ident, author) = match ident.split_once("::") {
        Some((a, id)) => (id.to_string(), a.to_string()),
        None => (ident.to_string(), default_author.to_string()),
    };
    let mut parts = rest.split('/');
    let pos: Vec<f32> = parts.next().unwrap_or("").split(',').filter_map(|x| x.parse().ok()).collect();
    if pos.len() != 3 {
        eprintln!("spec {raw}: position needs x,y,z");
        std::process::exit(2);
    }
    // yaw[,pitch[,roll]] in degrees
    let angles: Vec<f32> = parts.next().map(|y| y.split(',').filter_map(|v| v.parse().ok()).collect()).unwrap_or_default();
    let pivot: Vec<f32> = parts.next().map(|p| p.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|| vec![8.0, 0.0, 8.0]);
    // the 4th field is the placement SCALE (the anchored object's own scale
    // word): `…/0/0,0,0/0.5` puts a half-size copy of the item there — how
    // small a gate's trigger can get without a new bake (2026-10-17: it does
    // not get smaller at all, only the model does)
    let scale: f32 = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
    Spec {
        ident,
        author,
        pos: [pos[0], pos[1], pos[2]],
        angles: [angles.first().copied().unwrap_or(0.0), angles.get(1).copied().unwrap_or(0.0), angles.get(2).copied().unwrap_or(0.0)],
        pivot: [pivot[0], pivot[1], pivot[2]],
        scale,
        tag,
        copies,
        step,
    }
}

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: item_set_map HOST OUT AUTHOR SPEC...");
        std::process::exit(2);
    }
    let (host, out, author) = (&a[1], &a[2], &a[3]);
    let specs: Vec<Spec> = a[4..].iter().map(|s| parse_spec(s, author)).collect();
    let total: usize = specs.iter().map(|s| s.copies).sum();
    let mut tags: Vec<(usize, String)> = Vec::new();
    // The records used are CLONES appended to the host (`append_item_clones`,
    // written and reloaded first): uniform records from one donor. Re-pointing
    // the host's own records (2026-09-23, TinySet11/12) gave every item but
    // the first NO collision — those U10S records carry per-placement state
    // (snap groups, flags) the game reads.
    let base = MapFile::load(Path::new(host));
    let first = base.items.len();
    let grown = Path::new(out).with_extension("grown.Map.Gbx");
    let mut mg = base;
    mg.append_item_clones(first + total);
    mg.write_to(&grown).unwrap();
    let mut m = MapFile::load(&grown);
    if m.items.len() < first + total {
        eprintln!("host grew to {} item records, {} needed", m.items.len(), first + total);
        std::process::exit(1);
    }
    m.set_map_uid(&format!("Set{:024}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() % 10u128.pow(24)));
    let cell = |p: [f32; 3]| ((p[0] / 32.0) as i32, ((p[1] + 64.0) / 8.0) as i32, (p[2] / 32.0) as i32);
    for i in 0..m.items.len() {
        m.move_item_pos(i, [16.0, -1000.0, 16.0]);
    }
    let mut i = first;
    for s in &specs {
        for k in 0..s.copies {
            let pos = [s.pos[0] + s.step[0] * k as f32, s.pos[1] + s.step[1] * k as f32, s.pos[2] + s.step[2] * k as f32];
            m.move_item(i, pos, s.angles[0].to_radians(), cell(pos));
            m.set_item_frame(i, [s.angles[0].to_radians(), s.angles[1].to_radians(), s.angles[2].to_radians()], s.pivot);
            m.set_item_scale(i, s.scale);
            m.set_item_model(i, &s.ident);
            m.set_item_author(i, &s.author);
            // the donor's variant byte (the high byte of the placement flags: a
            // Nadeo vegetation variant index) means nothing on our items and broke
            // the placement (2026-09-23: records with 0x300 had no collision, a
            // start with one hung the game on "Updating data...")
            m.clear_item_variant(i);
            if let Some(t) = &s.tag {
                tags.push((i, t.clone()));
            }
            if s.copies == 1 || k == 0 || k + 1 == s.copies {
                println!(
                    "item {i}: {} ({}) at {pos:?} yaw {} pivot {:?} scale {}{}{}",
                    s.ident,
                    s.author,
                    s.angles[0],
                    s.pivot,
                    s.scale,
                    s.tag.as_ref().map(|t| format!(" waypoint {t}")).unwrap_or_default(),
                    if s.copies > 1 { format!(" [copy {}/{}]", k + 1, s.copies) } else { String::new() }
                );
            }
            i += 1;
        }
    }
    let stage = Path::new(out).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    // the waypoint properties are variable-length splices: a second pass over
    // the written file, after the model renames
    let mut m2 = MapFile::load(&stage);
    for (i, t) in &tags {
        m2.set_item_waypoint(*i, Some(t), 0);
    }
    m2.remove_password();
    m2.write_to(Path::new(out)).unwrap();
    let _ = std::fs::remove_file(stage);
    let _ = std::fs::remove_file(grown);
    println!("test map: {out} with {total} loose items on cloned records {first}.. (author {author})");
}

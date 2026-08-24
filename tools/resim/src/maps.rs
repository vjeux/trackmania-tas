//! The map registry: how a fresh box gets the maps back without the repo ever
//! carrying them.
//!
//! **Ruling, 2026-08-24: Nadeo's map files do not go in the public repo.**
//! They are not ours to redistribute. What goes in instead is everything
//! needed to *re-obtain* one and prove it came back the same:
//!
//! | field | why it is here |
//! |---|---|
//! | `uid` | the map's own identity, out of its header — the key every fetch route takes |
//! | `name`, `author_ms` | so a human reading the registry knows what the row is; the author time is a number **in the map file**, which is what makes it a legitimate yardstick under the no-ghost rule |
//! | `url` | the exact documented GET route, from `ACQUISITION.md` |
//! | `md5`, `bytes` | the control. A map that comes back different is a fact worth knowing, not a detail |
//! | `spawn_x`, `spawn_z` | the start line, for the start-position check |
//! | `cps` | checkpoint count, so a verdict of `cps 3` can be read against something |
//!
//! The hash is doing two jobs and the second is the more interesting one. It
//! makes recovery deterministic, and it makes "the map I am driving is the map
//! the result was measured on" checkable — which matters because this project
//! has a history of measurements made against a file nobody re-identified.
//!
//! **The vertical coordinate is deliberately not here.** A spawn read from a
//! block gives its cell, and `world_y = 8*cy + yoff` where `yoff` is a
//! property of the map's *decoration* that has to be fitted per map
//! (`mapgeom::place::Yoff`). `x` and `z` are exact — `32*c + 16` — and the
//! check that uses them is horizontal for that reason, stated rather than
//! quietly fudged.

use haul::md5::md5_hex;
use haul::rec::Rec;
use std::path::{Path, PathBuf};

/// Metres per cell in x and z; the centre of a cell is `32*c + 16`.
const CELL_XZ: f64 = 32.0;

#[derive(Debug, Clone, PartialEq)]
pub struct MapRow {
    pub id: String,
    /// The file's own basename. A map directory routinely holds SEVERAL
    /// `.Map.Gbx` files — the pristine map plus the segment, detector and
    /// wall-removed variants this project cuts from it — so a row that named
    /// only the directory described whichever one the scanner happened to
    /// pick, and the verifier compared it against whichever one IT picked.
    /// Two maps read as CHANGED for that reason before the file was pinned.
    pub file: String,
    pub uid: String,
    pub name: String,
    pub author_ms: Option<i64>,
    pub md5: String,
    pub bytes: u64,
    pub spawn: Option<(f64, f64)>,
    pub cps: usize,
    pub url: String,
}

impl MapRow {
    pub fn to_rec(&self) -> Rec {
        let mut r = Rec::new("map")
            .f("id", &self.id)
            .f("file", &self.file)
            .f("uid", &self.uid)
            .f("name", &self.name)
            .f("md5", &self.md5)
            .f("bytes", self.bytes)
            .f("cps", self.cps)
            .f("url", &self.url);
        match self.author_ms {
            Some(ms) => r.set("author_ms", ms),
            None => r.set("author_ms", "unknown"),
        }
        match self.spawn {
            Some((x, z)) => {
                r.set("spawn_x", x);
                r.set("spawn_z", z);
            }
            None => r.set("spawn", "unknown"),
        }
        r
    }

    pub fn from_rec(r: &Rec) -> Option<MapRow> {
        Some(MapRow {
            id: r.get("id")?.to_string(),
            file: r.get("file").unwrap_or("").to_string(),
            uid: r.get("uid").unwrap_or("").to_string(),
            name: r.get("name").unwrap_or("").to_string(),
            author_ms: r.get_i64("author_ms"),
            md5: r.get("md5").unwrap_or("").to_string(),
            bytes: r.get_u64("bytes").unwrap_or(0),
            spawn: match (r.get_f64("spawn_x"), r.get_f64("spawn_z")) {
                (Some(x), Some(z)) => Some((x, z)),
                _ => None,
            },
            cps: r.get("cps").and_then(|v| v.parse().ok()).unwrap_or(0),
            url: r.get("url").unwrap_or("").to_string(),
        })
    }
}

/// The documented resolver: `trackmania.io/api/map/<uid>` carries `fileUrl`,
/// which is the Nadeo core endpoint for the bytes. `ACQUISITION.md` §"fetching
/// a map". GET only, rate-limited, descriptive User-Agent — as every fetch on
/// this project is.
pub fn resolver_url(uid: &str) -> String {
    if uid.is_empty() || uid == "-" {
        return "unknown — the file declares no uid".to_string();
    }
    format!("https://trackmania.io/api/map/{uid}")
}

/// The centre of a cell, in world metres. Only x and z: see the module note.
pub fn cell_centre_xz(cx: i32, cz: i32) -> (f64, f64) {
    (CELL_XZ * cx as f64 + CELL_XZ / 2.0, CELL_XZ * cz as f64 + CELL_XZ / 2.0)
}

/// Read one map file into a registry row.
pub fn read_map(path: &Path, id: &str) -> Result<MapRow, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let md5 = md5_hex(&bytes);
    let h = tmmaps::header::read(&path.to_string_lossy())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let m = tmmaps::map::MapFile::try_load(path)?;

    let wps = m.waypoints();
    let spawn = wps.iter().find(|w| w.tag == "Spawn").map(|w| match w.pos {
        // A free item carries its exact position; a block carries only its
        // cell, and the cell centre is the honest answer for one.
        Some(p) => (p[0] as f64, p[2] as f64),
        None => cell_centre_xz(w.coords.0, w.coords.2),
    });
    let cps = wps.iter().filter(|w| w.tag == "Checkpoint").count();

    Ok(MapRow {
        id: id.to_string(),
        file: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        uid: h.uid.clone(),
        name: if h.name == "-" || h.name.is_empty() { id.to_string() } else { h.name.clone() },
        author_ms: h.authortime.trim().parse().ok(),
        md5,
        bytes: bytes.len() as u64,
        spawn,
        cps,
        url: resolver_url(&h.uid),
    })
}

/// Every map under the given corpus roots, in a stable order.
pub fn scan(corpus: &[PathBuf]) -> Result<Vec<MapRow>, String> {
    let mut rows: Vec<MapRow> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    for root in corpus {
        if !root.is_dir() {
            continue;
        }
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
            .map_err(|e| format!("{}: {e}", root.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        dirs.sort();
        for d in dirs {
            let candidates: Vec<PathBuf> = if d.is_dir() {
                std::fs::read_dir(&d)
                    .map_err(|e| format!("{}: {e}", d.display()))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.to_string_lossy().ends_with(".Map.Gbx"))
                    .collect()
            } else if d.to_string_lossy().ends_with(".Map.Gbx") {
                vec![d.clone()]
            } else {
                continue;
            };
            for c in candidates {
                let id = if d.is_dir() {
                    d.file_name().unwrap_or_default().to_string_lossy().to_string()
                } else {
                    c.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .replace(".Map.Gbx", "")
                };
                match read_map(&c, &id) {
                    Ok(r) => {
                        // Keyed by (id, file), not by uid: the surgically
                        // modified variants share their parent's uid and are
                        // genuinely different bytes.
                        if !rows.iter().any(|x| x.id == r.id && x.file == r.file) {
                            rows.push(r);
                        }
                    }
                    // A map that will not parse is reported, never dropped: a
                    // registry that is quietly short is worse than one that
                    // says which row is missing and why.
                    Err(e) => problems.push(e),
                }
            }
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.file.cmp(&b.file)));
    for p in &problems {
        eprintln!("tmresim maps: {p}");
    }
    Ok(rows)
}

pub fn write(rows: &[MapRow], path: &Path) -> Result<(), String> {
    let mut s = String::from(
        "# The map registry. Nadeo's map files are NOT in this repo — they are not\n\
         # ours to redistribute. This is everything needed to fetch one again and\n\
         # prove it came back the same. Regenerate with `tmresim maps scan`;\n\
         # check a corpus against it with `tmresim maps verify`.\n\
         #\n\
         # Times are milliseconds here because this is a machine file; every\n\
         # printed line renders them as seconds with a decimal.\n\
         # spawn_x/spawn_z are world metres. There is deliberately no spawn_y:\n\
         # a block's world y needs a per-map decoration offset that is fitted,\n\
         # not read, so the start-position check is horizontal.\n",
    );
    for r in rows {
        s.push_str(&r.to_rec().render());
        s.push('\n');
    }
    std::fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read_registry(path: &Path) -> Result<Vec<MapRow>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Rec::parse_all(&text)?
        .iter()
        .filter(|r| r.kind == "map")
        .filter_map(MapRow::from_rec)
        .collect())
}

#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    Ok,
    Missing,
    Changed { got: String },
}

/// Check a corpus against the registry. This is the recovery path's proof:
/// after a refetch, every row must come back byte-identical.
pub fn verify(rows: &[MapRow], corpus: &[PathBuf]) -> Vec<(String, Check)> {
    rows.iter()
        .map(|r| {
            let found = find_pinned(corpus, r);
            let check = match found {
                None => Check::Missing,
                Some(p) => match std::fs::read(&p) {
                    Err(_) => Check::Missing,
                    Ok(b) => {
                        let got = md5_hex(&b);
                        if got == r.md5 {
                            Check::Ok
                        } else {
                            Check::Changed { got }
                        }
                    }
                },
            };
            (r.id.clone(), check)
        })
        .collect()
}

/// Horizontal distance, in metres, between where a run started and where the
/// map says the start line is.
pub fn start_deviation_m(spawn: (f64, f64), start: (f64, f64)) -> f64 {
    let dx = spawn.0 - start.0;
    let dz = spawn.1 - start.1;
    (dx * dx + dz * dz).sqrt()
}

/// Where a container's **telemetry** says the car was at the first sample.
///
/// **Read the provenance caveat before believing this.** The position comes
/// out of the container's `CPlugEntRecordData`, and this project's memory
/// records that *a synthesised tape carries its TEMPLATE's telemetry*. So for
/// a recording the engine made, this is the run's own start; for a tape
/// written into a container by our tooling, it may be the template's. The
/// distinction is not decidable from this function, which is why what it
/// returns is labelled `telemetry` everywhere it is reported, and why the
/// authoritative answer is a live re-simulation reading tick 0.
///
/// It is still worth having: it is nearly free, it runs over the whole corpus
/// standing, and it would have caught a run that began 390 m away at
/// checkpoint 3 in the first minute rather than the fourth hour.
pub fn telemetry_start_xz(path: &Path) -> Result<(f64, f64), String> {
    let body = gbx::record::load_body(&path.to_string_lossy())?;
    let (ver, blob) = gbx::record::find_entrecord_blob(&body)?;
    let rd = gbx::record::parse_record_data(&blob, ver)?;
    let ent = rd
        .ents
        .iter()
        .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
        .max_by_key(|e| e.times.len())
        .ok_or("this container holds no vehicle telemetry to read a start position from")?;
    if ent.raw.len() < ent.sample_size {
        return Err("the vehicle entity is shorter than one sample".into());
    }
    let s = gbx::record::decode_vehicle_sample(&ent.raw[..ent.sample_size]);
    Ok((s.x, s.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_becomes_the_centre_of_that_cell() {
        // Control from two independent sources, both about *Summer 2026 - 01*:
        // the map's Spawn block sits at cell (49, 7, 24), and the lead's own
        // 3D render of the map put the start line at x 1584, z 784. They must
        // agree, and they do.
        assert_eq!(cell_centre_xz(49, 24), (1584.0, 784.0));
    }

    #[test]
    fn the_start_deviation_catches_a_car_that_began_at_a_checkpoint() {
        // The real case: a run whose trajectory began at (1359.5, 1103) on a
        // map whose start line is (1584, 784). That is not a rounding error.
        let d = start_deviation_m((1584.0, 784.0), (1359.5, 1103.0));
        assert!(d > 300.0, "{d}");
    }

    #[test]
    fn a_car_on_the_start_line_deviates_by_about_nothing() {
        // The control. Without it, a deviation function that returned a large
        // number for everything would pass the test above.
        let d = start_deviation_m((1584.0, 784.0), (1585.2, 783.1));
        assert!(d < 2.0, "{d}");
    }

    #[test]
    fn a_registry_row_survives_a_round_trip() {
        let r = MapRow {
            id: "276874".into(),
            file: "276874.map.Map.Gbx".into(),
            uid: "abc".into(),
            name: "untitled 01".into(),
            author_ms: Some(23_839),
            md5: "0".repeat(32),
            bytes: 1234,
            spawn: Some((1584.0, 784.0)),
            cps: 3,
            url: resolver_url("abc"),
        };
        let back = MapRow::from_rec(&r.to_rec()).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn an_unknown_author_time_does_not_become_zero() {
        // A map declaring no author time must not read as one every run beats.
        let r = MapRow {
            id: "x".into(),
            file: "x.Map.Gbx".into(),
            uid: "u".into(),
            name: "x".into(),
            author_ms: None,
            md5: "0".repeat(32),
            bytes: 1,
            spawn: None,
            cps: 0,
            url: resolver_url("u"),
        };
        let back = MapRow::from_rec(&r.to_rec()).unwrap();
        assert_eq!(back.author_ms, None);
        assert_eq!(back.spawn, None);
        assert!(r.to_rec().render().contains("author_ms=unknown"));
    }

    #[test]
    fn a_map_with_no_uid_gets_no_invented_url() {
        assert!(resolver_url("").starts_with("unknown"));
        assert!(resolver_url("-").starts_with("unknown"));
        assert_eq!(resolver_url("buNz"), "https://trackmania.io/api/map/buNz");
    }

    #[test]
    fn verification_distinguishes_missing_from_changed() {
        let dir = std::env::temp_dir().join(format!("resim-maps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("111")).unwrap();
        std::fs::write(dir.join("111/111.Map.Gbx"), b"the bytes").unwrap();
        let rows = vec![
            MapRow {
                id: "111".into(),
                file: "111.Map.Gbx".into(),
                uid: "u1".into(),
                name: "one".into(),
                author_ms: Some(1),
                md5: md5_hex(b"the bytes"),
                bytes: 9,
                spawn: None,
                cps: 0,
                url: String::new(),
            },
            MapRow {
                id: "222".into(),
                file: "222.Map.Gbx".into(),
                uid: "u2".into(),
                name: "two".into(),
                author_ms: Some(2),
                md5: md5_hex(b"other"),
                bytes: 5,
                spawn: None,
                cps: 0,
                url: String::new(),
            },
        ];
        let got = verify(&rows, &[dir.clone()]);
        assert_eq!(got[0].1, Check::Ok);
        assert_eq!(got[1].1, Check::Missing);

        std::fs::write(dir.join("111/111.Map.Gbx"), b"tampered").unwrap();
        let got = verify(&rows, &[dir]);
        assert!(matches!(got[0].1, Check::Changed { .. }), "{:?}", got[0].1);
    }
}

/// Resolve the exact file a registry row names, never merely "a map in that
/// directory".
pub fn find_pinned(corpus: &[PathBuf], r: &MapRow) -> Option<PathBuf> {
    if r.file.is_empty() {
        return crate::find_map(corpus, &r.id);
    }
    for root in corpus {
        for cand in [root.join(&r.id).join(&r.file), root.join(&r.file)] {
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    None
}

#[cfg(test)]
mod pin_tests {
    use super::*;

    #[test]
    fn a_directory_of_variants_verifies_each_file_against_its_own_row() {
        // The real shape: 227654 holds `map.Map.Gbx` and `map_seg2.Map.Gbx`,
        // 267460 holds four. Before the file was pinned, the scanner recorded
        // one and the verifier hashed another, and both maps read CHANGED —
        // a false alarm that would have sent somebody refetching a map that
        // was already correct on the box.
        let root = std::env::temp_dir().join(format!("resim-pin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("227654")).unwrap();
        std::fs::write(root.join("227654/map.Map.Gbx"), b"pristine").unwrap();
        std::fs::write(root.join("227654/map_seg2.Map.Gbx"), b"a segment cut from it").unwrap();

        let mk = |file: &str, body: &[u8]| MapRow {
            id: "227654".into(),
            file: file.into(),
            uid: "shared-uid".into(),
            name: "The Blev Special".into(),
            author_ms: Some(57_853),
            md5: md5_hex(body),
            bytes: body.len() as u64,
            spawn: None,
            cps: 0,
            url: String::new(),
        };
        let rows = vec![
            mk("map.Map.Gbx", b"pristine"),
            mk("map_seg2.Map.Gbx", b"a segment cut from it"),
        ];
        let got = verify(&rows, &[root.clone()]);
        assert!(got.iter().all(|(_, c)| *c == Check::Ok), "{got:?}");

        // And the control: change one variant and only that row moves.
        std::fs::write(root.join("227654/map_seg2.Map.Gbx"), b"different now").unwrap();
        let got = verify(&rows, &[root]);
        assert_eq!(got[0].1, Check::Ok);
        assert!(matches!(got[1].1, Check::Changed { .. }));
    }
}

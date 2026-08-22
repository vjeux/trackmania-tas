//! Golden-data verification of the decoder against the 51 trajectories the
//! Python produced in `/tmp/entrec` (`paths/*.json`, `csv/*.csv`).
//!
//! Run with output:  `cargo test --release --test golden_decode -- --nocapture`
//!
//! Two levels of comparison per run:
//!   1. the rendered artefact, byte for byte (this is the strict test: it
//!      covers the value AND the `%.6g` / `repr()` formatting);
//!   2. numerically, field by field: max |rust_full_precision - python_file|
//!      over every sample of every run. This can only be as small as the
//!      file's own quantisation (`%.6g`, or `round(x, 4)` / `round(x, 6)` in
//!      the JSON), so it is reported per field as evidence of the floor, not
//!      asserted to zero.

use std::collections::BTreeMap;
use gbx::record::{self, Decoded};
use tmtraj::json;
use tmtraj::testonly::SampleFields;

const GOLD_JSON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/decoder-goldens/paths");
const GOLD_CSV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/decoder-goldens/csv");
const GHOST_DIRS: &[&str] = &[concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/decoder-goldens/ghosts")];

fn golden_names() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(GOLD_JSON)
        .expect("golden paths dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".json"))
        .map(|n| n.trim_end_matches(".json").to_string())
        .collect();
    v.sort();
    v
}

fn find_ghost(name: &str) -> Option<String> {
    for d in GHOST_DIRS {
        let p = format!("{}/{}.Ghost.Gbx", d, name);
        if std::path::Path::new(&p).is_file() {
            return Some(p);
        }
    }
    None
}

#[derive(Default, Clone, Copy)]
struct Dev {
    rendered: f64,
    full: f64,
    n: usize,
}

impl Dev {
    fn add(&mut self, rendered_delta: f64, full_delta: f64) {
        self.rendered = self.rendered.max(rendered_delta);
        self.full = self.full.max(full_delta);
        self.n += 1;
    }
}

#[test]
fn reproduces_the_51_python_trajectories() {
    let names = golden_names();
    assert_eq!(names.len(), 51, "expected the 51 reference trajectories");

    let mut csv_dev: BTreeMap<&str, Dev> = BTreeMap::new();
    let mut json_dev: BTreeMap<&str, Dev> = BTreeMap::new();
    let mut identical_csv = 0usize;
    let mut identical_json = 0usize;
    let mut compared = 0usize;
    let mut samples_compared = 0usize;
    let mut missing_source: Vec<String> = Vec::new();
    let mut mismatches: Vec<String> = Vec::new();

    for name in &names {
        let Some(path) = find_ghost(name) else {
            missing_source.push(name.clone());
            continue;
        };
        let dec: Decoded = record::decode_ghost(&path).unwrap_or_else(|e| panic!("{}: {}", name, e));
        compared += 1;
        samples_compared += dec.samples.len();

        // ---- 1. the rendered artefacts, byte for byte -------------------
        let mine_csv = tmtraj::testonly::csv_string(&dec);
        let gold_csv = std::fs::read_to_string(format!("{}/{}.csv", GOLD_CSV, name)).unwrap();
        if mine_csv == gold_csv {
            identical_csv += 1;
        } else {
            mismatches.push(format!("{}: CSV differs", name));
        }
        let mine_json = tmtraj::testonly::path_json_string(&dec);
        let gold_json_txt = std::fs::read_to_string(format!("{}/{}.json", GOLD_JSON, name)).unwrap();
        if mine_json == gold_json_txt {
            identical_json += 1;
        } else {
            mismatches.push(format!("{}: JSON differs", name));
        }

        // ---- 2. numeric, field by field ---------------------------------
        // CSV: golden text vs my value, and golden text vs my rendered text.
        let mut lines = gold_csv.split("\r\n");
        let header: Vec<&str> = lines.next().unwrap().split(',').collect();
        assert_eq!(header, tmtraj::testonly::CSV_COLUMNS);
        for (row, s) in lines.zip(dec.samples.iter()) {
            if row.is_empty() {
                continue;
            }
            for (col, cell) in header.iter().zip(row.split(',')) {
                let mine = s.field(col);
                let mine_txt = mine.csv();
                let (gv, mv) = match cell {
                    "True" => (1.0, mine.as_f64()),
                    "False" => (0.0, mine.as_f64()),
                    other => (other.parse::<f64>().unwrap(), mine.as_f64()),
                };
                let rendered_delta = if mine_txt == cell {
                    0.0
                } else {
                    (mine_txt.parse::<f64>().unwrap_or(f64::NAN) - gv).abs()
                };
                csv_dev
                    .entry(tmtraj::testonly::CSV_COLUMNS.iter().find(|c| *c == col).unwrap())
                    .or_default()
                    .add(rendered_delta, (mv - gv).abs());
            }
        }

        // JSON: t, x, y, z, speed, gear, yaw
        let gold = json::parse(&gold_json_txt).unwrap();
        assert_eq!(gold.get("name").unwrap().str(), name);
        assert_eq!(
            gold.get("time_ms").unwrap().int() as i32,
            dec.race_time_ms.unwrap(),
            "{}: race time", name
        );
        let gcps: Vec<i32> = gold.get("checkpoints_ms").unwrap().arr().iter().map(|v| v.int() as i32).collect();
        assert_eq!(gcps, dec.checkpoints_ms, "{}: checkpoints", name);
        assert_eq!(
            gold.get("sample_period_ms").unwrap().int() as i32,
            dec.sample_period_ms.unwrap(),
            "{}: sample period", name
        );
        let gs = gold.get("samples").unwrap().arr();
        assert_eq!(gs.len(), dec.samples.len(), "{}: sample count", name);
        for (g, s) in gs.iter().zip(dec.samples.iter()) {
            for (key, full, rounded) in [
                ("t", s.time_ms as f64, s.time_ms as f64),
                ("x", s.x, json::py_round(s.x, 4)),
                ("y", s.y, json::py_round(s.y, 4)),
                ("z", s.z, json::py_round(s.z, 4)),
                ("speed", s.speed_kmh, json::py_round(s.speed_kmh, 4)),
                ("gear", s.gear, s.gear),
                ("yaw", s.yaw, json::py_round(s.yaw, 6)),
            ] {
                let gv = g.get(key).unwrap().num();
                json_dev
                    .entry(match key {
                        "t" => "t",
                        "x" => "x",
                        "y" => "y",
                        "z" => "z",
                        "speed" => "speed",
                        "gear" => "gear",
                        _ => "yaw",
                    })
                    .or_default()
                    .add((rounded - gv).abs(), (full - gv).abs());
            }
        }
    }

    // ------------------------------------------------------------------
    println!("\n=== DECODER vs the Python's 51 reference trajectories ===");
    println!(
        "runs re-decoded from source ghosts: {} / {}   ({} samples, {} fields per sample)",
        compared,
        names.len(),
        samples_compared,
        record::FIELD_CONFIDENCE.len()
    );
    if !missing_source.is_empty() {
        println!(
            "source .Ghost.Gbx not present on this box for {} runs (their JSON/CSV survive but the\n\
             ghosts do not, so they cannot be re-decoded by anyone): {}",
            missing_source.len(),
            missing_source.join(", ")
        );
    }
    println!(
        "byte-identical CSV: {} / {}      byte-identical JSON: {} / {}",
        identical_csv, compared, identical_json, compared
    );

    println!("\nCSV columns -- max |deviation| over all runs and samples:");
    println!(
        "  {:<22} {:>14} {:>16} {:>10}",
        "column", "rendered", "vs full precision", "values"
    );
    for (k, v) in &csv_dev {
        println!(
            "  {:<22} {:>14.1e} {:>16.3e} {:>10}",
            k, v.rendered, v.full, v.n
        );
    }
    println!("\npath JSON keys -- max |deviation| over all runs and samples:");
    println!(
        "  {:<22} {:>14} {:>16} {:>10}",
        "key", "rounded", "vs full precision", "values"
    );
    for (k, v) in &json_dev {
        println!(
            "  {:<22} {:>14.1e} {:>16.3e} {:>10}",
            k, v.rendered, v.full, v.n
        );
    }
    println!(
        "\n(the 'vs full precision' column is the file's own quantisation: %.6g in the CSV,\n\
         round(x,4) / round(x,6) in the JSON. The 'rendered' column is what the port must\n\
         reproduce, and it is exactly zero.)"
    );

    assert!(mismatches.is_empty(), "mismatches: {:?}", mismatches);
    assert_eq!(identical_csv, compared);
    assert_eq!(identical_json, compared);
    for (k, v) in &csv_dev {
        assert_eq!(v.rendered, 0.0, "CSV column {} deviates", k);
    }
    for (k, v) in &json_dev {
        assert_eq!(v.rendered, 0.0, "JSON key {} deviates", k);
    }
    assert!(compared >= 45, "only {} source ghosts found", compared);
}

/// The record blob must be consumed to the exact last byte on every ghost we
/// can reach -- the structural check that validates every field width in the
/// grammar.
#[test]
fn every_available_ghost_parses_exactly() {
    let mut n = 0;
    let mut ents_with_two_vehicles = Vec::new();
    for name in golden_names() {
        let Some(path) = find_ghost(&name) else { continue };
        let dec = record::decode_ghost(&path).unwrap();
        assert_eq!(
            dec.bytes_consumed, dec.bytes_total,
            "{}: blob not consumed exactly", name
        );
        assert_eq!(dec.sample_size, 116, "{}: unexpected sample size", name);
        assert_eq!(dec.sample_period_ms, Some(50), "{}: unexpected period", name);
        let veh = dec
            .ents
            .iter()
            .filter(|e| e.class_id == Some(record::CLASS_CSCENEVEHICLEVIS))
            .count();
        if veh > 1 {
            ents_with_two_vehicles.push(name.clone());
        }
        n += 1;
    }
    println!(
        "\n{} ghosts parsed to the exact last byte; {} of them carry TWO CSceneVehicleVis \
         entities (decimated + full rate): {:?}",
        n, ents_with_two_vehicles.len(), ents_with_two_vehicles
    );
    assert!(n >= 45);
}

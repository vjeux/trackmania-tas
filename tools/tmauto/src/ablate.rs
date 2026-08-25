//! Structural ablation ladder for the from-scratch ghost writer.
//!
//! Every row adds one record feature. The command writes each GBX, its parsed
//! JSON manifest, the dedicated server's raw stdout/stderr, and a TSV verdict.
//! It never infers a start position from acceptance: until a live-state reader
//! reports tick 0, an accepted row is explicitly `start_unmeasured`.

use std::path::PathBuf;
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, RecordMode};
use tmauto::tape::Input;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let out = PathBuf::from(arg(args, "--out").ok_or("--out DIR is required")?);
    let tape = match arg(args, "--tape") {
        Some(p) => super::tape_from_tsv(std::path::Path::new(&p))?,
        None => {
            let ticks: usize = arg(args, "--ticks")
                .unwrap_or_else(|| "600".into())
                .parse()
                .map_err(|_| "--ticks")?;
            vec![Input::FULL_GAS; ticks]
        }
    };
    let declared: u32 = arg(args, "--declared")
        .unwrap_or_else(|| (tape.len() as u32 * 10).to_string())
        .parse()
        .map_err(|_| "--declared")?;
    let checkpoints: i32 = arg(args, "--checkpoints")
        .unwrap_or_else(|| "3".into())
        .parse()
        .map_err(|_| "--checkpoints")?;
    let mut meta = synth::complete_meta_for_map(&map)?;
    let declared_cps: Vec<i32> = (1..=checkpoints)
        .map(|i| (declared as i32 / (checkpoints + 1)) * i)
        .chain(std::iter::once(declared as i32))
        .collect();
    meta.set_declared(declared, declared_cps);
    let initial = synth::initial_state_for_map(&map)?;

    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let variants = [
        ("00-no-record", RecordMode::None, 0.0f32),
        ("01-parent", RecordMode::Parent, 0.0),
        ("02-descriptor", RecordMode::Descriptor, 0.0),
        ("03-entity", RecordMode::Entity, 0.0),
        ("04-first-sample", RecordMode::Sample, 0.0),
        ("05-full-grid", RecordMode::Grid, 0.0),
        ("06-corrupt-start-x-plus-64m", RecordMode::Sample, 64.0),
    ];
    let mut files = Vec::new();
    for (name, mode, dx) in variants {
        let bytes = synth::synthesize_complete(&tape, &meta, &ChunkSet::ALL, initial, mode, dx);
        let p = out.join(format!("{name}.Ghost.Gbx"));
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        let manifest = gbx::manifest::manifest_bytes(&bytes)?;
        std::fs::write(out.join(format!("{name}.manifest.json")), manifest)
            .map_err(|e| e.to_string())?;
        files.push((name, mode, dx, p));
    }

    let paths: Vec<PathBuf> = files.iter().map(|x| x.3.clone()).collect();
    let batch = oracle::validate_raw(
        &oracle::server_dir(),
        &paths,
        Maps::One(&map),
        "record-ablation",
    )?;
    std::fs::write(out.join("server.stdout.txt"), &batch.raw).map_err(|e| e.to_string())?;
    std::fs::write(out.join("server.stderr.txt"), &batch.err).map_err(|e| e.to_string())?;

    let mut report = String::from(
        "variant\trecord_mode\tcorrupt_x_m\tclassification\tvalidated_ms\tvalidated_cps\tdeclared_ms\tdeclared_cps\tdescription\n",
    );
    for (name, mode, dx, p) in &files {
        let base = p.file_name().unwrap().to_string_lossy();
        let a = batch.by_name(&base);
        let class = match a {
            None => "parser_refusal_or_not_enumerated",
            Some(a) if !a.simulated() => "parser_or_validator_refusal",
            Some(a) if a.time_ms.is_some() => "accepted_finish_start_unmeasured",
            Some(_) => "accepted_dnf_start_unmeasured",
        };
        let field = |v: Option<i64>| v.map(|x| x.to_string()).unwrap_or_else(|| "-".into());
        let fieldu = |v: Option<u32>| v.map(|x| x.to_string()).unwrap_or_else(|| "-".into());
        let (tm, cp, dm, dc, desc) = match a {
            Some(a) => (
                field(a.time_ms),
                fieldu(a.cps),
                field(a.declared_ms),
                fieldu(a.declared_cps),
                a.desc.trim().replace(['\t', '\n', '\r'], " "),
            ),
            None => (
                "-".into(),
                "-".into(),
                "-".into(),
                "-".into(),
                String::new(),
            ),
        };
        report.push_str(&format!(
            "{name}\t{}\t{dx:.3}\t{class}\t{tm}\t{cp}\t{dm}\t{dc}\t{desc}\n",
            mode.name()
        ));
    }
    std::fs::write(out.join("report.tsv"), &report).map_err(|e| e.to_string())?;
    print!("{report}");
    println!(
        "raw server transcript: {} and {}",
        out.join("server.stdout.txt").display(),
        out.join("server.stderr.txt").display()
    );
    println!(
        "map-derived initial: pos=({:.3},{:.3},{:.3}) quat=({:.6},{:.6},{:.6},{:.6}) dir={:?} validation_start_index={}",
        initial.pos[0],
        initial.pos[1],
        initial.pos[2],
        initial.quat[0],
        initial.quat[1],
        initial.quat[2],
        initial.quat[3],
        initial.roadtech_dir,
        meta.validation_start_index
    );
    Ok(())
}

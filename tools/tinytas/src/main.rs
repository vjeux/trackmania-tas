//! `tinytas` CLI — one subcommand per pipeline stage.

use std::path::PathBuf;
use tinytas::scale::{self, Xform};

fn usage() -> ! {
    eprintln!(
        "tinytas -- the tiny-campaign TAS pipeline, one subcommand per stage

STAGE C  scaled reference set
  tinytas scale-ref --pack ORIG.pack.json --route ORIG.route.json --map TINY.Map.Gbx --out DIR
                    [--anchor x,y,z] [--anchor-to x,y,z] [--scale K] [--half M] [--name NAME]
        Map the cartographer's pack + route of the ORIGINAL map through
        p' = anchor_to + K*(p - anchor) and write <tinyuid>.pack.json /
        <tinyuid>.route.json. Then the CONTROL: every scaled gate centre against
        the tiny map's own waypoint items (both yaw conventions tried; the one
        that lands is reported). Defaults: anchor 1584,16,784 -> 1584,11.5,784,
        K 0.5, half footprint 8 m. Exit 1 if any gate residual exceeds 0.5 m.

STAGE G  the validated map (author ghost embedded)
  tinytas authorghost probe --map M.Map.Gbx
        What the map embeds in 0x0305B00F: the ghost's chunks, Id literals,
        record node, declared time.
  tinytas authorghost extract --map M.Map.Gbx --out G.Ghost.Gbx [--keep-uid]
        The embedded author ghost as a standalone .Ghost.Gbx (the two four-byte
        defects reversed). Re-simulate it with `tmauto verdict` -- the proof.
  tinytas authorghost embed --map M.Map.Gbx --ghost OURS.Ghost.Gbx --out V.Map.Gbx [--login L]
        THE VALIDATED MAP: our ghost (declared from the oracle for THIS map) as
        the author ghost, AT = its millisecond, medals 1.08/1.20/1.50 rounded
        up to the second, validated=\"1\", every copy of the times rewritten.
        Prove it: extract from the output and re-simulate to the same ms."
    );
    std::process::exit(2)
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(|s| s.as_str()) {
        Some("scale-ref") => cmd_scale_ref(&args[1..]),
        Some("authorghost") if args.get(1).map(|s| s.as_str()) == Some("dumphead") => cmd_dumphead(&args[2..]),
        Some("authorghost") => cmd_authorghost(&args[1..]),
        Some("tape") => cmd_tape(&args[1..]),
        _ => usage(),
    };
    if let Err(e) = r {
        eprintln!("tinytas: {e}");
        std::process::exit(1);
    }
}

fn cmd_scale_ref(args: &[String]) -> Result<(), String> {
    let pack_p = PathBuf::from(arg(args, "--pack").ok_or("--pack is required")?);
    let route_p = PathBuf::from(arg(args, "--route").ok_or("--route is required")?);
    let map_p = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let out = PathBuf::from(arg(args, "--out").ok_or("--out is required")?);
    let x = Xform {
        anchor: scale::parse_xyz(&arg(args, "--anchor").unwrap_or_else(|| "1584,16,784".into()))?,
        anchor_to: scale::parse_xyz(&arg(args, "--anchor-to").unwrap_or_else(|| "1584,11.5,784".into()))?,
        k: arg(args, "--scale").unwrap_or_else(|| "0.5".into()).parse().map_err(|_| "--scale")?,
    };
    let half: f64 = arg(args, "--half").unwrap_or_else(|| "8".into()).parse().map_err(|_| "--half")?;

    let bytes = std::fs::read(&map_p).map_err(|e| format!("{}: {}", map_p.display(), e))?;
    let uid = gbx::map_uid_of(&bytes).ok_or("tiny map: no uid found")?;
    let name = arg(args, "--name").unwrap_or_else(|| {
        tmmaps::header::read(&map_p.to_string_lossy())
            .map(|h| h.name)
            .unwrap_or_else(|_| "tiny".into())
    });
    let tiny = tmmaps::map::MapFile::load(&map_p);

    let pack = scale::load_json(&pack_p)?;
    let route = scale::load_json(&route_p)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {}", out.display(), e))?;
    for (j, suffix) in [(&pack, "pack"), (&route, "route")] {
        let mut s = String::new();
        scale::scale_json(j, &x, None, true, &uid, &name, &mut s);
        s.push('\n');
        let p = out.join(format!("{uid}.{suffix}.json"));
        std::fs::write(&p, s).map_err(|e| format!("{}: {}", p.display(), e))?;
        println!("wrote {}", p.display());
    }
    println!(
        "transform  p' = {:?} + {} * (p - {:?})   lengths x{}",
        x.anchor_to, x.k, x.anchor, x.k
    );

    // THE CONTROL. Try both yaw conventions; report the better one and the
    // worse one beside it, so the convention is a measurement.
    let mut best: Option<(f64, f64, Vec<scale::GateControlRow>)> = None;
    for sign in [1.0, -1.0] {
        let rows = scale::gate_control(&pack, &x, &tiny, half, sign);
        let worst = rows.iter().filter_map(|r| r.residual_m).fold(0.0, f64::max);
        if best.as_ref().map(|b| worst < b.0).unwrap_or(true) {
            best = Some((worst, sign, rows));
        }
    }
    let (worst, sign, rows) = best.ok_or("no gates in the pack")?;
    println!("\ngate control (yaw convention sign {sign:+}): scaled ORIGINAL gate centre vs tiny waypoint ITEM centre");
    println!("{:<22} {:<28} {:<10} {:<20} {:<28} {}", "tag", "scaled centre", "item#", "model", "item centre", "residual m (xz)");
    let mut missing = 0;
    for r in &rows {
        match &r.item {
            Some((idx, model, c, d)) => println!(
                "{:<22} {:<28} {:<10} {:<20} {:<28} {:.3}",
                r.tag,
                format!("({:.1}, {:.1}, {:.1})", r.scaled_centre[0], r.scaled_centre[1], r.scaled_centre[2]),
                idx,
                model,
                format!("({:.1}, {:.1}, {:.1})", c[0], c[1], c[2]),
                d
            ),
            None => {
                missing += 1;
                println!("{:<22} {:<28} NO ITEM WITH THIS TAG", r.tag, format!("{:?}", r.scaled_centre));
            }
        }
    }
    println!("worst residual {worst:.3} m over {} gates; {} unmatched", rows.len(), missing);
    if missing > 0 || worst > 0.5 {
        return Err(format!(
            "TRANSFORM CONTROL FAILED: worst residual {worst:.3} m (bar 0.5), {missing} unmatched. \
             The scaled reference set was written but must not be used until this is understood."
        ));
    }
    println!("TRANSFORM CONTROL PASS");
    Ok(())
}

fn cmd_authorghost(args: &[String]) -> Result<(), String> {
    use tinytas::authorghost as ag;
    if let Some(gp) = arg(args, "--ghost") {
        // A standalone ghost: list its chunks the same way, for the differential.
        let g = tmmaps::gbx::Gbx::load(std::path::Path::new(&gp)).map_err(|e| format!("{gp}: {e}"))?;
        println!("ghost {} body {} bytes num_nodes {} class 0x{:08X}", gp, g.body.len(), g.num_nodes, g.class_id);
        let bd = ag::split_body(&g.body)?;
        for c in &bd.chunks {
            let ids = ag::id_literals(ag::payload(c));
            println!("  0x{:08X} {:>5} bytes {}  ids {:?}", c.id, ag::payload(c).len(), if ag::is_skip(c) { "skip" } else { "    " }, ids.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>());
            if c.id == ag::CLASS_GHOST {
                if let Some((a, b)) = ag::find_record_node(ag::payload(c)) {
                    let p = ag::payload(c);
                    println!("      record node at payload [{a}, {b}); word before class id = 0x{:08X}", if a >= 4 { u32::from_le_bytes(p[a - 4..a].try_into().unwrap()) } else { 0 });
                }
            }
        }
        println!("  suffix {:02X?}", &bd.suffix[..bd.suffix.len().min(8)]);
        return Ok(());
    }
    let map_p = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let map = tmmaps::gbx::Gbx::load(&map_p).map_err(|e| format!("{}: {}", map_p.display(), e))?;
    match args.first().map(|s| s.as_str()) {
        Some("probe") => {
            let p = ag::probe(&map);
            print!("{}", p.report);
            if p.ghosts.is_empty() {
                return Err("NO EMBEDDED GHOST (no CGameCtnGhost node blob in the body)".into());
            }
            Ok(())
        }
        Some("extract") => {
            let out = PathBuf::from(arg(args, "--out").ok_or("--out is required")?);
            let p = ag::probe(&map);
            let g = p.ghosts.first().ok_or("NO EMBEDDED GHOST")?;
            let keep = args.iter().any(|a| a == "--keep-uid");
            let uid = if keep { None } else { p.uid.as_deref() };
            let (bytes, log) = ag::extract(&map, g, uid)?;
            print!("{log}");
            std::fs::write(&out, &bytes).map_err(|e| format!("{}: {}", out.display(), e))?;
            println!("wrote {} ({} bytes)", out.display(), bytes.len());
            Ok(())
        }
        Some("embed") => {
            let out = PathBuf::from(arg(args, "--out").ok_or("--out is required")?);
            let gp = PathBuf::from(arg(args, "--ghost").ok_or("--ghost is required")?);
            let gbytes = std::fs::read(&gp).map_err(|e| format!("{}: {}", gp.display(), e))?;
            let login = arg(args, "--login");
            let e = ag::embed(&map, &gbytes, login.as_deref())?;
            print!("{}", e.log);
            std::fs::write(&out, &e.bytes).map_err(|e| format!("{}: {}", out.display(), e))?;
            println!(
                "wrote {} ({} bytes): author {} gold {} silver {} bronze {}",
                out.display(), e.bytes.len(), secs(e.author_ms), secs(e.medals.0), secs(e.medals.1), secs(e.medals.2)
            );
            Ok(())
        }
        _ => Err("authorghost wants probe|extract|embed".into()),
    }
}

/// `tinytas authorghost dumphead --map M [--n N] [--find MS]`: the first N body
/// bytes as hex, and every u32 equal to --find in header and body.
fn cmd_dumphead(args: &[String]) -> Result<(), String> {
    let map_p = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let map = tmmaps::gbx::Gbx::load(&map_p).map_err(|e| format!("{}: {}", map_p.display(), e))?;
    let n: usize = arg(args, "--n").unwrap_or_else(|| "220".into()).parse().map_err(|_| "--n")?;
    for (i, row) in map.body[..n.min(map.body.len())].chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{b:02X}")).collect();
        let asc: String = row.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect();
        println!("{:6}  {:<48}  {}", i * 16, hex.join(" "), asc);
    }
    if let Some(f) = arg(args, "--find") {
        let v: u32 = f.parse().map_err(|_| "--find wants a u32")?;
        for (name, bytes) in [("header", &map.user_data), ("body", &map.body)] {
            for i in 0..bytes.len().saturating_sub(4) {
                if u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) == v {
                    println!("{name} @{i}: {v}  context {:02X?}", &bytes[i.saturating_sub(16)..(i + 20).min(bytes.len())]);
                }
            }
        }
    }
    Ok(())
}

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// `tinytas tape assemble --template T.Ghost.Gbx --tape best.tape.tsv --out full.tsv [--frame N]`
///
/// A search tape's tick 0 is the fork's RESUME BOUNDARY, not the file's: the
/// container's own inputs fill the file below the frame. This writes the
/// whole-file tape (template[0..frame] ++ search tape) as the TSV `tmauto synth
/// write --tape` takes, so the certified file is built from tick 0 by the
/// container writer and nothing else. The frame comes from the tape file's
/// `# frame N` line unless --frame overrides it; a tape with neither is refused.
fn cmd_tape(args: &[String]) -> Result<(), String> {
    if args.first().map(|s| s.as_str()) != Some("assemble") {
        return Err("tape wants assemble".into());
    }
    let template = arg(args, "--template").ok_or("--template is required")?;
    let tape_p = arg(args, "--tape").ok_or("--tape is required")?;
    let out = PathBuf::from(arg(args, "--out").ok_or("--out is required")?);
    let txt = std::fs::read_to_string(&tape_p).map_err(|e| format!("{tape_p}: {e}"))?;
    let mut file_frame: Option<usize> = None;
    let mut rows: Vec<(i8, bool, bool)> = Vec::new();
    for line in txt.lines() {
        if let Some(r) = line.strip_prefix("# frame") {
            file_frame = r.trim().parse().ok();
        }
        if line.starts_with('#') || line.starts_with("tick") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        rows.push((f[1].parse().map_err(|_| format!("bad steer {:?}", f[1]))?, f[2] != "0", f[3] != "0"));
    }
    let frame = match (arg(args, "--frame"), file_frame) {
        (Some(f), _) => f.parse().map_err(|_| "--frame")?,
        (None, Some(f)) => f,
        (None, None) => return Err("the tape carries no `# frame N` line and no --frame was given; a tape replayed at the wrong boundary is a different run".into()),
    };
    let t = gbx::tape::Tape::from_file(&template)?;
    let steer = t.steer_i8s();
    let gas = t.accels();
    let brake = t.brakes();
    if steer.len() < frame {
        return Err(format!("template has {} ticks, frame is {}", steer.len(), frame));
    }
    let mut s = String::from(format!("# assembled: template {} ticks 0..{} + search tape {} ticks\ntick\tsteer\tgas\tbrake\n", template, frame, rows.len()));
    for i in 0..frame {
        s.push_str(&format!("{}\t{}\t{}\t{}\n", i, steer[i], gas[i], brake[i]));
    }
    for (k, (st, g, b)) in rows.iter().enumerate() {
        s.push_str(&format!("{}\t{}\t{}\t{}\n", frame + k, st, *g as u8, *b as u8));
    }
    std::fs::write(&out, s).map_err(|e| format!("{}: {}", out.display(), e))?;
    println!("wrote {} ({} ticks = {} prefix + {} search; frame {})", out.display(), frame + rows.len(), frame, rows.len(), frame);
    Ok(())
}

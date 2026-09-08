//! `tinyctl ship` — a certified set out of the per-map builds: the 25 maps
//! copied under their published names, MANIFEST.txt (map, MB, items, md5,
//! collhash, fillers_left_out), ANCHORS.tsv, and — with `--startcheck` — the
//! client start check of every map, one at a time, into STARTCHECK.tsv.
//!
//! ```text
//! tinyctl ship --set ship11-<commit> --dest DIR [--out-root /tmp] [--tag v2]
//!              [--maps 01,02,…] [--note "one line for the MANIFEST header"]
//!              [--startcheck [--startcheck-outdir D]]
//! ```
//!
//! The map NN is read from `<out-root>/tinyNN/<tag>/Summer-NN-Tiny.Map.Gbx`
//! with its `build.log` (the `fillers left out` line, the anchor line) and
//! `tiny.log` beside it. Everything is computed in-process (mapgeom's md5 and
//! collhash), no shell.

use std::path::PathBuf;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let set = f("--set").ok_or("ship needs --set NAME (ship11-<commit>)")?;
    let dest = PathBuf::from(f("--dest").ok_or("ship needs --dest DIR")?);
    let out_root = PathBuf::from(f("--out-root").unwrap_or_else(|| "/tmp".into()));
    let tag = f("--tag").unwrap_or_else(|| "auto".into());
    let note = f("--note").unwrap_or_default();
    let maps: Vec<String> = match f("--maps") {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        None => (1..=25).map(|n| format!("{n:02}")).collect(),
    };
    let startcheck = tmmaps::cli::has(args, "--startcheck");
    let sc_outdir = PathBuf::from(f("--startcheck-outdir").unwrap_or_else(|| "/tmp/tiny3".into()));
    let src_dir = PathBuf::from(f("--src-dir").unwrap_or_else(|| "/tmp/summer2026".into()));
    std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;

    let mut manifest = String::new();
    if !note.is_empty() {
        manifest.push_str(&format!("# {set} — {note}\n"));
    } else {
        manifest.push_str(&format!("# {set}\n"));
    }
    manifest.push_str("map\tMB\titems\tmd5\tcollhash\tfillers_left_out\n");
    let mut anchors = String::from("nn\tsource_map\tanchor_src_x\tanchor_src_y\tanchor_src_z\tanchor_tiny_x\tanchor_tiny_y\tanchor_tiny_z\tscale\n");
    let mut total_left_out = 0usize;
    for nn in &maps {
        let dir = out_root.join(format!("tiny{nn}")).join(&tag);
        let src = dir.join(format!("Summer-{nn}-Tiny.Map.Gbx"));
        if !src.exists() {
            return Err(format!("{}: no such build (tinyctl build {nn} --tag {tag} first)", src.display()));
        }
        let bytes = std::fs::read(&src).map_err(|e| format!("{}: {e}", src.display()))?;
        let name = format!("Tiny Summer 2026 - {nn}.Map.Gbx");
        let out = dest.join(&name);
        std::fs::write(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
        let md5 = mapgeom::md5::md5(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>();
        let m = tmmaps::map::MapFile::load(&src);
        let coll = mapgeom::collhash::summary(&m);
        let build_log = std::fs::read_to_string(dir.join("build.log")).unwrap_or_default();
        let tiny_log = std::fs::read_to_string(dir.join("tiny.log")).unwrap_or_default();
        let left_out = fillers_left_out(&build_log);
        total_left_out += left_out;
        manifest.push_str(&format!("{nn}\t{:.1}\t{}\t{}\t{}\t{}\n", bytes.len() as f64 / 1.0e6, m.items.len(), &md5[..8], coll.total, left_out));
        let source = std::fs::read_dir(&src_dir).ok().and_then(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().map(|n| n.to_string_lossy().starts_with(&format!("{nn}-"))).unwrap_or(false))).map(|p| p.display().to_string()).unwrap_or_default();
        if let Some(row) = anchor_row(nn, &source, &format!("{build_log}\n{tiny_log}")) {
            anchors.push_str(&row);
        }
        println!("{nn}\t{:.1} MB\t{} items\tmd5 {}\tcollhash {}\tleft out {left_out}\t-> {}", bytes.len() as f64 / 1.0e6, m.items.len(), &md5[..8], coll.total, out.display());
    }
    manifest.push_str(&format!("# {} maps, {} filler records left out in all\n", maps.len(), total_left_out));
    std::fs::write(dest.join("MANIFEST.txt"), &manifest).map_err(|e| e.to_string())?;
    std::fs::write(dest.join("ANCHORS.tsv"), &anchors).map_err(|e| e.to_string())?;
    println!("wrote {}/MANIFEST.txt and ANCHORS.tsv", dest.display());

    if startcheck {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut tsv = String::from("map\tresult\tdetail\n");
        for nn in &maps {
            let map = dest.join(format!("Tiny Summer 2026 - {nn}.Map.Gbx"));
            let out = std::process::Command::new(&exe)
                .args(["startcheck", "--map"])
                .arg(&map)
                .args(["--tag", &format!("sc{nn}"), "--outdir"])
                .arg(&sc_outdir)
                .output()
                .map_err(|e| format!("tinyctl startcheck: {e}"))?;
            let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
            let detail = text.lines().find(|l| l.contains("client car at")).map(|l| l.trim().to_string()).unwrap_or_else(|| text.lines().last().unwrap_or("").trim().to_string());
            let result = if out.status.success() && text.contains("PASS") { "PASS" } else { "FAIL" };
            println!("{nn}\t{result}\t{detail}");
            tsv.push_str(&format!("{nn}\t{result}\t{detail}\n"));
            std::fs::write(dest.join("STARTCHECK.tsv"), &tsv).map_err(|e| e.to_string())?;
        }
        println!("wrote {}/STARTCHECK.tsv", dest.display());
    }
    Ok(())
}

/// The sum of the `xN` counts on the build log's `fillers left out by
/// TINY_FILLER_RULE=…:` line (0 when the line is absent).
pub fn fillers_left_out(build_log: &str) -> usize {
    let Some(line) = build_log.lines().find(|l| l.contains("fillers left out by")) else { return 0 };
    line.split(',')
        .filter_map(|part| part.trim().rsplit(" x").next().and_then(|n| n.trim().parse::<usize>().ok()))
        .sum()
}

/// The ANCHORS.tsv row from the build log's
/// `anchor: source [x, y, z] -> target [x, y, z]; scale S` line and its
/// `source map` line.
fn anchor_row(nn: &str, src: &str, build_log: &str) -> Option<String> {
    let line = build_log.lines().find(|l| l.contains("anchor: source ["))?;
    let nums: Vec<f32> = line
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter(|s| !s.is_empty() && *s != "-")
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    if nums.len() < 7 {
        return None;
    }
    Some(format!("{nn}\t{src}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", nums[0], nums[1], nums[2], nums[3], nums[4], nums[5], nums[6]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn left_out_sums_the_counts() {
        let log = "  x\n  fillers left out by TINY_FILLER_RULE=face: A (free clip against B's face [c]) x4, D (free clip against E's face [f|g]) x1, H (x) x22\n";
        assert_eq!(super::fillers_left_out(log), 27);
        assert_eq!(super::fillers_left_out("nothing"), 0);
    }
}

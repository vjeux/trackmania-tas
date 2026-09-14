//! `tinyctl hunt-publish` — a whole-club surface set (the `surface-all` dist
//! zips) to GitHub AND to Everios' hunt club, box-side and unattended
//! (2026-09-13, vjeux: "convert all the 975 files and upload them both to
//! nadeo at hunt campaign and github for all the types").
//!
//! ```text
//! tinyctl hunt-publish --dist DIR --surface S --tag wood-u10s-maps --title T --notes README.md
//!                      --club 145872 --folder "Wood U10S" --room-prefix "Wood U10S - Part "
//!                      [--parts 01-39] [--lanes 3] [--box-dir /home/vjeux/shoot/hunt/<surface>]
//!                      [--repo vjeux/trackmania-tas] [--no-github] [--no-rooms] [--wsx P] [--box-tinyctl P]
//! ```
//!
//! Per part zip, in order: pushed to the box ONCE, then on the box `gh release
//! upload` (the release created with the notes when missing), unzipped into
//! `<box-dir>/pNN/`, a manifest written (`path<TAB>name`, the name = the zip
//! entry's stem without its `NN-` order prefix — what Nadeo stores as the map
//! name). The publishing runs in `--lanes` detached box jobs (lane k takes the
//! parts k, k+lanes, …; each `publish-batch` run mints one token and uploads
//! 25 maps, ~10 s a map), started as soon as the first parts are staged so the
//! bridge pushes and the uploads overlap. When every part has its results,
//! `nadeo-here club-rooms` (detached) builds Everios' layout: one FOLDER named
//! after the alteration, one Nadeo-hosted TimeAttack ROOM per part with the
//! part's 25 maps in campaign order. Results and the rooms table are pulled
//! into `<dist>/../hunt/`. Idempotent: a part whose results say 25 IDENTICAL is
//! skipped, an asset already on GitHub with the right size is skipped.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::wsx::Wsx;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let dist = PathBuf::from(f("--dist").ok_or("hunt-publish needs --dist DIR (the surface-all zips)")?);
    let surface = f("--surface").ok_or("hunt-publish needs --surface S")?;
    let word = crate::surface::surface_label(&surface)?.0;
    let no_github = tmmaps::cli::has(args, "--no-github");
    let no_rooms = tmmaps::cli::has(args, "--no-rooms");
    let tag = f("--tag").unwrap_or_else(|| format!("{}-u10s-maps", word.to_ascii_lowercase().replace(' ', "-")));
    let title = f("--title").unwrap_or_else(|| format!("{word} U10S - maps"));
    let notes = f("--notes").map(PathBuf::from);
    let repo = f("--repo").unwrap_or_else(|| "vjeux/trackmania-tas".into());
    let club = f("--club").unwrap_or_else(|| "145872".into());
    let folder = f("--folder").unwrap_or_else(|| format!("{word} U10S"));
    let room_prefix = f("--room-prefix").unwrap_or_else(|| format!("{word} U10S - Part "));
    let lanes: usize = f("--lanes").and_then(|s| s.parse().ok()).unwrap_or(3).max(1);
    let box_dir = f("--box-dir").unwrap_or_else(|| format!("/home/vjeux/shoot/hunt/{}", surface.to_ascii_lowercase()));
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/variants/tinyctl".into());
    let out_dir = f("--out").map(PathBuf::from).unwrap_or_else(|| dist.parent().map(|p| p.join("hunt")).unwrap_or_else(|| PathBuf::from("hunt")));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let parts: Vec<String> = match f("--parts") {
        Some(p) if p.contains('-') => {
            let (a, b) = p.split_once('-').unwrap();
            let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| "--parts A-B")?, b.trim().parse().map_err(|_| "--parts A-B")?);
            (a..=b).map(|n| format!("{n:02}")).collect()
        }
        Some(p) => p.split(',').map(|s| format!("{:02}", s.trim().parse::<usize>().unwrap_or(0))).collect(),
        None => (1..=39).map(|n| format!("{n:02}")).collect(),
    };
    let wsx = Wsx::new(args);
    let gh = "cd /home/vjeux && ./bin/gh";
    let t0 = Instant::now();
    let zip_of = |part: &str| dist.join(format!("{}U10S-part{part}.zip", word.replace(' ', "")));
    for p in &parts {
        let z = zip_of(p);
        if !z.exists() {
            return Err(format!("{}: no such zip", z.display()));
        }
    }
    wsx.sh(&format!("mkdir -p {box_dir}/zips {box_dir}/results {box_dir}/lanes"))?;

    // --- GitHub release (created once with the notes)
    let mut have: std::collections::BTreeMap<String, u64> = Default::default();
    if !no_github {
        let exists = wsx.sh(&format!("{gh} release view {tag} -R {repo} --json tagName 2>/dev/null | grep -c tagName; true"))?;
        if exists.trim() == "0" || exists.trim().is_empty() {
            let notes = notes.as_ref().ok_or("hunt-publish needs --notes README.md to create the release")?;
            let remote_notes = format!("{box_dir}/{}", notes.file_name().unwrap_or_default().to_string_lossy());
            wsx.push(notes, &remote_notes)?;
            let out = wsx.sh(&format!("{gh} release create {tag} -R {repo} --title '{}' --notes-file {remote_notes} 2>&1; true", title.replace('\'', "")))?;
            println!("[{:>5.0}s] release create: {}", t0.elapsed().as_secs_f64(), out.trim());
        } else if let Some(notes) = notes.as_ref() {
            let remote_notes = format!("{box_dir}/{}", notes.file_name().unwrap_or_default().to_string_lossy());
            wsx.push(notes, &remote_notes)?;
            let out = wsx.sh(&format!("{gh} release edit {tag} -R {repo} --title '{}' --notes-file {remote_notes} 2>&1; true", title.replace('\'', "")))?;
            println!("[{:>5.0}s] release exists; notes refreshed: {}", t0.elapsed().as_secs_f64(), out.trim());
        }
        let present = wsx.sh(&format!("{gh} release view {tag} -R {repo} --json assets --jq '.assets[] | \"\\(.name)\\t\\(.size)\"' 2>/dev/null; true"))?;
        have = present.lines().filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse().ok().map(|s| (n.trim().to_string(), s)))).collect();
    }

    // --- which parts are published already (results pulled earlier)
    let done_part = |part: &str| -> bool {
        std::fs::read_to_string(out_dir.join(format!("results-p{part}.tsv"))).map(|t| t.lines().filter(|l| l.ends_with("\tIDENTICAL")).count() >= 25).unwrap_or(false)
    };

    // --- the lane runner on the box: lane k publishes the parts of its queue file,
    // one publish-batch (blocking) per part, in order of appearance; a part is
    // appended to a lane's queue once its maps are staged
    let runner = format!(
        "#!/bin/sh\n# lane runner: $1 = lane index; reads {box_dir}/lanes/lane-$1.queue (one part per line, appended as parts are staged; STOP ends it)\nlane=$1\nseen=0\nwhile :; do\n  n=$(wc -l < {box_dir}/lanes/lane-$lane.queue 2>/dev/null || echo 0)\n  if [ \"$n\" -gt \"$seen\" ]; then\n    seen=$((seen+1))\n    part=$(sed -n \"${{seen}}p\" {box_dir}/lanes/lane-$lane.queue)\n    if [ \"$part\" = STOP ]; then break; fi\n    cd /home/vjeux/shoot/u10s && {box_tinyctl} publish-batch --manifest {box_dir}/p$part/manifest.tsv --results {box_dir}/results/results-p$part.tsv --outdir {box_dir}/p$part > {box_dir}/results/publish-p$part.log 2>&1\n    echo \"$part $(tail -n 1 {box_dir}/results/publish-p$part.log | cut -c1-120)\" >> {box_dir}/lanes/lane-$lane.done\n  else\n    sleep 5\n  fi\ndone\necho FINISHED >> {box_dir}/lanes/lane-$lane.done\n"
    );
    let runner_local = out_dir.join("lane-runner.sh");
    std::fs::write(&runner_local, &runner).map_err(|e| e.to_string())?;
    wsx.push(&runner_local, &format!("{box_dir}/lanes/runner.sh"))?;
    wsx.sh(&format!("cd {box_dir}/lanes && rm -f lane-*.queue lane-*.done; for k in $(seq 0 {}); do : > lane-$k.queue; nohup setsid sh runner.sh $k > lane-$k.log 2>&1 < /dev/null & done; sleep 1; echo started", lanes - 1))?;
    println!("[{:>5.0}s] {lanes} publish lanes running on the box", t0.elapsed().as_secs_f64());

    // --- per part: push, gh upload, unzip + manifest, queue
    let mut queued: Vec<String> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let z = zip_of(part);
        let name = z.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let size = std::fs::metadata(&z).map(|m| m.len()).unwrap_or(0);
        let remote_zip = format!("{box_dir}/zips/{name}");
        let need_gh = !no_github && have.get(&name) != Some(&size);
        let need_pub = !done_part(part);
        if !need_gh && !need_pub {
            println!("[{:>5.0}s] part {part}: on GitHub and published already", t0.elapsed().as_secs_f64());
            continue;
        }
        let t1 = Instant::now();
        wsx.push(&z, &remote_zip)?;
        let mut steps = Vec::new();
        if need_gh {
            let out = wsx.sh(&format!("{gh} release upload {tag} {remote_zip} -R {repo} --clobber 2>&1; echo rc=$?"))?;
            if !out.contains("rc=0") {
                return Err(format!("part {part}: gh upload failed: {}", out.trim()));
            }
            steps.push("github");
        }
        if need_pub {
            // unzip, the manifest: `NN-<name>.Map.Gbx` -> name without the order prefix
            let script = format!("set -e; rm -rf {box_dir}/p{part}; mkdir -p {box_dir}/p{part}; cd {box_dir}/p{part} && unzip -q -o {remote_zip} && rm -f MAPS.tsv.manifest; : > manifest.tsv; for f in *.Map.Gbx; do n=${{f#??-}}; n=${{n%.Map.Gbx}}; printf '%s\\t%s\\n' \"{box_dir}/p{part}/$f\" \"$n\" >> manifest.tsv; done; wc -l < manifest.tsv");
            let out = wsx.sh(&script)?;
            let lane = queued.len() % lanes;
            wsx.sh(&format!("echo {part} >> {box_dir}/lanes/lane-{lane}.queue"))?;
            queued.push(part.clone());
            steps.push("queued");
            println!("[{:>5.0}s] part {part} ({}/{}): {} maps staged → lane {lane}; {} in {:.0} s", t0.elapsed().as_secs_f64(), i + 1, parts.len(), out.trim(), steps.join(" + "), t1.elapsed().as_secs_f64());
        } else {
            println!("[{:>5.0}s] part {part} ({}/{}): {} in {:.0} s", t0.elapsed().as_secs_f64(), i + 1, parts.len(), steps.join(" + "), t1.elapsed().as_secs_f64());
        }
        wsx.sh(&format!("rm -f {remote_zip}"))?;
    }
    for k in 0..lanes {
        wsx.sh(&format!("echo STOP >> {box_dir}/lanes/lane-{k}.queue"))?;
    }
    // --- wait for the lanes
    let t_wait = Instant::now();
    loop {
        let out = wsx.sh(&format!("cat {box_dir}/lanes/lane-*.done 2>/dev/null; true"))?;
        let finished = out.lines().filter(|l| l.trim() == "FINISHED").count();
        let done_parts = out.lines().filter(|l| l.trim() != "FINISHED" && !l.trim().is_empty()).count();
        if finished >= lanes {
            println!("[{:>5.0}s] publish lanes finished ({done_parts} part runs)", t0.elapsed().as_secs_f64());
            break;
        }
        if t_wait.elapsed() > Duration::from_secs(4 * 3600) {
            return Err("publish lanes: no FINISHED after 4 h".into());
        }
        if t_wait.elapsed().as_secs() % 120 < 30 {
            eprintln!("[{:>5.0}s] publishing: {done_parts}/{} part runs done, {finished}/{lanes} lanes finished", t0.elapsed().as_secs_f64(), queued.len());
        }
        std::thread::sleep(Duration::from_secs(30));
    }
    // --- results
    let mut identical_total = 0usize;
    let mut bad_parts: Vec<String> = Vec::new();
    for part in &parts {
        let local = out_dir.join(format!("results-p{part}.tsv"));
        if queued.contains(part) {
            wsx.pull(&format!("{box_dir}/results/results-p{part}.tsv"), &local)?;
        }
        let n = std::fs::read_to_string(&local).map(|t| t.lines().filter(|l| l.ends_with("\tIDENTICAL")).count()).unwrap_or(0);
        identical_total += n;
        if n < 25 {
            bad_parts.push(format!("{part} ({n}/25)"));
        }
    }
    println!("[{:>5.0}s] {identical_total} maps identical on Nadeo; parts short: {}", t0.elapsed().as_secs_f64(), if bad_parts.is_empty() { "none".into() } else { bad_parts.join(", ") });
    // --- the rooms
    if !no_rooms {
        let log = format!("{box_dir}/rooms.log");
        let done = format!("{box_dir}/rooms.done");
        let job = format!("cd /home/vjeux/shoot/u10s && {box_tinyctl} nadeo-here club-rooms --club {club} --folder '{folder}' --room-prefix '{room_prefix}' --parts-dir {box_dir}/results --outdir /home/vjeux/shoot/hunt --no-lock; rc=$?; if [ $rc = 0 ]; then echo OK rooms > {done}; else echo FAILED rc=$rc > {done}; fi");
        wsx.sh(&format!("rm -f {done}; nohup setsid sh -c '{}' > {log} 2>&1 < /dev/null &", job.replace('\'', "'\\''")))?;
        let text = wsx.wait_done(&done, &log, Duration::from_secs(3600), "club-rooms")?;
        println!("[{:>5.0}s] rooms: {}", t0.elapsed().as_secs_f64(), text.trim());
        let rooms_tsv = format!("/home/vjeux/shoot/hunt/club-{club}-rooms-{}.tsv", folder.replace(' ', "_"));
        let _ = wsx.pull(&rooms_tsv, &out_dir.join(format!("club-{club}-rooms.tsv")));
        let _ = wsx.pull(&log, &out_dir.join("rooms.log"));
    }
    if !no_github {
        let present = wsx.sh(&format!("{gh} release view {tag} -R {repo} --json assets --jq '.assets[] | \"\\(.name)\\t\\(.size)\"' 2>/dev/null; true"))?;
        let have: std::collections::BTreeMap<String, u64> = present.lines().filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse().ok().map(|s| (n.trim().to_string(), s)))).collect();
        let mut bad = 0usize;
        for p in &parts {
            let z = zip_of(p);
            let name = z.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let size = std::fs::metadata(&z).map(|m| m.len()).unwrap_or(0);
            if have.get(&name) != Some(&size) {
                bad += 1;
                eprintln!("  VERIFY {name}: {:?} on GitHub, {size} B here", have.get(&name));
            }
        }
        println!("[{:>5.0}s] GitHub https://github.com/{repo}/releases/tag/{tag}: {} assets, {bad} mismatches", t0.elapsed().as_secs_f64(), have.len());
    }
    let _ = bad_parts;
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: &Path) {}

//! `tmrl eval` — the evaluation harness (BRIEF-LEARN L4): a fixed set of maps, fixed seeds, fixed budget → one table
//! per policy: per map, the CONST control's progress, the policy's oracle finishes, cps ≥ 2 rate, median progress
//! fraction, best oracle time and its medal. Every number comes from `rollout::run` (plain oracle per episode).
//!
//! Maps come from DATA's `maps/<uid>/` layout (map.Map.Gbx, map.json with medals, geom.json, ghosts/<rank>-<ms>.Ghost.Gbx).
//! Per map the env reference is built from the SLOWEST ghost as an opaque template (ENV's from-template path),
//! lengthened to `--tape-factor` × AT with the declared time and the validation walltime updated (`ghost trim`,
//! `ghost declare`, `tmrl walltime`). Held-out maps are selected by the INTERFACES split rule unless `--maps` lists uids.

use crate::rollout::{self, RolloutArgs};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
struct MapJson {
    name: String,
    author_ms: Option<i64>,
    gold_ms: Option<i64>,
    silver_ms: Option<i64>,
    bronze_ms: Option<i64>,
}

pub struct EvalArgs {
    pub policy: String,
    pub maps_dir: String,
    pub maps: Vec<String>, // explicit uids; empty = held-out by fnv rule
    pub max_maps: usize,
    pub episodes: usize,
    pub temp: f32,
    pub seed: u64,
    pub tape_factor: f32,
    /// Cap on the template length (ms). MEASURED 2026-09-07: tapes past ~7,000 ticks make every step of that worker fail
    /// with `PROBE-EMPTY … n 7196` (ENV item); 60 s keeps every template under 6,000 ticks.
    pub tape_cap_ms: u32,
    pub out: PathBuf,
    pub work: PathBuf,
    pub server: PathBuf,
    pub shim: PathBuf,
    pub threads_note: usize,
    pub skip_const: bool,
    pub margin_m: f32,
    /// ENV's start-sanity gate: only maps whose `<sanity_dir>/<uid>/env-sanity.json` says
    /// identity_ok && ghost_reproducible && error == "" are evaluated (the others are listed as ungated).
    pub sanity_dir: Option<String>,
}

fn run_cli(cmd: &str, args: &[&str]) -> Result<String, String> {
    let o = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    if !o.status.success() {
        return Err(format!("{cmd} {:?} failed: {}{}", args, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A ghost in `<map>/ghosts/` by the ms in its file name (`<rank>-<ms>.Ghost.Gbx`): the fastest (`fastest = true`) or the
/// slowest. MEASURED 2026-09-07 (BAR M2-0 diagnosis): a ghost's tape reproduces only in its OWN container on most
/// maps (Fall 2024 - 08 / 24, Winter 2025 - 19, Summer 2026 - 02), so the template must be the ghost whose replay
/// is the identity control — the fastest — and that control is run per map before any policy number counts.
pub fn pick_ghost(map_dir: &Path, fastest: bool) -> Result<(PathBuf, i64), String> {
    let mut best: Option<(PathBuf, i64)> = None;
    for e in std::fs::read_dir(map_dir.join("ghosts")).map_err(|e| format!("{}: {e}", map_dir.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.ends_with(".Ghost.Gbx") {
            continue;
        }
        let ms: i64 = name.trim_end_matches(".Ghost.Gbx").split('-').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let better = if fastest { ms < best.as_ref().map(|(_, m)| *m).unwrap_or(i64::MAX) } else { ms > best.as_ref().map(|(_, m)| *m).unwrap_or(-1) };
        // Only a PLAIN-state-word ghost can serve as the identity control: an Action-Key ghost's steer/gas/brake
        // alone do not reproduce its run (BAR CL-0 finding 2), so its replay would fail for the wrong reason.
        let plain = matches!(crate::synth::nonplain_reason(&p), Ok(None));
        if ms > 0 && better && plain {
            best = Some((p, ms));
        }
    }
    best.ok_or_else(|| format!("{}: no ghosts", map_dir.display()))
}

/// Build the env reference for a map: slowest ghost → lengthened template → from-template reference.
pub fn build_reference(map_dir: &Path, work: &Path, tape_ms: u32) -> Result<PathBuf, String> {
    build_reference_from(map_dir, work, tape_ms, true).map(|(r, _)| r)
}

/// `build_reference` returning the donor ghost too (for the identity replay).
pub fn build_reference_from(map_dir: &Path, work: &Path, tape_ms: u32, fastest: bool) -> Result<(PathBuf, PathBuf), String> {
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let (ghost, _) = pick_ghost(map_dir, fastest)?;
    let t1 = work.join("tpl-1.Ghost.Gbx");
    let t2 = work.join("tpl-2.Ghost.Gbx");
    let t3 = work.join("tpl.Ghost.Gbx");
    let reference = work.join("reference.Ghost.Gbx");
    run_cli("ghost", &["trim", ghost.to_str().unwrap(), t1.to_str().unwrap(), "--to", &tape_ms.to_string(), "--declare", &tape_ms.to_string()])?;
    run_cli("ghost", &["declare", t1.to_str().unwrap(), t2.to_str().unwrap(), "--time", &tape_ms.to_string()])?;
    rollout::set_walltime(t2.to_str().unwrap(), t3.to_str().unwrap(), tape_ms)?;
    let tpl = tmenv::template::Template::load(&t3)?;
    let n = tpl.facts().ticks;
    let s: Vec<u8> = (0..n).map(|t| ((((t as i64 * 7919 + 13) % 25) - 12) as i8) as u8).collect();
    tpl.write_with_inputs(&s, &vec![1u8; n], &vec![0u8; n], &reference)?;
    // THE SEED (INPUT's VALIDATION-SEED.md, ENV 9ca9a894): a tape reproduces only under its own validation seed, and a
    // policy's inputs land exactly under seed 0. So: `reference-identity.Ghost.Gbx` keeps the donor's seed (for the
    // identity replay), `reference.Ghost.Gbx` gets seed 0 (for every policy rollout).
    let identity = work.join("reference-identity.Ghost.Gbx");
    std::fs::copy(&reference, &identity).map_err(|e| e.to_string())?;
    tmenv::template::set_validation_seed(&reference, 0).map_err(|e| format!("template seed: {e}"))?;
    Ok((reference, ghost))
}

/// The donor-seed twin of a reference built by `build_reference_from` (for identity replays).
pub fn identity_reference(reference: &Path) -> PathBuf {
    reference.with_file_name("reference-identity.Ghost.Gbx")
}

pub struct MapRow {
    pub uid: String,
    pub name: String,
    pub author_ms: Option<i64>,
    pub length_m: f32,
    pub const_best_s: f32,
    pub episodes: usize,
    pub finishes: usize,
    pub cps2: usize,
    pub median_frac: f32,
    pub best_time_s: Option<f64>,
    pub medal: String,
    pub env_errors: usize,
    pub note: String,
    /// Identity control: the donor ghost's own tape replayed through the env in this template → oracle time.
    pub identity: String,
    pub identity_ok: bool,
}

/// ENV's env-sanity.json verdict: identity_ok && ghost_reproducible && error == "".
pub fn sanity_ok(path: &Path) -> Result<bool, String> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let b = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
    let err = v.get("error").and_then(|x| x.as_str()).unwrap_or("?");
    Ok(b("identity_ok") && b("ghost_reproducible") && err.is_empty())
}

fn medal(t_ms: i64, m: &MapJson) -> String {
    let at = m.author_ms.unwrap_or(i64::MAX);
    if t_ms <= at {
        "AUTHOR".into()
    } else if t_ms <= m.gold_ms.unwrap_or(i64::MAX) {
        "gold".into()
    } else if t_ms <= m.silver_ms.unwrap_or(i64::MAX) {
        "silver".into()
    } else if t_ms <= m.bronze_ms.unwrap_or(i64::MAX) {
        "bronze".into()
    } else {
        "finish".into()
    }
}

pub fn run(a: &EvalArgs) -> Result<Vec<MapRow>, String> {
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    // Map selection.
    let mut uids: Vec<String> = if a.maps.is_empty() {
        let mut v = Vec::new();
        for e in std::fs::read_dir(&a.maps_dir).map_err(|e| format!("{}: {e}", a.maps_dir))? {
            let p = e.map_err(|e| e.to_string())?.path();
            let uid = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if crate::bc::is_heldout_map(&uid) && p.join("geom.json").exists() && p.join("map.Map.Gbx").exists() && p.join("ghosts").exists() {
                if let Some(sd) = &a.sanity_dir {
                    match sanity_ok(&Path::new(sd).join(&uid).join("env-sanity.json")) {
                        Ok(true) => {}
                        Ok(false) => {
                            println!("# {uid}: env-sanity says not sane — skipped");
                            continue;
                        }
                        Err(e) => {
                            println!("# {uid}: no usable env-sanity.json ({e}) — skipped");
                            continue;
                        }
                    }
                }
                v.push(uid);
            }
        }
        v.sort();
        v
    } else {
        a.maps.clone()
    };
    uids.truncate(a.max_maps);
    println!("# tmrl eval — {} maps, {} episodes each at temp {}, policy {}", uids.len(), a.episodes, a.temp, a.policy);
    let mut rows = Vec::new();
    let mut tsv = String::from("uid\tname\tauthor_s\tlength_m\tidentity\tidentity_ok\tconst_best_s\tepisodes\tfinishes\tcps2\tmedian_frac\tbest_time_s\tmedal\tenv_errors\tnote\n");
    for (i, uid) in uids.iter().enumerate() {
        let map_dir = Path::new(&a.maps_dir).join(uid);
        let mj: MapJson = std::fs::read_to_string(map_dir.join("map.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(MapJson { name: uid.clone(), author_ms: None, gold_ms: None, silver_ms: None, bronze_ms: None });
        let geom_path = map_dir.join("geom.json");
        let length = tmenv::Track::load_geom_json(&geom_path).map(|t| t.geom.length()).unwrap_or(0.0);
        let work = a.work.join(uid);
        let at = mj.author_ms.unwrap_or(60_000).max(10_000) as f32;
        let tape_ms = ((at * a.tape_factor) as u32).min(a.tape_cap_ms);
        let mut row = MapRow { uid: uid.clone(), name: mj.name.clone(), author_ms: mj.author_ms, length_m: length, const_best_s: 0.0, episodes: 0, finishes: 0, cps2: 0, median_frac: 0.0, best_time_s: None, medal: "-".into(), env_errors: 0, note: String::new(), identity: "-".into(), identity_ok: false };
        println!("\n## [{}/{}] {} ({uid}) AT {:?} length {:.1} m", i + 1, uids.len(), mj.name, mj.author_ms.map(|x| x as f64 / 1000.0), length);
        let (reference, donor) = match build_reference_from(&map_dir, &work, tape_ms, true) {
            Ok(r) => r,
            Err(e) => {
                row.note = format!("reference build failed: {e}");
                println!("   {}", row.note);
                rows.push(row);
                continue;
            }
        };
        let base = RolloutArgs {
            policy: a.policy.clone(),
            geom: geom_path.to_string_lossy().into_owned(),
            map: map_dir.join("map.Map.Gbx"),
            reference: reference.clone(),
            server: a.server.clone(),
            shim: a.shim.clone(),
            work: work.join("env"),
            out: a.out.join(uid),
            episodes: a.episodes,
            max_ticks: (tape_ms / 10) as usize - 60,
            max_steps: (tape_ms / 100) as usize,
            temp: a.temp,
            seed: a.seed,
            verbose: false,
            no_oracle: false,
            const_ctrl: false,
            tape: None,
            tape_shift_ms: 0,
            no_cut: false,
            margin_m: a.margin_m,
            refs: None,
        };
        // IDENTITY CONTROL first: the donor's own tape through the env in this template must reproduce its run.
        {
            let mut idc = base_copy(&base);
            idc.episodes = 1;
            idc.reference = identity_reference(&reference);
            idc.tape = Some(donor.to_string_lossy().into_owned());
            idc.no_cut = true;
            idc.out = a.out.join(uid).join("identity");
            idc.work = work.join("env-identity");
            match rollout::run(&idc) {
                Ok(r) => {
                    let donor_ms: i64 = donor.file_name().and_then(|s| s.to_str()).and_then(|s| s.trim_end_matches(".Ghost.Gbx").split('-').nth(1)).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let got = r.first().and_then(|x| x.oracle_finish_s);
                    row.identity = got.map(|t| format!("{t:.3}")).unwrap_or_else(|| r.first().map(|x| x.oracle.clone()).unwrap_or("-".into()));
                    row.identity_ok = got.map(|t| ((t * 1000.0).round() as i64 - donor_ms).abs() <= 20).unwrap_or(false);
                    println!("   identity replay ({}): oracle {} → {}", donor.file_name().unwrap().to_string_lossy(), row.identity, if row.identity_ok { "OK" } else { "FAIL — this map's numbers do not count" });
                }
                Err(e) => {
                    row.identity = format!("error: {e}");
                    println!("   identity replay failed: {e}");
                }
            }
        }
        if !a.skip_const {
            let mut c = base_copy(&base);
            c.episodes = 1;
            c.const_ctrl = true;
            c.out = a.out.join(uid).join("const");
            c.work = work.join("env-const");
            match rollout::run(&c) {
                Ok(r) => row.const_best_s = r.first().map(|x| x.best_s).unwrap_or(0.0),
                Err(e) => row.note = format!("const failed: {e}; "),
            }
        }
        match rollout::run(&base) {
            Err(e) => {
                row.note.push_str(&format!("rollout failed: {e}"));
                println!("   {}", row.note);
            }
            Ok(res) => {
                row.episodes = res.len();
                row.finishes = res.iter().filter(|r| r.oracle_finish_s.is_some()).count();
                row.cps2 = res.iter().filter(|r| r.oracle_cps.map(|c| c >= 2).unwrap_or(false) || r.oracle_finish_s.is_some()).count();
                let mut fr: Vec<f32> = res.iter().map(|r| if length > 0.0 { (r.best_s / length).min(1.0) } else { 0.0 }).collect();
                fr.sort_by(|x, y| x.partial_cmp(y).unwrap());
                row.median_frac = fr.get(fr.len() / 2).copied().unwrap_or(0.0);
                row.best_time_s = res.iter().filter_map(|r| r.oracle_finish_s).fold(None, |m: Option<f64>, t| Some(m.map_or(t, |x| x.min(t))));
                row.medal = row.best_time_s.map(|t| medal((t * 1000.0).round() as i64, &mj)).unwrap_or("-".into());
                row.env_errors = res.iter().filter(|r| r.oracle.starts_with("skipped") && r.done.is_none()).count();
            }
        }
        println!(
            "   CONST {:.0} m | policy: finishes {}/{} cps≥2 {} median progress {:.2} best {} {}",
            row.const_best_s, row.finishes, row.episodes, row.cps2, row.median_frac, row.best_time_s.map(|t| format!("{t:.3}")).unwrap_or("-".into()), row.medal
        );
        tsv.push_str(&format!("{}\t{}\t{}\t{:.1}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\n", row.uid, row.name, row.author_ms.map(|x| format!("{:.3}", x as f64 / 1000.0)).unwrap_or("-".into()), row.length_m, row.identity, row.identity_ok, row.const_best_s, row.episodes, row.finishes, row.cps2, row.median_frac, row.best_time_s.map(|t| format!("{t:.3}")).unwrap_or("-".into()), row.medal, row.env_errors, row.note));
        std::fs::write(a.out.join("table.tsv"), &tsv).map_err(|e| e.to_string())?;
        rows.push(row);
    }
    // Summary over the maps whose identity control PASSED (the others are listed, never counted).
    let unsane: Vec<String> = rows.iter().filter(|r| !r.identity_ok).map(|r| format!("{} ({})", r.name, r.identity)).collect();
    let sane: Vec<&MapRow> = rows.iter().filter(|r| r.identity_ok && r.episodes > 0).collect();
    let n = sane.len().max(1);
    let fin_maps = sane.iter().filter(|r| r.finishes > 0).count();
    let eps: usize = sane.iter().map(|r| r.episodes).sum();
    let fins: usize = sane.iter().map(|r| r.finishes).sum();
    let cps2: usize = sane.iter().map(|r| r.cps2).sum();
    println!("\nIDENTITY FAILED on {} map(s), excluded: {:?}", unsane.len(), unsane);
    let rows_all = rows;
    let rows: Vec<MapRow> = rows_all.into_iter().filter(|r| r.identity_ok).collect();
    let mut fr: Vec<f32> = rows.iter().filter(|r| r.episodes > 0).map(|r| r.median_frac).collect();
    fr.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut cf: Vec<f32> = rows.iter().filter(|r| r.episodes > 0 && r.length_m > 0.0).map(|r| (r.const_best_s / r.length_m).min(1.0)).collect();
    cf.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!(
        "\nSUMMARY {} identity-sane maps evaluated: maps with ≥1 oracle finish {fin_maps}/{n}; episodes finishing {fins}/{eps}; cps≥2 {cps2}/{eps}; median (over maps) of median progress fraction {:.3} (CONST {:.3}); medals {:?}",
        n,
        fr.get(fr.len() / 2).copied().unwrap_or(0.0),
        cf.get(cf.len() / 2).copied().unwrap_or(0.0),
        rows.iter().fold(std::collections::BTreeMap::new(), |mut m, r| { *m.entry(r.medal.clone()).or_insert(0usize) += 1; m })
    );
    let _ = a.threads_note;
    Ok(rows)
}

fn base_copy(b: &RolloutArgs) -> RolloutArgs {
    RolloutArgs {
        policy: b.policy.clone(),
        geom: b.geom.clone(),
        map: b.map.clone(),
        reference: b.reference.clone(),
        server: b.server.clone(),
        shim: b.shim.clone(),
        work: b.work.clone(),
        out: b.out.clone(),
        episodes: b.episodes,
        max_ticks: b.max_ticks,
        max_steps: b.max_steps,
        temp: b.temp,
        seed: b.seed,
        verbose: b.verbose,
        no_oracle: b.no_oracle,
        const_ctrl: b.const_ctrl,
        tape: b.tape.clone(),
        tape_shift_ms: b.tape_shift_ms,
        no_cut: b.no_cut,
        margin_m: b.margin_m,
        refs: b.refs.clone(),
    }
}

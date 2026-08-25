//! tmsite -- the TM2020 TAS toolchain's presentation end, in Rust.
//!
//!   site     build the full self-contained 3D minisite
//!   compact  build the packed-binary variant of the same page
//!   tick     export a ghost's inputs as a TICK input script
//!   verify   round-trip a TICK script against the ghost it came from
//!   stats    measure a built page (either variant) and report what is in it
//!   serve    serve a directory over HTTP, for fetching a built page
//!   refresh  fetch every map's live human leaderboard and bank the responses
//!   acquire  fetch one map, its live board, and every available replay seed
//!   records  join that bank with what the pages claim, and write the table
//!   names    is the title we publish for a map the map's own name?
//!
//! Manual argument parsing; the only dependency is the workspace's `gbx` crate.

use tmsite::tick::secs;
use tmsite::{compact, names, records, serve, site, stats, tick};

const USAGE: &str = "\
usage: tmsite <command> [flags]

  site     [--dir D] [--out F] [--stride N]
           full page, every sample as rounded JSON floats   (default stride 1)
  compact  [--dir D] [--out F] [--stride N] [--pick K]
           packed binary + base64 page                      (default stride 3)
  tick     --ghost G [--out F] [--archive N] [--raw] [--seed N]
           TICK input script on stdout (or --out)
  verify   --ghost G [--script F] [--archive N] [--raw]
           re-read the script and compare per tick with the ghost
  stats    --html F [--html F2 ...]
           measure built pages and print their numbers
  serve    --root D [--port N] [--requests N]
  refresh  --root D --bank DIR [--proxy URL] [--sleep MS] [--ua S]
           GET every map's live board and bank the raw responses
  acquire  --map-id ID --out DIR [--replays N] [--proxy URL] [--sleep MS] [--ua S]
           GET one map, its live board, and up to N available replay seeds
  records  --root D --bank DIR [--prev TSV] [--out F] [--tsv F] [--fetched S] [--detail ID]
           the leaderboard table, from the bank and the pages; no network
  names    --root D --bank DIR [--headers TSV] [--out F]
           what each map is PUBLISHED as, beside its own header name and
           trackmania.io's; --headers is a `tmmaps header --names` TSV, and
           without one every map is UNVERIFIABLE. No network.

defaults: --dir /tmp/entrec/paths
";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprint!("{}", USAGE);
        std::process::exit(2);
    }
    let cmd = args[1].clone();
    let mut i = 2;
    let mut dir = "/tmp/entrec/paths".to_string();
    let mut out: Option<String> = None;
    let mut stride: Option<usize> = None;
    let mut pick = 0usize;
    let mut ghost_path: Option<String> = None;
    let mut script: Option<String> = None;
    let mut archive = 0usize;
    let mut raw = false;
    let mut seed: Option<u32> = None;
    let mut htmls: Vec<String> = Vec::new();
    let mut root = ".".to_string();
    let mut port = 8731u16;
    let mut requests = 0usize;
    let mut bank: Option<String> = None;
    let mut prev: Option<String> = None;
    let mut tsv: Option<String> = None;
    let mut proxy = "http://fwdproxy:8080".to_string();
    let mut sleep_ms = 1800u64;
    let mut ua = records::DEFAULT_UA.to_string();
    let mut fetched = String::new();
    let mut detail: Option<i64> = None;
    let mut map_id: Option<i64> = None;
    let mut replays = usize::MAX;
    let mut headers: Option<String> = None;

    while i < args.len() {
        let a = args[i].clone();
        macro_rules! next {
            () => {{
                i += 1;
                match args.get(i) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("{} needs a value", a);
                        std::process::exit(2);
                    }
                }
            }};
        }
        macro_rules! next_num {
            ($what:expr) => {{
                let v = next!();
                match v.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("{} wants {}", a, $what);
                        std::process::exit(2);
                    }
                }
            }};
        }
        match a.as_str() {
            "--dir" => dir = next!(),
            "--out" => out = Some(next!()),
            "--stride" => stride = Some(next_num!("an integer")),
            "--pick" => pick = next_num!("an integer"),
            "--ghost" => ghost_path = Some(next!()),
            "--script" => script = Some(next!()),
            "--archive" => archive = next_num!("an integer"),
            "--raw" => raw = true,
            "--seed" => seed = Some(next_num!("a u32")),
            "--html" => htmls.push(next!()),
            "--root" => root = next!(),
            "--port" => port = next_num!("an integer"),
            "--requests" => requests = next_num!("an integer"),
            "--bank" => bank = Some(next!()),
            "--prev" => prev = Some(next!()),
            "--tsv" => tsv = Some(next!()),
            "--proxy" => proxy = next!(),
            "--sleep" => sleep_ms = next_num!("milliseconds"),
            "--ua" => ua = next!(),
            "--fetched" => fetched = next!(),
            "--detail" => detail = Some(next_num!("a map id")),
            "--map-id" => map_id = Some(next_num!("a map id")),
            "--replays" => replays = next_num!("an integer"),
            "--headers" => headers = Some(next!()),
            "-h" | "--help" => {
                print!("{}", USAGE);
                return;
            }
            other => {
                eprintln!("unknown flag {}", other);
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let r: Result<(), String> = match cmd.as_str() {
        "site" => site::build(&site::Opts {
            dir,
            out: out.unwrap_or_else(|| "/tmp/tm_lines.html".into()),
            stride: stride.unwrap_or(1),
        })
        .map(|m| println!("{}", m)),
        "compact" => compact::build(&compact::Opts {
            dir,
            out: out.unwrap_or_else(|| "/tmp/tm_compact.html".into()),
            stride: stride.unwrap_or(3),
            pick,
        })
        .map(|m| println!("{}", m)),
        "tick" => run_tick(ghost_path, archive, raw, seed, out),
        "verify" => run_verify(ghost_path, script, archive, raw),
        "stats" => run_stats(&htmls),
        "serve" => serve::serve(&root, port, requests),
        "refresh" => match bank {
            Some(bank) => records::refresh(&records::Fetch { root, bank, proxy, sleep_ms, ua })
                .map(|m| println!("{}", m)),
            None => Err("refresh needs --bank".to_string()),
        },
        "acquire" => match (map_id, out) {
            (Some(id), Some(out)) => records::acquire(&records::Acquire {
                id, out, proxy, sleep_ms, ua, replay_limit: replays,
            }).map(|m| println!("{}", m)),
            (None, _) => Err("acquire needs --map-id".to_string()),
            (_, None) => Err("acquire needs --out".to_string()),
        },
        "records" => match bank {
            Some(bank) => records::records(&records::Table { root, bank, prev, out, tsv, fetched, detail })
                .map(|m| eprintln!("{}", m)),
            None => Err("records needs --bank".to_string()),
        },
        "names" => match bank {
            Some(bank) => names::run(&names::Opts { root, bank, headers, out })
                .map(|m| println!("{}", m)),
            None => Err("names needs --bank".to_string()),
        },
        other => {
            eprintln!("unknown command {:?}\n{}", other, USAGE);
            std::process::exit(2);
        }
    };
    if let Err(e) = r {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn head(v: &[usize]) -> String {
    format!(
        "{:?}{}",
        &v[..v.len().min(5)],
        if v.len() > 5 { " ..." } else { "" }
    )
}

fn run_tick(
    ghost_path: Option<String>,
    archive: usize,
    raw: bool,
    seed: Option<u32>,
    out: Option<String>,
) -> Result<(), String> {
    let path = ghost_path.ok_or("tick needs --ghost")?;
    let o = tick::Opts { path, archive, raw, seed };
    let e = tick::export(&o)?;
    if !e.out_of_range.is_empty() {
        eprintln!(
            "warning: {} tick(s) hold steer -128, outside TICK's -127..127{}  (first: {})",
            e.out_of_range.len(),
            if raw { " -- emitted verbatim (--raw)" } else { " -- clamped to -127" },
            head(&e.out_of_range)
        );
    }
    if !e.respawns.is_empty() || !e.standing_respawns.is_empty() {
        // Not a warning any more: these are emitted as TICK `respawn` /
        // `srespawn` actions and `tmsite verify` checks them tick for tick.
        eprintln!(
            "{} respawn + {} standing-respawn input(s) encoded  (respawn ticks: {})",
            e.respawns.len(),
            e.standing_respawns.len(),
            head(&e.respawns)
        );
    }
    match out {
        Some(f) => {
            std::fs::write(&f, format!("{}\n", e.text)).map_err(|x| format!("write {}: {}", f, x))?;
            eprintln!(
                "wrote {}  ({} ticks, {} lines, start offset {} s, declared {} s)",
                f,
                e.ticks,
                e.text.lines().count(),
                secs(e.start_offset_ms as i64),
                e.race_time_ms.map(|v| secs(v as i64)).unwrap_or_else(|| "unknown".into())
            );
        }
        None => println!("{}", e.text),
    }
    Ok(())
}

fn run_verify(
    ghost_path: Option<String>,
    script: Option<String>,
    archive: usize,
    raw: bool,
) -> Result<(), String> {
    let path = ghost_path.ok_or("verify needs --ghost")?;
    let o = tick::Opts { path: path.clone(), archive, raw, seed: None };
    let text = match &script {
        Some(f) => std::fs::read_to_string(f).map_err(|e| format!("read {}: {}", f, e))?,
        None => tick::export(&o)?.text,
    };
    let d = tick::verify(&o, &text)?;
    let lines = text.lines().filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).count();
    println!(
        "{}\n  script            {}\n  ticks compared    {}\n  action lines      {}\n  steer mismatch    {}\n  accel mismatch    {}\n  brake mismatch    {}\n  respawn mismatch  {}\n  srespawn mismatch {}\n  result            {}",
        path,
        script.unwrap_or_else(|| "<freshly exported>".into()),
        d.ticks,
        lines,
        d.steer_bad.len(),
        d.accel_bad.len(),
        d.brake_bad.len(),
        d.respawn_bad.len(),
        d.srespawn_bad.len(),
        if d.is_exact() { "EXACT MATCH" } else { "MISMATCH" }
    );
    if !d.is_exact() {
        return Err("round trip failed".into());
    }
    Ok(())
}

fn run_stats(htmls: &[String]) -> Result<(), String> {
    if htmls.is_empty() {
        return Err("stats needs at least one --html".into());
    }
    for h in htmls {
        let text = std::fs::read_to_string(h).map_err(|e| format!("read {}: {}", h, e))?;
        let s = stats::analyse(&text)?;
        print!("{}", s.report(h));
    }
    Ok(())
}

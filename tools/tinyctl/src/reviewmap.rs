//! `tinyctl review-map MAP.Map.Gbx [--name "…"] [--to CHANNEL] [--note "…"]` — a map
//! for a HUMAN to look at, made safe first.
//!
//! The 2026-10-09 slip: a converted U10S 32 sent to the plastic hunters' DM carried
//! the live Plastic 32 uid — "test uid" in the message, a live id in the file. Anyone
//! uploading it would have replaced the live map. A review copy therefore ALWAYS gets
//! a `Test` uid prefix (the game's uid lookup is by the whole string, so `Test` + the
//! original tail collides with nothing online), a `[TEST]` name suffix, and the
//! publish guard refuses it anyway. Then it is pushed to the box's review folder and,
//! with `--to`, sent to the Discord channel through `discordctl`.
//!
//! Nothing here publishes. Nothing here can.

use std::path::PathBuf;
use std::process::Command;

pub fn cmd(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    // `args` = [cmd, MAP, flags…] (tinyctl strips the program name)
    let Some(map) = args.get(1).filter(|a| !a.starts_with("--")) else {
        eprintln!("review-map MAP.Map.Gbx [--name N] [--to CHANNEL] [--note TEXT] [--wsx PATH]");
        return 2;
    };
    let map = PathBuf::from(map);
    let hdr = match tmmaps::header::read(&map.to_string_lossy()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}: {e}", map.display());
            return 2;
        }
    };
    let old = hdr.uid.clone();
    if old.len() < 5 {
        eprintln!("{}: uid `{old}` too short to re-id", map.display());
        return 2;
    }
    let new_uid = format!("Test{}", &old[4..]);
    let base = f("--name").unwrap_or_else(|| hdr.name.clone());
    let name = if base.contains("[TEST]") { base.clone() } else { format!("{base} [TEST]") };
    let safe = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '[' || c == ']' || c == ' ' { c } else { '_' }).collect::<String>().replace(' ', "_");
    let out = std::env::temp_dir().join(format!("{safe}.Map.Gbx"));
    // two passes: the name is a variable-length splice and the uid a model rename — the
    // map library wants them in separate write/reload rounds
    let tmp = out.with_extension("uid.Gbx");
    let mut m = tmmaps::map::MapFile::load(&map);
    m.set_map_uid(&new_uid);
    if let Err(e) = m.write_to(&tmp) {
        eprintln!("{}: {e}", tmp.display());
        return 2;
    }
    let mut m = tmmaps::map::MapFile::load(&tmp);
    m.set_map_name(&hdr.name, &name);
    if let Err(e) = m.write_to(&out) {
        eprintln!("{}: {e}", out.display());
        return 2;
    }
    let _ = std::fs::remove_file(&tmp);
    let back = tmmaps::header::read(&out.to_string_lossy()).ok();
    println!("review copy: {}\n  uid {old} -> {}\n  name {:?}", out.display(), back.as_ref().map(|h| h.uid.as_str()).unwrap_or("?"), back.as_ref().map(|h| h.name.as_str()).unwrap_or("?"));
    if back.as_ref().map(|h| h.uid != new_uid).unwrap_or(true) {
        eprintln!("review-map: the re-id did not take — NOT sending");
        return 1;
    }
    // to the box's review folder, then Discord
    let wsx = f("--wsx").unwrap_or_else(|| format!("{}/bin/wsx", std::env::var("HOME").unwrap_or_default()));
    let remote = format!("/tmp/review/{}", out.file_name().unwrap_or_default().to_string_lossy());
    let push = Command::new(&wsx).arg("push").arg(&out).arg(&remote).output();
    if !push.map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("review-map: wsx push failed — the copy is at {}", out.display());
        return 1;
    }
    if let Some(chan) = f("--to") {
        let note = f("--note").unwrap_or_else(|| format!("{name} - review copy, test uid, do not upload"));
        let cmd = format!("~/bin/discordctl send {chan} --file '{remote}' '{}'", note.replace('\'', ""));
        let sent = Command::new(&wsx).arg("sh").arg(&cmd).output();
        match sent {
            Ok(o) if o.status.success() => println!("sent to {chan}: {}", String::from_utf8_lossy(&o.stdout).lines().last().unwrap_or("")),
            _ => {
                eprintln!("review-map: discord send failed; the file is on the box at {remote}");
                return 1;
            }
        }
    }
    0
}

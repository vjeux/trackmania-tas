//! What the 10 ms input tape says, in race time.
//!
//! `ghost tape stats` counts the respawns; it does not say WHEN. On a map
//! whose one recording is 110 respawned attempts, when is the whole question:
//! a respawn is a hard reset of the car to its own crossing state at the last
//! checkpoint, so it is the only place where a re-simulation that has drifted
//! off a recording can be put back onto it for free.
//!
//! Reads the `gtape` text emitted by `ghost tape extract`, which is full
//! fidelity and round-trips byte for byte, so nothing here is a second
//! implementation of the packet format — it is a reader of the one dump.

pub struct Tape {
    pub start_offset_ms: i64,
    pub respawns: Vec<i64>,
    pub ticks: i64,
}

pub fn read(path: &str) -> Result<Tape, String> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut t = Tape { start_offset_ms: 0, respawns: Vec::new(), ticks: 0 };
    for line in std::io::BufReader::new(f).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.starts_with("@archive") {
            for kv in line.split_whitespace() {
                if let Some(v) = kv.strip_prefix("start_offset_ms=") {
                    t.start_offset_ms = v.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if !line.starts_with("t=") {
            continue;
        }
        t.ticks += 1;
        if !line.contains("respawn=1") {
            continue;
        }
        let tick: i64 = line[2..].split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
        t.respawns.push(tick);
    }
    Ok(t)
}

pub fn report(path: &str) {
    let t = match read(path) {
        Ok(t) => t,
        Err(e) => { eprintln!("{}", e); std::process::exit(1) }
    };
    println!(
        "{} ticks, start_offset {} ms, {} respawn ticks",
        t.ticks, t.start_offset_ms, t.respawns.len()
    );
    println!("{:>6} {:>10} {:>12}", "n", "tick", "race_s");
    // group consecutive ticks: one press can hold the bit for several ticks
    let mut i = 0;
    let mut n = 0;
    while i < t.respawns.len() {
        let mut j = i;
        while j + 1 < t.respawns.len() && t.respawns[j + 1] == t.respawns[j] + 1 {
            j += 1;
        }
        n += 1;
        let race = (t.respawns[i] * 10 + t.start_offset_ms) as f64 / 1000.0;
        println!(
            "{:>6} {:>10} {:>12.3}{}",
            n, t.respawns[i], race,
            if j > i { format!("  (held {} ticks)", j - i + 1) } else { String::new() }
        );
        i = j + 1;
    }
}

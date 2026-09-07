//! `shootctl perfsum PERF.csv [--every N] [--t0 "MM/DD/YYYY HH:MM:SS.mmm"]`
//! — the per-second `typeperf` counters `shootctl loadprof` recorded, as one
//! readable table: the game's CPU (process, and its busiest THREADS by id),
//! its I/O, page faults, working set, the GPU engines of its pid, the disk.
//!
//! The CSV is PDH's: a header row of quoted counter paths
//! (`\\HOST\Process(Trackmania)\% Processor Time`, `\\HOST\Thread(Trackmania/7)\ID Thread`
//! …) and one row per sample with the timestamp first. A `% Processor Time`
//! of 100 is ONE logical CPU flat out (the box has 20).

use std::collections::BTreeMap;

fn parse_row(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut inq = false;
    for c in line.chars() {
        match c {
            '"' => inq = !inq,
            ',' if !inq => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

pub fn run(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("usage: shootctl perfsum PERF.csv [--every N] [--threads K]");
        return 2;
    };
    let val = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let every: usize = val("--every").and_then(|s| s.parse().ok()).unwrap_or(1);
    let nthreads: usize = val("--threads").and_then(|s| s.parse().ok()).unwrap_or(4);
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{path}: {e}");
            return 1;
        }
    };
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        eprintln!("{path}: empty");
        return 1;
    };
    let cols = parse_row(header);
    // strip the machine prefix: \\HOST\Object(inst)\Counter -> Object(inst)\Counter
    let names: Vec<String> = cols
        .iter()
        .map(|c| {
            let c = c.trim();
            match c.strip_prefix("\\\\") {
                Some(rest) => rest.splitn(2, '\\').nth(1).unwrap_or(rest).to_string(),
                None => c.to_string(),
            }
        })
        .collect();
    let find = |suffix: &str| names.iter().position(|n| n.eq_ignore_ascii_case(suffix));
    let c_cpu = find("Process(Trackmania)\\% Processor Time");
    let c_user = find("Process(Trackmania)\\% User Time");
    let c_priv = find("Process(Trackmania)\\% Privileged Time");
    let c_rd = find("Process(Trackmania)\\IO Read Bytes/sec");
    let c_rdops = find("Process(Trackmania)\\IO Read Operations/sec");
    let c_ws = find("Process(Trackmania)\\Working Set");
    let c_pf = find("Process(Trackmania)\\Page Faults/sec");
    let c_thr = find("Process(Trackmania)\\Thread Count");
    let c_disk = find("PhysicalDisk(_Total)\\Disk Read Bytes/sec");
    let c_all = find("Processor(_Total)\\% Processor Time");
    // thread columns: Thread(Trackmania/N)\% Processor Time  +  \ID Thread
    let mut th_cpu: BTreeMap<String, usize> = BTreeMap::new();
    let mut th_id: BTreeMap<String, usize> = BTreeMap::new();
    let mut gpu: Vec<(String, usize)> = Vec::new();
    for (i, n) in names.iter().enumerate() {
        if let Some(rest) = n.strip_prefix("Thread(Trackmania/") {
            let inst = rest.split(')').next().unwrap_or("").to_string();
            if n.ends_with("\\% Processor Time") {
                th_cpu.insert(inst, i);
            } else if n.ends_with("\\ID Thread") {
                th_id.insert(inst, i);
            }
        } else if n.starts_with("GPU Engine(") && n.ends_with("\\Utilization Percentage") {
            let inst = n["GPU Engine(".len()..].split(')').next().unwrap_or("").to_string();
            let eng = inst.rsplit("engtype_").next().unwrap_or(&inst).to_string();
            gpu.push((eng, i));
        }
    }
    println!("{path}: {} columns, {} thread columns, {} GPU engine columns", names.len(), th_cpu.len(), gpu.len());
    let g = |row: &[String], c: Option<usize>| c.and_then(|i| row.get(i)).and_then(|s| num(s));
    println!(
        "{:>4} {:>6} {:>5} {:>5} {:>7} {:>6} {:>7} {:>6} {:>4} {:>6} {:>7} {:>5}  {}",
        "t", "cpu%", "usr%", "prv%", "rdMB/s", "rdops", "pf/s", "WS-MB", "thr", "gpu%", "diskMB", "all%", "busiest threads (tid cpu%)"
    );
    let mut t = 0usize;
    let mut sum_cpu = 0.0;
    let mut n_rows = 0usize;
    let mut th_total: BTreeMap<String, f64> = BTreeMap::new();
    for line in lines {
        let row = parse_row(line);
        if row.len() < 2 {
            continue;
        }
        n_rows += 1;
        let cpu = g(&row, c_cpu).unwrap_or(0.0);
        sum_cpu += cpu;
        let mut ths: Vec<(String, f64)> = th_cpu
            .iter()
            .filter_map(|(inst, &ci)| {
                let v = num(row.get(ci)?)?;
                let id = th_id.get(inst).and_then(|&ii| row.get(ii)).and_then(|s| num(s)).map(|x| format!("{}", x as u64)).unwrap_or_else(|| format!("#{inst}"));
                *th_total.entry(id.clone()).or_default() += v;
                Some((id, v))
            })
            .collect();
        ths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let gpu_max = gpu.iter().filter_map(|(e, ci)| num(row.get(*ci)?).map(|v| (e.clone(), v))).fold((String::new(), 0.0f64), |acc, (e, v)| if v > acc.1 { (e, v) } else { acc });
        if t % every == 0 {
            let top: Vec<String> = ths.iter().take(nthreads).filter(|(_, v)| *v >= 1.0).map(|(id, v)| format!("{id} {v:.0}")).collect();
            println!(
                "{:>4} {:>6.0} {:>5.0} {:>5.0} {:>7.1} {:>6.0} {:>7.0} {:>6.0} {:>4.0} {:>6} {:>7.1} {:>5.0}  {}",
                t,
                cpu,
                g(&row, c_user).unwrap_or(0.0),
                g(&row, c_priv).unwrap_or(0.0),
                g(&row, c_rd).unwrap_or(0.0) / 1e6,
                g(&row, c_rdops).unwrap_or(0.0),
                g(&row, c_pf).unwrap_or(0.0),
                g(&row, c_ws).unwrap_or(0.0) / 1e6,
                g(&row, c_thr).unwrap_or(0.0),
                if gpu_max.0.is_empty() { "-".to_string() } else { format!("{:.0}{}", gpu_max.1, gpu_max.0.chars().take(2).collect::<String>()) },
                g(&row, c_disk).unwrap_or(0.0) / 1e6,
                g(&row, c_all).unwrap_or(0.0),
                top.join(", ")
            );
        }
        t += 1;
    }
    let mut tot: Vec<(&String, &f64)> = th_total.iter().collect();
    tot.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "\n{n_rows} s; process CPU average {:.0}% (of one logical CPU); thread CPU-seconds: {}",
        sum_cpu / n_rows.max(1) as f64,
        tot.iter().take(8).map(|(id, v)| format!("{id} {:.1}", *v / 100.0)).collect::<Vec<_>>().join(", ")
    );
    0
}

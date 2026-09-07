//! `mapgeom etlsum` -- read a Windows Performance Recorder trace (as
//! `tracerpt -of CSV` dumps it) and say where a process spent its CPU time:
//! per thread, per second, per module, per function -- with call stacks.
//!
//! ```text
//! mapgeom etlsum TRACE.csv --exe Trackmania.exe [--pid N] [--t0 UNIX_MS]
//!                [--from S] [--to S] [--bin S] [--top N] [--tid T]
//!                [--folded OUT.txt] [--exe-only]
//! ```
//!
//! What the CSV holds (kernel `NT Kernel Logger` events, `shootctl loadprof`'s
//! `Light` profile: sampled profile + stacks + process/thread/image rundown):
//!
//! * `PerfInfo, SampleProf`: one row per CPU per sampling interval (1 ms) for
//!   a CPU that was NOT idle: `InstructionPointer, ThreadId, Count`;
//! * `StackWalk, Stack`: the stack of the event just before it on the same
//!   thread: `EventTimeStamp, StackProcess, StackThread, frame0 (= the IP),
//!   frame1 (return address in the caller), ...`. The walk goes through the
//!   game exe fine (17 frames measured 2026-09-07) although VMProtect
//!   scrambled the exe's `.pdata`;
//! * `Image, DCStart|Load|DCEnd`: `ImageBase, ImageSize, ProcessId, ...,
//!   "FileName"` -- what turns an address into `module+offset`.
//!
//! The game has no symbols. Functions are named by their START: frame k lives
//! in the function that frame k+1's `call` targets (a direct `call rel32`
//! decodes from the exe bytes; an indirect one leaves the function unknown and
//! the sample is bucketed by its 256-byte page). Every exe address is printed
//! as an RVA (`Trackmania.exe+0x…`) and as the objdump address (image base
//! 0x140000000) so `mapgeom crash --disasm` / objdump take it as is.
//!
//! `--t0 UNIX_MS` (the `T0 unix_ms` line of `loadprof`'s timeline) puts the
//! sample times on the load's clock: the per-bin table shows the phases.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};

use crate::minidump::{decode_call, Pe};

const USAGE: &str = "mapgeom etlsum TRACE.csv --exe Trackmania.exe [--pid N] [--t0 UNIX_MS] [--from S] [--to S] [--bin S] [--top N] [--tid T] [--folded OUT] [--exe-only]";

struct Image {
    base: u64,
    size: u64,
    name: String,
}

#[derive(Default)]
struct Thread {
    samples: u64,
    kernel: u64,
    bins: BTreeMap<i64, u64>,
    modules: HashMap<String, u64>,
    /// leaf function start (or 256 B page when unknown) -> count
    leaf_fn: HashMap<(u64, bool), u64>,
    /// exact leaf IP (exe only) -> count
    leaf_ip: HashMap<u64, u64>,
    /// return-address call sites (exe frames, deduped per sample) -> count
    incl_site: HashMap<u64, u64>,
    /// function starts (exe, deduped per sample) -> count
    incl_fn: HashMap<u64, u64>,
    /// exact exe-frame stacks -> count
    stacks: HashMap<Vec<u64>, u64>,
    folded: HashMap<String, u64>,
    no_stack: u64,
}

struct Pending {
    ft: u64,
    ip: u64,
}

fn hex(s: &str) -> Option<u64> {
    let t = s.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    u64::from_str_radix(t, 16).ok()
}

fn ft_to_unix_ms(ft: u64) -> i64 {
    (ft as i64 - 116_444_736_000_000_000) / 10_000
}

fn field<'a>(f: &[&'a str], i: usize) -> &'a str {
    f.get(i).map(|s| s.trim()).unwrap_or("")
}

pub fn run(args: &[String]) -> Result<(), String> {
    // args[0] == "etlsum"
    let csv = args.get(1).filter(|a| !a.starts_with("--")).ok_or_else(|| USAGE.to_string())?;
    let mut exe: Option<String> = None;
    let mut pid: Option<u64> = None;
    let mut t0: Option<i64> = None;
    let (mut from, mut to) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut bin = 5.0f64;
    let mut top = 25usize;
    let mut only_tid: Option<u64> = None;
    let mut folded: Option<String> = None;
    let mut exe_only = false;
    let mut i = 2;
    let mut next = |i: &mut usize, k: &str| -> Result<String, String> {
        *i += 1;
        args.get(*i).cloned().ok_or_else(|| format!("{k} needs a value"))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--exe" => exe = Some(next(&mut i, "--exe")?),
            "--pid" => pid = Some(next(&mut i, "--pid")?.parse().map_err(|_| "--pid N")?),
            "--t0" => t0 = Some(next(&mut i, "--t0")?.parse().map_err(|_| "--t0 UNIX_MS")?),
            "--from" => from = next(&mut i, "--from")?.parse().map_err(|_| "--from S")?,
            "--to" => to = next(&mut i, "--to")?.parse().map_err(|_| "--to S")?,
            "--bin" => bin = next(&mut i, "--bin")?.parse().map_err(|_| "--bin S")?,
            "--top" => top = next(&mut i, "--top")?.parse().map_err(|_| "--top N")?,
            "--tid" => only_tid = Some(next(&mut i, "--tid")?.parse().map_err(|_| "--tid T")?),
            "--folded" => folded = Some(next(&mut i, "--folded")?),
            "--exe-only" => exe_only = true,
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
        i += 1;
    }
    let pe = match &exe {
        Some(p) => Some(Pe::open(p)?),
        None => None,
    };
    let file = std::fs::File::open(csv).map_err(|e| format!("{csv}: {e}"))?;
    let mut rd = BufReader::with_capacity(1 << 20, file);
    let mut line = String::new();

    let mut images: Vec<Image> = Vec::new(); // of the chosen pid
    let mut all_images: Vec<(u64, Image)> = Vec::new(); // (pid, image) until the pid is known
    let mut exe_base: Option<(u64, u64)> = None;
    let mut tid_pid: HashMap<u64, u64> = HashMap::new();
    let mut pending: HashMap<u64, Pending> = HashMap::new();
    let mut threads: BTreeMap<u64, Thread> = BTreeMap::new();
    let (mut n_samples, mut n_stacks, mut n_lines) = (0u64, 0u64, 0u64);
    let (mut ft_min, mut ft_max) = (u64::MAX, 0u64);
    let mut other_pid_samples = 0u64;

    let record = |threads: &mut BTreeMap<u64, Thread>, tid: u64, p: Pending, frames: &[u64], images: &[Image], exe_base: Option<(u64, u64)>, pe: &Option<Pe>, ref_ms: i64| {
        if let Some(t) = only_tid {
            if t != tid {
                return;
            }
        }
        let ms = ft_to_unix_ms(p.ft);
        let rel = (ms - ref_ms) as f64 / 1000.0;
        if rel < from || rel > to {
            return;
        }
        let th = threads.entry(tid).or_default();
        th.samples += 1;
        *th.bins.entry((rel / bin).floor() as i64).or_default() += 1;
        let kernel = p.ip >= 0xFFFF_8000_0000_0000;
        if kernel {
            th.kernel += 1;
        }
        let module_of = |va: u64| -> String {
            if va >= 0xFFFF_8000_0000_0000 {
                return "[kernel]".into();
            }
            for im in images {
                if va >= im.base && va < im.base + im.size {
                    return im.name.clone();
                }
            }
            "?".into()
        };
        let exe_rva = |va: u64| -> Option<u64> {
            let (b, s) = exe_base?;
            (va >= b && va < b + s).then(|| va - b)
        };
        // the function a return address lives in = the target of the call
        // before the NEXT frame's return address (direct calls only)
        let fn_of_frame = |k: usize| -> Option<u64> {
            let ret_above = *frames.get(k + 1)?;
            let rva_above = exe_rva(ret_above)?;
            let pe = pe.as_ref()?;
            let before = pe.bytes((rva_above as u32).checked_sub(7)?, 7)?;
            let (_, _, rel) = decode_call(before)?;
            let target = (rva_above as i64 + rel?) as u64;
            (target < pe.size_of_image as u64).then_some(target)
        };
        // the leaf: the sampled IP is frame 0 when there is a stack
        let leaf_module = module_of(p.ip);
        *th.modules.entry(leaf_module).or_default() += 1;
        if frames.is_empty() {
            th.no_stack += 1;
            if let Some(r) = exe_rva(p.ip) {
                *th.leaf_ip.entry(r).or_default() += 1;
                *th.leaf_fn.entry((r & !0xFF, false)).or_default() += 1;
            }
            return;
        }
        // user-mode part of the stack (a kernel-mode sample carries the
        // kernel frames first)
        let user: Vec<u64> = frames.iter().copied().filter(|&f| f < 0xFFFF_8000_0000_0000).collect();
        if let Some(&leaf) = user.first() {
            if let Some(r) = exe_rva(leaf) {
                *th.leaf_ip.entry(r).or_default() += 1;
                // frame index of `leaf` inside `frames`
                let k = frames.iter().position(|&f| f == leaf).unwrap_or(0);
                match fn_of_frame(k) {
                    Some(f) => *th.leaf_fn.entry((f, true)).or_default() += 1,
                    None => *th.leaf_fn.entry((r & !0xFF, false)).or_default() += 1,
                }
            }
        }
        let mut sites: Vec<u64> = Vec::new();
        let mut fns: Vec<u64> = Vec::new();
        let mut key: Vec<u64> = Vec::new();
        let mut fold = String::new();
        for (k, &f) in frames.iter().enumerate().rev() {
            let m = module_of(f);
            match exe_rva(f) {
                Some(r) => {
                    if !sites.contains(&r) {
                        sites.push(r);
                    }
                    if let Some(s) = fn_of_frame(k) {
                        if !fns.contains(&s) {
                            fns.push(s);
                        }
                    }
                    key.push(r);
                    if !fold.is_empty() {
                        fold.push(';');
                    }
                    match fn_of_frame(k) {
                        Some(s) => {
                            let _ = write!(fold, "exe+{:x} (fn {:x})", r, s);
                        }
                        None => {
                            let _ = write!(fold, "exe+{:x}", r);
                        }
                    }
                }
                None => {
                    if !exe_only {
                        if !fold.is_empty() {
                            fold.push(';');
                        }
                        if m == "[kernel]" {
                            fold.push_str("[kernel]");
                        } else {
                            let _ = write!(fold, "{}", m);
                        }
                    }
                }
            }
        }
        for s in sites {
            *th.incl_site.entry(s).or_default() += 1;
        }
        for s in fns {
            *th.incl_fn.entry(s).or_default() += 1;
        }
        *th.stacks.entry(key).or_default() += 1;
        if folded.is_some() {
            *th.folded.entry(fold).or_default() += 1;
        }
    };

    loop {
        line.clear();
        let n = rd.read_line(&mut line).map_err(|e| format!("{csv}: {e}"))?;
        if n == 0 {
            break;
        }
        n_lines += 1;
        let l = line.trim_start();
        let Some(kind) = l.split(',').next() else { continue };
        match kind.trim() {
            "PerfInfo" => {
                let f: Vec<&str> = l.split(',').collect();
                if field(&f, 1) != "SampleProf" {
                    continue;
                }
                let Some(ft) = field(&f, 16).parse::<u64>().ok() else { continue };
                let Some(ip) = hex(field(&f, 19)) else { continue };
                let Some(tid) = field(&f, 20).parse::<u64>().ok() else { continue };
                n_samples += 1;
                ft_min = ft_min.min(ft);
                ft_max = ft_max.max(ft);
                // a sample still pending for this thread never got its stack
                if let Some(prev) = pending.insert(tid, Pending { ft, ip }) {
                    match (pid, tid_pid.get(&tid)) {
                        (Some(c), Some(&tp)) if tp == c => record(&mut threads, tid, prev, &[], &images, exe_base, &pe, t0.unwrap_or_else(|| ft_to_unix_ms(ft_min))),
                        _ => other_pid_samples += 1,
                    }
                }
            }
            "StackWalk" => {
                let f: Vec<&str> = l.split(',').collect();
                if field(&f, 1) != "Stack" {
                    continue;
                }
                let Some(sp) = hex(field(&f, 20)) else { continue };
                let Some(tid) = field(&f, 21).parse::<u64>().ok() else { continue };
                tid_pid.insert(tid, sp);
                n_stacks += 1;
                let Some(p) = pending.remove(&tid) else { continue };
                if Some(sp) != pid {
                    other_pid_samples += 1;
                    continue;
                }
                let frames: Vec<u64> = f.get(22..).unwrap_or(&[]).iter().filter_map(|s| hex(s)).collect();
                record(&mut threads, tid, p, &frames, &images, exe_base, &pe, t0.unwrap_or_else(|| ft_to_unix_ms(ft_min)));
            }
            "Image" => {
                let ty = {
                    let f: Vec<&str> = l.splitn(3, ',').collect();
                    field(&f, 1).to_string()
                };
                if ty != "DCStart" && ty != "Load" && ty != "DCEnd" {
                    continue;
                }
                let f: Vec<&str> = l.split(',').collect();
                let (Some(base), Some(size), Some(ipid)) = (hex(field(&f, 19)), hex(field(&f, 20)), field(&f, 21).parse::<u64>().ok()) else { continue };
                let name = f.last().map(|s| s.trim().trim_matches('"')).unwrap_or("").to_string();
                let short = name.rsplit(['\\', '/']).next().unwrap_or(&name).to_string();
                let is_exe = short.to_lowercase().ends_with("trackmania.exe");
                let im = Image { base, size, name: short };
                if pid.is_none() && is_exe {
                    // the process is known from here on: adopt what was seen before
                    pid = Some(ipid);
                    for (p, im) in all_images.drain(..) {
                        if p == ipid && !images.iter().any(|x| x.base == im.base) {
                            images.push(im);
                        }
                    }
                }
                if Some(ipid) == pid {
                    if is_exe {
                        exe_base = Some((base, size));
                    }
                    if !images.iter().any(|x| x.base == base) {
                        images.push(im);
                    }
                } else if pid.is_none() {
                    all_images.push((ipid, im));
                }
            }
            _ => {}
        }
    }
    // flush the samples still waiting for a stack
    let chosen = pid;
    for (tid, p) in pending.drain() {
        match (chosen, tid_pid.get(&tid)) {
            (Some(c), Some(&tp)) if tp == c => record(&mut threads, tid, p, &[], &images, exe_base, &pe, t0.unwrap_or_else(|| ft_to_unix_ms(ft_min))),
            _ => other_pid_samples += 1,
        }
    }

    // ---- report
    let mut out = String::new();
    let span = if ft_max > ft_min { (ft_max - ft_min) as f64 / 1e7 } else { 0.0 };
    let _ = writeln!(out, "trace    {csv}: {n_lines} rows, {n_samples} samples, {n_stacks} stacks, {span:.1} s of samples");
    if let Some(z) = t0 {
        let _ = writeln!(out, "clock    T0 unix_ms {z}; samples from T0{:+.1}s to T0{:+.1}s", (ft_to_unix_ms(ft_min) - z) as f64 / 1000.0, (ft_to_unix_ms(ft_max) - z) as f64 / 1000.0);
    }
    match chosen {
        Some(p) => {
            let _ = writeln!(out, "process  pid {p}: {} images; exe {}", images.len(), match exe_base {
                Some((b, s)) => format!("at 0x{b:x} size 0x{s:x}"),
                None => "NOT SEEN in the image rundown -- exe frames cannot be attributed".into(),
            });
        }
        None => {
            let _ = writeln!(out, "process  no Trackmania.exe image in the trace and no --pid: nothing attributed");
        }
    }
    if let (Some(pe), Some((_, s))) = (&pe, exe_base) {
        if pe.size_of_image as u64 != s {
            let _ = writeln!(out, "warning  exe on disk has SizeOfImage 0x{:x}, the trace 0x{s:x} -- different build?", pe.size_of_image);
        }
    }
    let total: u64 = threads.values().map(|t| t.samples).sum();
    let _ = writeln!(out, "samples  {total} in the process ({other_pid_samples} belonged to other processes){}", if from.is_finite() || to.is_finite() { format!(", window {from}..{to} s") } else { String::new() });
    let mut by: Vec<(&u64, &Thread)> = threads.iter().collect();
    by.sort_by(|a, b| b.1.samples.cmp(&a.1.samples));
    let _ = writeln!(out, "\n=== threads (1 sample = 1 ms of CPU; {:.0}% = the whole wall time of the window when the window is {span:.0} s) ===", 100.0);
    let _ = writeln!(out, "{:>8} {:>9} {:>7} {:>7}  {}", "tid", "samples", "cpu-s", "kernel", "leaf modules");
    for (tid, t) in by.iter().take(top) {
        let mut mods: Vec<(&String, &u64)> = t.modules.iter().collect();
        mods.sort_by(|a, b| b.1.cmp(a.1));
        let ms: Vec<String> = mods.iter().take(5).map(|(m, n)| format!("{m} {:.0}%", **n as f64 * 100.0 / t.samples as f64)).collect();
        let _ = writeln!(out, "{:>8} {:>9} {:>7.1} {:>6.0}%  {}", tid, t.samples, t.samples as f64 / 1000.0, t.kernel as f64 * 100.0 / t.samples.max(1) as f64, ms.join(", "));
    }
    // per-bin table for the top threads
    let cols: Vec<u64> = by.iter().take(8).map(|(tid, _)| **tid).collect();
    let mut bins: Vec<i64> = threads.values().flat_map(|t| t.bins.keys().copied()).collect();
    bins.sort_unstable();
    bins.dedup();
    if !bins.is_empty() {
        let _ = writeln!(out, "\n=== CPU per {bin:.0} s bin (samples/1000 = CPU seconds; 1 thread flat out = {bin:.0}) ===");
        let _ = write!(out, "{:>9} ", "t (s)");
        for c in &cols {
            let _ = write!(out, "{:>8}", c);
        }
        let _ = writeln!(out, "{:>8}", "others");
        for b in bins {
            let _ = write!(out, "{:>9.0} ", b as f64 * bin);
            let mut acc = 0u64;
            for c in &cols {
                let n = threads[c].bins.get(&b).copied().unwrap_or(0);
                acc += n;
                let _ = write!(out, "{:>8.2}", n as f64 / 1000.0);
            }
            let all: u64 = threads.values().map(|t| t.bins.get(&b).copied().unwrap_or(0)).sum();
            let _ = writeln!(out, "{:>8.2}", (all - acc) as f64 / 1000.0);
        }
    }
    let objd = |rva: u64| format!("0x{:x}", 0x1_4000_0000u64 + rva);
    for (tid, t) in by.iter().take(3) {
        if t.samples < 200 {
            break;
        }
        let pct = |n: u64| n as f64 * 100.0 / t.samples as f64;
        let _ = writeln!(out, "\n=== thread {tid}: {} samples ({} without a stack) ===", t.samples, t.no_stack);
        let mut lf: Vec<(&(u64, bool), &u64)> = t.leaf_fn.iter().collect();
        lf.sort_by(|a, b| b.1.cmp(a.1));
        let _ = writeln!(out, "--- leaf functions in the exe (self time; `~` = 256 B page, caller unknown) ---");
        for ((a, is_fn), n) in lf.iter().take(top) {
            let (a, is_fn) = (*a, *is_fn);
            let hot: String = {
                let mut ips: Vec<(&u64, &u64)> = t.leaf_ip.iter().filter(|(ip, _)| if is_fn { **ip >= a && **ip < a + 0x4000 } else { **ip & !0xFF == a }).collect();
                ips.sort_by(|x, y| y.1.cmp(x.1));
                ips.iter().take(3).map(|(ip, n)| format!("+{:x}×{}", **ip - a, n)).collect::<Vec<_>>().join(" ")
            };
            let _ = writeln!(out, "{:>6.1}% {:>7}  {}Trackmania.exe+0x{:<8x} objdump {}  hot {hot}", pct(**n), n, if is_fn { " " } else { "~" }, a, objd(a));
        }
        let mut inc: Vec<(&u64, &u64)> = t.incl_fn.iter().collect();
        inc.sort_by(|a, b| b.1.cmp(a.1));
        let _ = writeln!(out, "--- inclusive by function (a sample counts once per function on its stack) ---");
        for (a, n) in inc.iter().take(top) {
            let _ = writeln!(out, "{:>6.1}% {:>7}  Trackmania.exe+0x{:<8x} objdump {}", pct(**n), n, a, objd(**a));
        }
        let mut st: Vec<(&Vec<u64>, &u64)> = t.stacks.iter().collect();
        st.sort_by(|a, b| b.1.cmp(a.1));
        let _ = writeln!(out, "--- most common exe stacks (leaf first) ---");
        for (k, n) in st.iter().take(8) {
            let _ = writeln!(out, "{:>6.1}% {:>7}", pct(**n), n);
            for r in k.iter().take(24) {
                let _ = writeln!(out, "           exe+0x{:<8x} objdump {}", r, objd(*r));
            }
        }
    }
    if let Some(fp) = folded {
        let mut text = String::new();
        for (tid, t) in &threads {
            for (k, n) in &t.folded {
                let _ = writeln!(text, "tid{tid};{k} {n}");
            }
        }
        std::fs::write(&fp, text).map_err(|e| format!("{fp}: {e}"))?;
        let _ = writeln!(out, "\nfolded stacks written to {fp}");
    }
    print!("{out}");
    Ok(())
}

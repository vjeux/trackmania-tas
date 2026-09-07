//! `mapgeom threads` -- the poor man's sampler: every thread's stack in every
//! minidump `shootctl loadprof` took during a load, and what repeats.
//!
//! ```text
//! mapgeom threads DUMP... --exe Trackmania.exe [--tid T] [--all] [--depth N] [--quiet]
//! ```
//!
//! A `mini` dump (`rundll32 comsvcs.dll, MiniDump PID FILE mini`) carries
//! every thread's CONTEXT and stack memory, so the heuristic walk of
//! `mapgeom crash` (return addresses preceded by a call, chained through the
//! callees) runs for ANY thread, not just a faulting one. Per dump this
//! prints the threads that are doing something -- rip outside ntdll's wait
//! syscalls -- with their chain; across dumps it counts, per thread, the
//! functions at the leaf and on the chain: a load that spends five minutes
//! in one place shows that place in every dump.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::minidump::{Link, Session};

const USAGE: &str = "mapgeom threads DUMP... --exe Trackmania.exe [--tid T] [--all] [--depth N] [--quiet]";

pub fn run(args: &[String]) -> Result<(), String> {
    // args[0] == "threads"
    let mut dumps: Vec<String> = Vec::new();
    let mut exe: Option<String> = None;
    let mut only_tid: Option<u32> = None;
    let mut all = false;
    let mut quiet = false;
    let mut depth = 12usize;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--exe" => {
                i += 1;
                exe = Some(args.get(i).cloned().ok_or("--exe needs a path")?);
            }
            "--tid" => {
                i += 1;
                only_tid = Some(args.get(i).and_then(|s| s.parse().ok()).ok_or("--tid T")?);
            }
            "--depth" => {
                i += 1;
                depth = args.get(i).and_then(|s| s.parse().ok()).ok_or("--depth N")?;
            }
            "--all" => all = true,
            "--quiet" => quiet = true,
            a if a.starts_with("--") => return Err(format!("unknown option {a}\n{USAGE}")),
            a => dumps.push(a.to_string()),
        }
        i += 1;
    }
    if dumps.is_empty() {
        return Err(USAGE.to_string());
    }
    dumps.sort();
    // per thread: leaf function -> count, chain function -> count, dumps seen busy
    let mut leaf: BTreeMap<u32, HashMap<String, usize>> = BTreeMap::new();
    let mut chain: BTreeMap<u32, HashMap<String, usize>> = BTreeMap::new();
    let mut busy: BTreeMap<u32, usize> = BTreeMap::new();
    let mut out = String::new();
    for dp in &dumps {
        let s = match Session::open(dp, exe.as_deref()) {
            Ok(s) => s,
            Err(e) => {
                let _ = writeln!(out, "{dp}: {e}");
                continue;
            }
        };
        let _ = writeln!(out, "=== {dp}: {} threads ===", s.dump.threads.len());
        for t in &s.dump.threads {
            if let Some(w) = only_tid {
                if w != t.tid {
                    continue;
                }
            }
            let Some(ctx) = s.thread_context(t.tid) else { continue };
            let rip = ctx.rip();
            let leaf_mod = s.dump.module_of(rip).map(|m| m.short().to_string()).unwrap_or_else(|| "?".into());
            // a thread parked in ntdll (NtWaitFor*, NtDelayExecution, the
            // thread pool) or win32u (GetMessage) is not doing anything
            let waiting = matches!(leaf_mod.to_ascii_lowercase().as_str(), "ntdll.dll" | "win32u.dll");
            if waiting && !all {
                continue;
            }
            *busy.entry(t.tid).or_default() += 1;
            let frames = s.stack_walk_at(rip, ctx.rsp(), Some(t.tid));
            let _ = writeln!(out, "-- tid {} rip {}", t.tid, s.fmt_exe(rip));
            *leaf.entry(t.tid).or_default().entry(s.fmt_exe(rip)).or_default() += 1;
            let mut n = 0;
            for f in &frames {
                if matches!(f.link, Link::Stale | Link::Possible | Link::Rip) {
                    continue;
                }
                let tag = match f.link {
                    Link::Verified => " ",
                    Link::Indirect => "v",
                    _ => "?",
                };
                let func = f.func.map(|x| format!("  in fn {}", s.fmt_exe(x))).unwrap_or_default();
                if !quiet {
                    let _ = writeln!(out, "   #{:<2}{tag} {}{func}", f.depth, s.fmt_exe(f.ret));
                }
                if let Some(x) = f.func {
                    *chain.entry(t.tid).or_default().entry(s.fmt_exe(x)).or_default() += 1;
                }
                n += 1;
                if n >= depth {
                    break;
                }
            }
            if let Some(f) = frames.first().and_then(|_| frames.iter().find(|f| f.depth == 1)) {
                // the leaf's own function = the callee of the first chain frame
                if let Some(x) = f.callee {
                    *leaf.entry(t.tid).or_default().entry(format!("fn {}", s.fmt_exe(x))).or_default() += 1;
                }
            }
        }
    }
    let _ = writeln!(out, "\n=== across {} dumps: threads seen busy (rip outside ntdll/win32u) ===", dumps.len());
    let mut bz: Vec<(&u32, &usize)> = busy.iter().collect();
    bz.sort_by(|a, b| b.1.cmp(a.1));
    for (tid, n) in bz.iter().take(12) {
        let _ = writeln!(out, "tid {tid}: busy in {n}/{} dumps", dumps.len());
        if let Some(h) = leaf.get(tid) {
            let mut v: Vec<(&String, &usize)> = h.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            for (k, c) in v.iter().take(6) {
                let _ = writeln!(out, "    leaf {c:>3}  {k}");
            }
        }
        if let Some(h) = chain.get(tid) {
            let mut v: Vec<(&String, &usize)> = h.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            for (k, c) in v.iter().take(12) {
                let _ = writeln!(out, "    chain {c:>3}  {k}");
            }
        }
    }
    print!("{out}");
    Ok(())
}

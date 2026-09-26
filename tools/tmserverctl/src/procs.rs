//! Pid files and signals.

use std::fs;
use std::path::Path;

pub fn read_pid(path: &Path) -> Option<i32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

pub fn alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // kill(pid, 0): no signal, just the existence/permission check.
    unsafe { libc::kill(pid, 0) == 0 }
}

pub fn terminate(pid: i32) {
    if pid > 0 {
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
}

pub fn kill9(pid: i32) {
    if pid > 0 {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
}

/// Seconds since the process started, from /proc (Linux only; None elsewhere).
pub fn uptime_secs(pid: i32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Field 22 (1-based) is starttime in clock ticks; the comm field may contain spaces, so
    // split after the closing parenthesis.
    let after = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = after.split_whitespace().collect();
    let start_ticks: u64 = fields.get(19)?.parse().ok()?;
    let uptime = fs::read_to_string("/proc/uptime").ok()?;
    let up: f64 = uptime.split_whitespace().next()?.parse().ok()?;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    if hz <= 0.0 {
        return None;
    }
    Some((up - start_ticks as f64 / hz).max(0.0) as u64)
}

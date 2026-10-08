//! inputidle — is a person using this Windows machine right now?
//!
//! vjeux 2026-10-04: "implement a check using the machine: needs to have keyboard /
//! mouse / controller in the past minute" — the box is FREE when none of the three saw
//! input for `--window-s` (60) seconds; before that rule every agent launch waited for a
//! word, while the machine sat idle most of the day (and twice a launch landed on a
//! game that a process-list count had not seen).
//!
//! ```text
//! inputidle.exe status [--window-s 60] [--confirm-s 5] [--pad-sample-ms 1500] [--state FILE]
//!     one line: verdict=free|busy kbm_idle_s=N pad_idle_s=N|unknown pads=K fg=<exe> "<title>"
//!     exit 0 free, 1 busy, 2 error
//! inputidle.exe watch --state FILE [--poll-ms 250]
//!     the controller sampler: polls the four XInput slots; on a packet change writes
//!     `last=<unix>` to FILE, a `beat=<unix>` heartbeat every 5 s (atomic replace)
//! inputidle.exe install --state FILE
//!     registers `watch` in HKCU\...\Run (starts with the user's session) and starts it now
//! ```
//!
//! Keyboard and mouse: `GetLastInputInfo` — the OS's own last-input tick for the
//! interactive session (covers a remote-play session too: the input arrives as input).
//! Controllers: XInput has no "last input time", so `watch` samples the packet number
//! (it changes on any state change) and keeps the time of the last change in a file;
//! `status` reads it (and runs a short live sample of its own, so a controller in use
//! RIGHT NOW is seen even with no watcher). A controller the game reads through another
//! API (DirectInput-only pads) is not seen — Steam's virtual Xbox pad is.
//! Non-Windows builds are a stub (the workspace builds on Linux).

#![allow(non_snake_case)]
// the msvc cross-link brings its own mainCRTStartup (entry.rs) which calls
// `idle_main`; a plain `fn main` would collide with rustc's entry shim
#![cfg_attr(target_env = "msvc", no_main)]

#[cfg(target_env = "msvc")]
mod entry;
#[cfg(target_env = "msvc")]
mod shim;

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy, PartialEq)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }
    #[repr(C)]
    pub struct LastInputInfo {
        pub cb_size: u32,
        pub dw_time: u32,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct XInputGamepad {
        pub buttons: u16,
        pub left_trigger: u8,
        pub right_trigger: u8,
        pub lx: i16,
        pub ly: i16,
        pub rx: i16,
        pub ry: i16,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct XInputState {
        pub packet: u32,
        pub gamepad: XInputGamepad,
    }

    // raw-dylib: rustc writes the import entries itself, so the cross-link needs
    // no user32.lib (no Windows SDK on this box — the credread recipe)
    #[link(name = "user32", kind = "raw-dylib")]
    extern "system" {
        pub fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
        pub fn GetForegroundWindow() -> *mut c_void;
        pub fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        pub fn GetWindowTextW(hwnd: *mut c_void, buf: *mut u16, n: i32) -> i32;
        pub fn GetCursorPos(p: *mut Point) -> i32;
        pub fn SendInput(n: u32, inputs: *const Input, size: i32) -> u32;
    }
    /// `INPUT` with a `KEYBDINPUT` body (type 1): the union is as wide as its
    /// widest member (MOUSEINPUT, 24 bytes + padding → 32 bytes on x64).
    #[repr(C)]
    pub struct Input {
        pub ty: u32,
        pub _pad: u32,
        pub vk: u16,
        pub scan: u16,
        pub flags: u32,
        pub time: u32,
        pub extra: usize,
        pub _tail: [u8; 8],
    }
    pub const INPUT_KEYBOARD: u32 = 1;
    pub const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
    pub const KEYEVENTF_KEYUP: u32 = 0x0002;
    pub const KEYEVENTF_SCANCODE: u32 = 0x0008;
    #[link(name = "kernel32", kind = "raw-dylib")]
    extern "system" {
        pub fn GetTickCount() -> u32;
        pub fn LoadLibraryA(name: *const u8) -> *mut c_void;
        pub fn GetProcAddress(h: *mut c_void, name: *const u8) -> *mut c_void;
        pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        pub fn CloseHandle(h: *mut c_void) -> i32;
        pub fn QueryFullProcessImageNameW(h: *mut c_void, flags: u32, buf: *mut u16, size: *mut u32) -> i32;
        pub fn GetCommandLineW() -> *const u16;
        pub fn Sleep(ms: u32);
        pub fn GetSystemTimeAsFileTime(ft: *mut u64);
        pub fn CreateProcessW(app: *const u16, cmd: *mut u16, pa: *mut c_void, ta: *mut c_void, inherit: i32, flags: u32, env: *mut c_void, cwd: *const u16, si: *mut StartupInfoW, pi: *mut ProcessInformation) -> i32;
    }
    #[link(name = "shell32", kind = "raw-dylib")]
    extern "system" {
        pub fn CommandLineToArgvW(cmd: *const u16, argc: *mut i32) -> *mut *mut u16;
    }

    #[repr(C)]
    #[derive(Default)]
    pub struct StartupInfoW {
        pub cb: u32,
        pub reserved: *mut u16,
        pub desktop: *mut u16,
        pub title: *mut u16,
        pub x: u32,
        pub y: u32,
        pub xsize: u32,
        pub ysize: u32,
        pub xcount: u32,
        pub ycount: u32,
        pub fill: u32,
        pub flags: u32,
        pub show: u16,
        pub cb_reserved2: u16,
        pub reserved2: *mut u8,
        pub stdin_h: *mut c_void,
        pub stdout_h: *mut c_void,
        pub stderr_h: *mut c_void,
    }
    #[repr(C)]
    #[derive(Default)]
    pub struct ProcessInformation {
        pub process: *mut c_void,
        pub thread: *mut c_void,
        pub pid: u32,
        pub tid: u32,
    }

    /// the command line, split the way the OS does (no std runtime init here)
    pub fn args() -> Vec<String> {
        let mut argc = 0i32;
        let argv = unsafe { CommandLineToArgvW(GetCommandLineW(), &mut argc) };
        let mut out = Vec::new();
        if !argv.is_null() {
            for i in 0..argc as isize {
                let w = unsafe { *argv.offset(i) };
                let mut n = 0isize;
                while unsafe { *w.offset(n) } != 0 {
                    n += 1;
                }
                out.push(String::from_utf16_lossy(unsafe { core::slice::from_raw_parts(w, n as usize) }));
            }
        }
        out
    }

    /// unix seconds from the system clock (std::time needs runtime init we do not run)
    pub fn now_unix() -> u64 {
        let mut ft: u64 = 0;
        unsafe { GetSystemTimeAsFileTime(&mut ft) };
        // 100 ns ticks since 1601-01-01 → unix seconds
        ft / 10_000_000 - 11_644_473_600
    }

    pub fn sleep_ms(ms: u32) {
        unsafe { Sleep(ms) }
    }

    /// launch a detached copy of ourselves (DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
    pub fn spawn_detached(cmdline: &str) -> Result<u32, String> {
        let mut cmd: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();
        let mut si = StartupInfoW { cb: std::mem::size_of::<StartupInfoW>() as u32, ..Default::default() };
        let mut pi = ProcessInformation::default();
        let ok = unsafe {
            CreateProcessW(std::ptr::null(), cmd.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), 0, 0x0000_0008 | 0x0000_0200, std::ptr::null_mut(), std::ptr::null(), &mut si, &mut pi)
        };
        if ok == 0 {
            return Err("CreateProcessW failed".into());
        }
        unsafe {
            CloseHandle(pi.thread);
            CloseHandle(pi.process);
        }
        Ok(pi.pid)
    }

    pub type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

    /// seconds since the session's last keyboard / mouse input (the OS tick counter wraps
    /// every 49.7 days; the subtraction wraps with it)
    pub fn kbm_idle_s() -> Result<f64, String> {
        let mut lii = LastInputInfo { cb_size: std::mem::size_of::<LastInputInfo>() as u32, dw_time: 0 };
        let ok = unsafe { GetLastInputInfo(&mut lii) };
        if ok == 0 {
            return Err("GetLastInputInfo failed".into());
        }
        let now = unsafe { GetTickCount() };
        Ok(now.wrapping_sub(lii.dw_time) as f64 / 1000.0)
    }

    pub fn xinput() -> Option<XInputGetStateFn> {
        for dll in ["xinput1_4.dll\0", "xinput1_3.dll\0", "xinput9_1_0.dll\0"] {
            let h = unsafe { LoadLibraryA(dll.as_ptr()) };
            if h.is_null() {
                continue;
            }
            let p = unsafe { GetProcAddress(h, "XInputGetState\0".as_ptr()) };
            if !p.is_null() {
                return Some(unsafe { std::mem::transmute::<*mut c_void, XInputGetStateFn>(p) });
            }
        }
        None
    }

    /// (connected pads, their packet numbers by slot)
    pub fn pads(get: XInputGetStateFn) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        for slot in 0..4u32 {
            let mut st = XInputState::default();
            if unsafe { get(slot, &mut st) } == 0 {
                out.push((slot, st.packet));
            }
        }
        out
    }

    pub fn cursor() -> Point {
        let mut p = Point::default();
        unsafe { GetCursorPos(&mut p) };
        p
    }

    /// the foreground window's process image name and title
    pub fn foreground() -> (String, String) {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return ("-".into(), String::new());
        }
        let mut title = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), 256) };
        let title = String::from_utf16_lossy(&title[..n.max(0) as usize]);
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let mut exe = String::from("-");
        let h = unsafe { OpenProcess(0x1000, 0, pid) }; // PROCESS_QUERY_LIMITED_INFORMATION
        if !h.is_null() {
            let mut buf = [0u16; 1024];
            let mut len = 1024u32;
            if unsafe { QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) } != 0 {
                let full = String::from_utf16_lossy(&buf[..len as usize]);
                exe = full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string();
            }
            unsafe { CloseHandle(h) };
        }
        (exe, title)
    }
}

#[cfg(windows)]
fn now_unix() -> u64 {
    win::now_unix()
}
#[cfg(not(windows))]
fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn flag(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

/// the sampler file: `last=<unix>` (last controller change) and `beat=<unix>` (heartbeat)
fn read_state(path: &str) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut last = None;
    let mut beat = None;
    for l in text.lines() {
        if let Some(v) = l.strip_prefix("last=") {
            last = v.trim().parse().ok();
        }
        if let Some(v) = l.strip_prefix("beat=") {
            beat = v.trim().parse().ok();
        }
    }
    Some((last.unwrap_or(0), beat.unwrap_or(0)))
}

fn write_state(path: &str, last: u64, beat: u64) {
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, format!("last={last}\nbeat={beat}\n")).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

#[cfg(all(windows, not(target_env = "msvc")))]
fn main() {
    std::process::exit(run(&std::env::args().collect::<Vec<_>>()));
}

/// the cross-link's entry (entry.rs `mainCRTStartup` calls this); no std runtime
/// init runs in that shape, so argv comes from the OS, not `std::env::args`
#[cfg(target_env = "msvc")]
#[no_mangle]
pub extern "Rust" fn idle_main() {
    let args = win::args();
    let code = run(&args);
    unsafe { shim::exit_process(code as u32) }
}

#[cfg(windows)]
fn run(args: &[String]) -> i32 {
    let cmd = args.get(1).map(String::as_str).unwrap_or("status");
    match cmd {
        "status" => status(args),
        "watch" => {
            watch(args);
            0
        }
        "probe" => probe(args),
        "hold" => {
            hold(args);
            0
        }
        "install" => install(args),
        _ => {
            println!("inputidle: hold [--scan 0x48] [--ext] [--ms 20000] | status [--window-s 60] [--state FILE] | watch --state FILE | install --state FILE");
            2
        }
    }
}

#[cfg(windows)]
fn status(args: &[String]) -> i32 {
    let window: f64 = flag(args, "--window-s").and_then(|v| v.parse().ok()).unwrap_or(60.0);
    let sample_ms: u64 = flag(args, "--pad-sample-ms").and_then(|v| v.parse().ok()).unwrap_or(1500);
    // The confirmation sample (vjeux 2026-10-05: "the check is to check if you can use the
    // machine"): a GAME left in the foreground keeps the OS input clock at 0 for ever
    // (Minecraft's launcher did for 9 hours), so a fresh clock alone does not mean a person
    // is there. When the clock looks fresh we watch it for a few seconds: a person moves the
    // pointer, switches window, or produces DISCRETE events (the idle counter climbs between
    // them); an app pumping the clock shows a flat 0 with nothing else moving.
    let confirm_s: u64 = flag(args, "--confirm-s").and_then(|v| v.parse().ok()).unwrap_or(5);
    let state = flag(args, "--state");
    let kbm = match win::kbm_idle_s() {
        Ok(v) => v,
        Err(e) => {
            println!("verdict=error {e}");
            return 2;
        }
    };
    // controllers: the sampler's record (if fresh) + a live sample of our own
    let mut pad_idle: Option<f64> = None;
    let mut pad_note = String::new();
    let now = now_unix();
    if let Some(p) = &state {
        match read_state(p) {
            Some((last, beat)) if now.saturating_sub(beat) <= 30 => {
                pad_idle = Some(if last == 0 { f64::INFINITY } else { now.saturating_sub(last) as f64 });
            }
            Some((_, beat)) => pad_note = format!(" watcher=stale({}s)", now.saturating_sub(beat)),
            None => pad_note = " watcher=absent".into(),
        }
    }
    let mut n_pads = 0usize;
    let mut live_change = false;
    if let Some(get) = win::xinput() {
        let before = win::pads(get);
        n_pads = before.len();
        if n_pads > 0 {
            let mut waited = 0u64;
            while waited < sample_ms {
                win::sleep_ms(50);
                waited += 50;
                let after = win::pads(get);
                if after != before {
                    live_change = true;
                    break;
                }
            }
        }
    } else {
        pad_note.push_str(" xinput=unavailable");
    }
    if live_change {
        pad_idle = Some(0.0);
    }
    let (exe, title) = win::foreground();
    // the confirmation: only needed when the clock says "input just now"
    let mut pumped = false;
    let mut why = String::new();
    if kbm < window && confirm_s > 0 {
        let c0 = win::cursor();
        let (fg0, _) = win::foreground();
        let mut moved = false;
        let mut fg_changed = false;
        let mut idle_max: f64 = 0.0;
        let mut t = 0u64;
        while t < confirm_s * 1000 {
            win::sleep_ms(250);
            t += 250;
            if win::cursor() != c0 {
                moved = true;
                break;
            }
            if win::foreground().0 != fg0 {
                fg_changed = true;
                break;
            }
            if let Ok(v) = win::kbm_idle_s() {
                idle_max = idle_max.max(v);
            }
        }
        // a person's input is discrete: the counter climbs between keystrokes. A flat 0 with
        // no pointer movement and no window change is the foreground app pumping it.
        pumped = !moved && !fg_changed && idle_max < 1.5;
        why = format!(" pointer_moved={moved} window_changed={fg_changed} idle_max_s={idle_max:.1}");
    }
    let busy = (kbm < window && !pumped) || pad_idle.map(|v| v < window).unwrap_or(false);
    let pad_s = match pad_idle {
        Some(v) if v.is_infinite() => "none".to_string(),
        Some(v) => format!("{v:.0}"),
        None => "unknown".to_string(),
    };
    println!(
        "verdict={} kbm_idle_s={kbm:.0}{} pad_idle_s={pad_s} pads={n_pads}{pad_note}{why} window_s={window:.0} fg={exe} \"{}\"",
        if busy { "busy" } else { "free" },
        if pumped { " clock=app-pumped" } else { "" },
        title.replace('"', "'")
    );
    if busy {
        1
    } else {
        0
    }
}

/// `probe --seconds N`: watch the three signals for N seconds and say whether a PERSON is
/// there. A game in the foreground keeps the OS input timer fresh by itself (Minecraft
/// recentres the mouse every frame), so "idle 0 s" alone cannot tell an active player from
/// an open game nobody is at: a person also MOVES the pointer or changes window now and
/// then. Reports `person=yes` (the pointer moved or the window changed), `person=app`
/// (input resets but nothing else ever moves — an app pumping the timer), `person=no`
/// (no input at all). The box is still BUSY for us whenever a game is in the foreground;
/// this line is the evidence for asking its owner (2026-10-05, a 3 h Minecraft window).
#[cfg(windows)]
fn probe(args: &[String]) -> i32 {
    let seconds: u64 = flag(args, "--seconds").and_then(|v| v.parse().ok()).unwrap_or(60);
    let (fg0, _) = win::foreground();
    let c0 = win::cursor();
    let mut moved = false;
    let mut fg_changed = false;
    let mut min_idle = f64::INFINITY;
    let mut max_idle: f64 = 0.0;
    let mut t = 0u64;
    while t < seconds * 1000 {
        win::sleep_ms(500);
        t += 500;
        if let Ok(v) = win::kbm_idle_s() {
            min_idle = min_idle.min(v);
            max_idle = max_idle.max(v);
        }
        if win::cursor() != c0 {
            moved = true;
        }
        let (fg, _) = win::foreground();
        if fg != fg0 {
            fg_changed = true;
        }
    }
    let person = if moved || fg_changed {
        "yes"
    } else if min_idle < 2.0 {
        "app"
    } else {
        "no"
    };
    println!("person={person} pointer_moved={moved} window_changed={fg_changed} idle_min_s={min_idle:.0} idle_max_s={max_idle:.0} seconds={seconds} fg={fg0}");
    i32::from(person != "no")
}

#[cfg(windows)]
fn watch(args: &[String]) {
    let Some(path) = flag(args, "--state") else {
        println!("watch needs --state FILE");
        return;
    };
    let poll_ms: u64 = flag(args, "--poll-ms").and_then(|v| v.parse().ok()).unwrap_or(250);
    let Some(get) = win::xinput() else {
        println!("inputidle watch: no XInput DLL");
        return;
    };
    let (mut last, _) = read_state(&path).unwrap_or((0, 0));
    let mut prev = win::pads(get);
    let mut last_beat = 0u64;
    write_state(&path, last, now_unix());
    loop {
        win::sleep_ms(poll_ms as u32);
        let cur = win::pads(get);
        let now = now_unix();
        // a packet change on a pad connected before and after = input; a pad appearing or
        // disappearing counts too (someone plugged it in / turned it on)
        if cur != prev {
            last = now;
            prev = cur;
            write_state(&path, last, now);
            last_beat = now;
        } else if now.saturating_sub(last_beat) >= 5 {
            write_state(&path, last, now);
            last_beat = now;
        }
    }
}

#[cfg(windows)]
fn install(args: &[String]) -> i32 {
    let Some(path) = flag(args, "--state") else {
        println!("install needs --state FILE");
        return 2;
    };
    let exe = flag(args, "--exe").unwrap_or_else(|| win::args().first().cloned().unwrap_or_else(|| "inputidle.exe".into()));
    let cmdline = format!("\"{exe}\" watch --state \"{path}\"");
    match win::spawn_detached(&cmdline) {
        Ok(pid) => {
            println!("watcher started (pid {pid}) — {cmdline}");
            println!("autostart: add it to HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run (value InputIdleWatch) if it should survive a reboot");
            0
        }
        Err(e) => {
            println!("inputidle install: {e}");
            2
        }
    }
}

#[cfg(not(windows))]
fn main() {
    let _ = (now_unix(), flag(&[], ""), read_state(""), write_state as fn(&str, u64, u64));
    eprintln!("inputidle is a Windows program (build it with --target x86_64-pc-windows-msvc on the box)");
    std::process::exit(2);
}


/// `hold --scan 0x48 [--ext] --ms 20000 [--delay-ms 0]`: press ONE key by scan code
/// through `SendInput`, keep it down for `--ms`, release it. Up arrow = scan 0x48
/// extended; Down 0x50, Left 0x4b, Right 0x4d (all extended); left Shift 0x2a.
/// The key goes to whatever window is in the foreground — the caller puts the
/// game there. Prints `held scan=0x48 ms=20000 inserted=2` (2 = down + up landed).
/// (2026-10-06: the physics measurement for Everios96's water blocks needed a
/// MOVING car; the MK64 kart driver refused its setup guard on this box.)
fn hold(args: &[String]) {
    let scan = flag(args, "--scan").and_then(|v| parse_u32(&v)).unwrap_or(0x48);
    let ms = flag(args, "--ms").and_then(|v| parse_u32(&v)).unwrap_or(20000);
    let delay = flag(args, "--delay-ms").and_then(|v| parse_u32(&v)).unwrap_or(0);
    let ext = args.iter().any(|a| a == "--ext") || matches!(scan, 0x48 | 0x50 | 0x4b | 0x4d);
    if delay > 0 {
        win::sleep_ms(delay);
    }
    let mut flags = win::KEYEVENTF_SCANCODE;
    if ext {
        flags |= win::KEYEVENTF_EXTENDEDKEY;
    }
    let down = win::Input { ty: win::INPUT_KEYBOARD, _pad: 0, vk: 0, scan: scan as u16, flags, time: 0, extra: 0, _tail: [0; 8] };
    let up = win::Input { ty: win::INPUT_KEYBOARD, _pad: 0, vk: 0, scan: scan as u16, flags: flags | win::KEYEVENTF_KEYUP, time: 0, extra: 0, _tail: [0; 8] };
    let size = std::mem::size_of::<win::Input>() as i32;
    let mut inserted = 0u32;
    unsafe {
        inserted += win::SendInput(1, &down, size);
    }
    win::sleep_ms(ms);
    unsafe {
        inserted += win::SendInput(1, &up, size);
    }
    println!("held scan=0x{scan:x} ext={ext} ms={ms} inserted={inserted}");
    if inserted != 2 {
        unsafe { shim::exit_process(11) }
    }
}


fn parse_u32(v: &str) -> Option<u32> {
    let v = v.trim();
    if let Some(h) = v.strip_prefix("0x") {
        u32::from_str_radix(h, 16).ok()
    } else {
        v.parse().ok()
    }
}

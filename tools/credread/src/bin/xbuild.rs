//! xbuild — cross-link credread.exe + payload.dll for Windows
//! (x86_64-pc-windows-msvc) on a Linux host with the msvc std + rust-lld,
//! no Windows SDK, no mingw.
//!
//!   cargo run -p credread --bin xbuild [-- dll:Sym,Sym ...]
//!
//! Output: `<crate>/target/xbuild/{credread.exe,payload.dll,winlib/*.lib}`.
//! Import libs are generated from symbol names (mkimplib); the fixpoint loop
//! re-runs the link, parses `undefined symbol` errors, and pulls known
//! symbols into the libs automatically. Truly unknown symbols stop the build
//! with the full linker output (add them via argv and re-run, no recompile).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

const TARGET: &str = "x86_64-pc-windows-msvc";

// Starter symbols per import lib. Superset-safe: unreferenced members are
// dropped by the linker. kernel32/ntdll starters come from the proven
// January-probe lists (socket funcs and Rtl/Nt* moved to their right homes).
const KERNEL32: &[&str] = &[
    "AddVectoredExceptionHandler",
    "CancelIo",
    "CloseHandle",
    "CompareStringOrdinal",
    "CopyFileExW",
    "CreateDirectoryW",
    "CreateEventW",
    "CreateFileW",
    "CreateHardLinkW",
    "CreateMutexA",
    "CreatePipe",
    "CreateProcessW",
    "CreateRemoteThread",
    "CreateSymbolicLinkW",
    "CreateThread",
    "CreateToolhelp32Snapshot",
    "CreateWaitableTimerExW",
    "DeleteFileW",
    "DeleteProcThreadAttributeList",
    "DeviceIoControl",
    "DuplicateHandle",
    "ExitProcess",
    "FindClose",
    "FindFirstFileExW",
    "FindFirstFileW",
    "FindNextFileW",
    "FindNextFileW",
    "FlushFileBuffers",
    "FormatMessageW",
    "FreeEnvironmentStringsW",
    "GetCommandLineW",
    "GetConsoleMode",
    "GetConsoleOutputCP",
    "GetCurrentDirectoryW",
    "GetCurrentProcess",
    "GetCurrentProcessId",
    "GetCurrentThread",
    "GetCurrentThreadId",
    "GetEnvironmentStringsW",
    "GetEnvironmentVariableW",
    "GetExitCodeProcess",
    "GetExitCodeThread",
    "GetFileAttributesW",
    "GetFileInformationByHandle",
    "GetFileInformationByHandleEx",
    "GetFileSizeEx",
    "GetFileType",
    "GetFinalPathNameByHandleW",
    "GetFullPathNameW",
    "GetHostNameW",
    "GetLastError",
    "GetModuleFileNameW",
    "GetModuleHandleA",
    "GetModuleHandleW",
    "GetOverlappedResult",
    "GetProcAddress",
    "GetProcessHeap",
    "GetProcessId",
    "GetStdHandle",
    "GetSystemDirectoryW",
    "GetSystemInfo",
    "GetSystemTimePreciseAsFileTime",
    "GetTempPathW",
    "GetWindowsDirectoryW",
    "HeapAlloc",
    "HeapFree",
    "HeapReAlloc",
    "InitializeProcThreadAttributeList",
    "LoadLibraryA",
    "LocalFree",
    "LockFileEx",
    "lstrlenW",
    "Module32FirstW",
    "MoveFileExW",
    "MultiByteToWideChar",
    "OpenProcess",
    "Process32FirstW",
    "Process32NextW",
    "QueryPerformanceCounter",
    "QueryPerformanceFrequency",
    "ReadConsoleW",
    "ReadFile",
    "ReadFileEx",
    "ReadProcessMemory",
    "ReleaseMutex",
    "RemoveDirectoryW",
    "SetCurrentDirectoryW",
    "SetEnvironmentVariableW",
    "SetFileAttributesW",
    "SetFileInformationByHandle",
    "SetFilePointerEx",
    "SetFileTime",
    "SetHandleInformation",
    "SetLastError",
    "SetThreadStackGuarantee",
    "SetWaitableTimer",
    "Sleep",
    "SleepEx",
    "SwitchToThread",
    "TerminateProcess",
    "UnlockFile",
    "UpdateProcThreadAttribute",
    "VirtualAllocEx",
    "VirtualFreeEx",
    "WaitForMultipleObjects",
    "WaitForSingleObject",
    "WaitForSingleObjectEx",
    "WideCharToMultiByte",
    "WriteConsoleW",
    "WriteFile",
    "WriteFileEx",
    "WriteProcessMemory",
];

const NTDLL: &[&str] = &[
    "NtCreateFile",
    "NtCreateNamedPipeFile",
    "NtDeviceIoControlFile",
    "NtOpenFile",
    "NtQueryDirectoryFile",
    "NtQueryInformationFile",
    "NtQuerySystemInformation",
    "NtReadFile",
    "NtSetInformationFile",
    "NtWriteFile",
    "RtlCaptureContext",
    "RtlLookupFunctionEntry",
    "RtlNtStatusToDosError",
    "RtlVirtualUnwind",
];

const CRYPT32: &[&str] = &["CryptUnprotectData"];
const OLE32: &[&str] = &[
    "CoCreateInstance",
    "CoInitializeEx",
    "CoSetProxyBlanket",
    "CoUninitialize",
];
const OLEAUT32: &[&str] = &[
    "SysAllocStringByteLen",
    "SysFreeString",
    "SysStringByteLen",
];
const BCRYPT: &[&str] = &[
    "BCryptCloseAlgorithmProvider",
    "BCryptDecrypt",
    "BCryptDestroyKey",
    "BCryptGenerateSymmetricKey",
    "BCryptOpenAlgorithmProvider",
    "BCryptSetProperty",
];
const SHELL32: &[&str] = &["CommandLineToArgvW"];
const ADVAPI32: &[&str] = &["RegCreateKeyExW", "RegSetValueExW", "RegCloseKey"];
// rustc passes these three default libs unconditionally for the msvc target,
// so the files must exist even though nothing references them (unreferenced
// members are dropped; lists mirror the proven January probe).
const USERENV: &[&str] = &["GetUserProfileDirectoryW"];
const WS2_32: &[&str] = &[
    "WSAStartup",
    "WSACleanup",
    "WSAGetLastError",
    "socket",
    "closesocket",
    "connect",
    "send",
    "recv",
    "bind",
    "listen",
    "accept",
    "setsockopt",
    "getsockopt",
    "ioctlsocket",
    "select",
    "getaddrinfo",
    "freeaddrinfo",
    "shutdown",
    "getsockname",
    "getpeername",
    "WSASocketW",
    "WSARecv",
    "WSASend",
    "recvfrom",
    "sendto",
    "WSAIoctl",
    "WSADuplicateSocketW",
];
const DBGHELP: &[&str] = &[
    "SymInitializeW",
    "SymGetLineFromAddrW64",
    "SymFromAddrW",
    "SymCleanup",
    "SymSetOptions",
    "SymGetOptions",
];

// Extra known (dll, symbol) pairs the fixpoint may pull in on demand.
const EXTRA: &[(&str, &str)] = &[
    ("kernel32", "AllocConsole"),
    ("kernel32", "AttachConsole"),
    ("kernel32", "CancelIoEx"),
    ("kernel32", "CompareStringEx"),
    ("kernel32", "CompareStringW"),
    ("kernel32", "FreeConsole"),
    ("kernel32", "GetACP"),
    ("kernel32", "GetConsoleCP"),
    ("kernel32", "GetConsoleWindow"),
    ("kernel32", "GetCPInfo"),
    ("kernel32", "GetLocalTime"),
    ("kernel32", "GetLocaleInfoEx"),
    ("kernel32", "GetLocaleInfoW"),
    ("kernel32", "GetNativeSystemInfo"),
    ("kernel32", "GetOEMCP"),
    ("kernel32", "GetProcessTimes"),
    ("kernel32", "GetStringTypeW"),
    ("kernel32", "GetSystemDefaultLCID"),
    ("kernel32", "GetSystemTimeAsFileTime"),
    ("kernel32", "GetThreadLocale"),
    ("kernel32", "GetTickCount"),
    ("kernel32", "GetTickCount64"),
    ("kernel32", "GetUserDefaultLCID"),
    ("kernel32", "IsDebuggerPresent"),
    ("kernel32", "IsValidCodePage"),
    ("kernel32", "LCMapStringEx"),
    ("kernel32", "LCMapStringW"),
    ("kernel32", "RemoveVectoredExceptionHandler"),
    ("kernel32", "SetConsoleCP"),
    ("kernel32", "SetConsoleMode"),
    ("kernel32", "SetThreadLocale"),
    ("kernel32", "SetUnhandledExceptionFilter"),
    ("kernel32", "UnhandledExceptionFilter"),
    ("ntdll", "NtQueryInformationProcess"),
    ("ntdll", "NtQueryObject"),
    ("ntdll", "RtlGetVersion"),
];

fn die(msg: &str) -> ! {
    eprintln!("xbuild: {msg}");
    std::process::exit(1);
}

fn find_rustc() -> String {
    if let Ok(r) = std::env::var("RUSTC") {
        return r;
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(&home).join(".cargo/bin/rustc");
        if p.is_file() {
            return p.to_string_lossy().into_owned();
        }
    }
    "rustc".to_string()
}

fn cmd_out(prog: &str, args: &[&str]) -> String {
    let o = Command::new(prog)
        .args(args)
        .output()
        .unwrap_or_else(|e| die(&format!("cannot run {prog}: {e}")));
    if !o.status.success() {
        die(&format!(
            "{prog} {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn run(prog: &str, args: &[String], env_extra: &[(&str, &str)]) {
    let mut c = Command::new(prog);
    c.args(args);
    for (k, v) in env_extra {
        c.env(k, v);
    }
    let o = c
        .output()
        .unwrap_or_else(|e| die(&format!("cannot run {prog}: {e}")));
    if !o.status.success() {
        die(&format!(
            "{prog} failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    let out = String::from_utf8_lossy(&o.stdout);
    if !out.trim().is_empty() {
        print!("{out}");
    }
}

fn gen_libs(mkimplib: &str, winlib: &PathBuf, sets: &BTreeMap<String, BTreeSet<String>>) {
    for (dll, syms) in sets {
        let lib = winlib.join(format!("{dll}.lib"));
        let mut args = vec![format!("{dll}.dll"), lib.to_string_lossy().into_owned()];
        args.extend(syms.iter().cloned());
        run(mkimplib, &args, &[]);
    }
}

fn parse_undefined(stderr: &str) -> Vec<String> {
    let mut v = Vec::new();
    for line in stderr.lines() {
        if let Some(pos) = line.find("undefined symbol:") {
            let mut s = line[pos + "undefined symbol:".len()..].trim().to_string();
            // lld sometimes annotates: `symbol (foo)`. Take the first token.
            s = s.split_whitespace().next().unwrap_or("").to_string();
            s = s.trim_matches(|c| c == '"' || c == '\'').to_string();
            if let Some(stripped) = s.strip_prefix("__imp_") {
                s = stripped.to_string();
            }
            if !s.is_empty() && !v.contains(&s) {
                v.push(s);
            }
        }
    }
    v
}

fn link_one(
    rustc: &str,
    lld: &str,
    winlib: &PathBuf,
    src: &PathBuf,
    out_exe: &PathBuf,
    is_dll: bool,
    sets: &mut BTreeMap<String, BTreeSet<String>>,
    dll_of: &BTreeMap<String, String>,
    mkimplib: &str,
) {
    for round in 1..=8 {
        let mut args: Vec<String> = vec![
            "--edition=2021".into(),
            "-O".into(),
            "--target".into(),
            TARGET.into(),
        ];
        // Every generated import lib must be named explicitly: /LIBPATH only
        // tells lld-link where to look, and rustc's own default set covers
        // just kernel32/ntdll/userenv/ws2_32/dbghelp.
        for dll in sets.keys() {
            args.push("-C".into());
            args.push(format!("link-arg={dll}.lib"));
        }
        args.extend(
            [
                "-C".to_string(),
                format!("linker={lld}"),
                "-C".into(),
                "linker-flavor=lld-link".into(),
                "-C".into(),
                "panic=abort".into(),
                "-C".into(),
                format!("link-arg=/LIBPATH:{}", winlib.to_string_lossy()),
                "-C".into(),
                "link-arg=/errorlimit:0".into(),
                "-C".into(),
                // Our shim provides the handful of CRT symbols std needs;
                // there is no real msvcrt import lib and none is wanted.
                "link-arg=/NODEFAULTLIB:msvcrt".into(),
                "-C".into(),
                "link-arg=/SUBSYSTEM:CONSOLE".into(),
                "-C".into(),
                "link-arg=/ALTERNATENAME:??_7type_info@@6B@=__RUSTC_TYPE_INFO_VFTABLE"
                    .into(),
            ]
            .into_iter(),
        );
        if is_dll {
            args.push("--crate-type=cdylib".into());
            args.push("-C".into());
            args.push("link-arg=/ENTRY:DllMain".into());
        }
        args.push("-o".into());
        args.push(out_exe.to_string_lossy().into_owned());
        args.push(src.to_string_lossy().into_owned());

        let mut c = Command::new(rustc);
        c.args(&args);
        c.env("RUSTC_BOOTSTRAP", "1");
        let o = c
            .output()
            .unwrap_or_else(|e| die(&format!("cannot run rustc: {e}")));
        if o.status.success() {
            println!(
                "linked {} (round {round})",
                out_exe.file_name().unwrap().to_string_lossy()
            );
            return;
        }
        let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
        let undef = parse_undefined(&stderr);
        if undef.is_empty() {
            die(&format!(
                "rustc failed with no undefined symbols (compile error?):\n{stderr}"
            ));
        }
        let mut added = Vec::new();
        let mut unknown = Vec::new();
        for s in &undef {
            match dll_of.get(s) {
                Some(dll) => {
                    let set = sets.get_mut(dll).unwrap();
                    if set.insert(s.clone()) {
                        added.push(format!("{dll}:{s}"));
                    }
                }
                None => unknown.push(s.clone()),
            }
        }
        if !unknown.is_empty() {
            die(&format!(
                "unknown undefined symbols (add via argv dll:sym): {}\nfull stderr:\n{stderr}",
                unknown.join(", ")
            ));
        }
        if added.is_empty() {
            die(&format!(
                "link still failing, nothing new to add ({}):\n{stderr}",
                undef.join(", ")
            ));
        }
        println!("round {round}: adding {}", added.join(", "));
        gen_libs(mkimplib, winlib, sets);
    }
    die("fixpoint did not converge in 8 rounds");
}

fn verify_pe(path: &PathBuf, want_dll: bool) {
    let b = std::fs::read(path).unwrap_or_else(|e| die(&format!("read {}: {e}", path.display())));
    let fail = |m: &str| -> ! { die(&format!("{}: bad PE: {m}", path.display())) };
    if b.len() < 0x100 || b[0] != b'M' || b[1] != b'Z' {
        fail("missing MZ");
    }
    let pe_off = u32::from_le_bytes([b[0x3c], b[0x3d], b[0x3e], b[0x3f]]) as usize;
    if b.len() < pe_off + 6 || b[pe_off..pe_off + 4] != [b'P', b'E', 0, 0] {
        fail("missing PE sig");
    }
    let machine = u16::from_le_bytes([b[pe_off + 4], b[pe_off + 5]]);
    if machine != 0x8664 {
        fail(&format!("machine 0x{machine:04x}, want 0x8664"));
    }
    let nsec = u16::from_le_bytes([b[pe_off + 6], b[pe_off + 7]]);
    let chars = u16::from_le_bytes([b[pe_off + 22], b[pe_off + 23]]);
    let is_dll = chars & 0x2000 != 0;
    if is_dll != want_dll {
        fail(&format!("dll flag {is_dll}, want {want_dll}"));
    }
    println!(
        "verified {}: PE32+ AMD64, {} sections, {} bytes{}",
        path.file_name().unwrap().to_string_lossy(),
        nsec,
        b.len(),
        if is_dll { ", DLL" } else { "" }
    );
}

fn main() {
    let root = option_env!("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    // argv: dll:Sym,Sym ...
    let mut cli_extra: Vec<(String, String)> = Vec::new();
    for a in std::env::args().skip(1) {
        match a.split_once(':') {
            Some((dll, syms)) => {
                for s in syms.split(',') {
                    if !s.is_empty() {
                        cli_extra.push((dll.to_string(), s.to_string()));
                    }
                }
            }
            None => die(&format!("bad arg {a:?}, want dll:Sym,Sym")),
        }
    }

    let out = root.join("target/xbuild");
    let winlib = out.join("winlib");
    std::fs::create_dir_all(&winlib).expect("mkdir target/xbuild/winlib");

    let rustc = find_rustc();
    let sysroot = cmd_out(&rustc, &["--print", "sysroot"]);
    let sysroot = sysroot.trim();
    let lld = PathBuf::from(sysroot)
        .join("lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld");
    if !lld.is_file() {
        die(&format!("rust-lld not at {}", lld.display()));
    }
    let std_dir =
        PathBuf::from(sysroot).join(format!("lib/rustlib/{TARGET}"));
    if !std_dir.is_dir() {
        die(&format!(
            "target std missing: {} (rustup target add {TARGET})",
            std_dir.display()
        ));
    }
    println!("rustc: {rustc}");
    println!("rust-lld: {}", lld.display());

    let mkimplib_src = root.join("mkimplib.rs");
    let mkimplib = out.join("mkimplib");
    run(
        &rustc,
        &[
            "--edition=2021".into(),
            "-O".into(),
            "-o".into(),
            mkimplib.to_string_lossy().into_owned(),
            mkimplib_src.to_string_lossy().into_owned(),
        ],
        &[],
    );

    let mut sets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut dll_of: BTreeMap<String, String> = BTreeMap::new();
    let mut add_const = |dll: &str, syms: &[&str]| {
        let set = sets.entry(dll.to_string()).or_default();
        for s in syms {
            set.insert(s.to_string());
            dll_of.insert(s.to_string(), dll.to_string());
        }
    };
    add_const("kernel32", KERNEL32);
    add_const("ntdll", NTDLL);
    add_const("crypt32", CRYPT32);
    add_const("ole32", OLE32);
    add_const("oleaut32", OLEAUT32);
    add_const("bcrypt", BCRYPT);
    add_const("shell32", SHELL32);
    add_const("advapi32", ADVAPI32);
    add_const("userenv", USERENV);
    add_const("ws2_32", WS2_32);
    add_const("dbghelp", DBGHELP);
    for (dll, s) in EXTRA {
        dll_of.insert(s.to_string(), dll.to_string());
    }
    for (dll, s) in &cli_extra {
        sets.entry(dll.clone()).or_default().insert(s.clone());
        dll_of.insert(s.clone(), dll.clone());
    }

    gen_libs(&mkimplib.to_string_lossy(), &winlib, &sets);

    let exe = out.join("credread.exe");
    link_one(
        &rustc,
        &lld.to_string_lossy(),
        &winlib,
        &root.join("src/main.rs"),
        &exe,
        false,
        &mut sets,
        &dll_of,
        &mkimplib.to_string_lossy(),
    );
    let dll = out.join("payload.dll");
    link_one(
        &rustc,
        &lld.to_string_lossy(),
        &winlib,
        &root.join("src/payload.rs"),
        &dll,
        true,
        &mut sets,
        &dll_of,
        &mkimplib.to_string_lossy(),
    );
    verify_pe(&exe, false);
    verify_pe(&dll, true);
    println!("done:\n  {}\n  {}", exe.display(), dll.display());
}

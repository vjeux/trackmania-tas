// shim — the handful of C-runtime symbols an MSVC-target link needs, so the
// binary can be cross-linked on a box with no Windows SDK and no mingw.
//
// Adapted from the January-probe crtshim (same proven set). This file holds
// everything EXCEPT the entry point: the stack probe, the mem* intrinsics,
// the never-reached unwind personality, and the TLS/typeinfo statics.
// The entry point lives in entry.rs and is only linked into the .exe;
// the .dll links this file alone (its entry is DllMain via /ENTRY).

use core::ffi::c_void;

extern "C" {
    fn ExitProcess(code: u32) -> !;
}

/// MSVC emits a call to __chkstk in any function with a frame larger than one
/// page: it touches each page downward so the guard page is hit in order and
/// the stack grows properly. rax carries the frame size and must be preserved.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn __chkstk() {
    core::arch::naked_asm!(
        "push rcx",
        "push rax",
        "cmp   rax, 0x1000",
        "lea   rcx, [rsp+0x18]",
        "jb    2f",
        "1:",
        "sub   rcx, 0x1000",
        "test  [rcx], ecx",
        "sub   rax, 0x1000",
        "cmp   rax, 0x1000",
        "ja    1b",
        "2:",
        "sub   rcx, rax",
        "test  [rcx], ecx",
        "pop   rax",
        "pop   rcx",
        "ret",
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *d.add(i) = *s.add(i);
        i += 1;
    }
    d
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    if (d as usize) < (s as usize) {
        let mut i = 0;
        while i < n {
            *d.add(i) = *s.add(i);
            i += 1;
        }
    } else {
        let mut i = n;
        while i > 0 {
            i -= 1;
            *d.add(i) = *s.add(i);
        }
    }
    d
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(d: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *d.add(i) = c as u8;
        i += 1;
    }
    d
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let (x, y) = (*a.add(i), *b.add(i));
        if x != y {
            return x as i32 - y as i32;
        }
        i += 1;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memchr(s: *const u8, c: i32, n: usize) -> *mut c_void {
    let mut i = 0;
    while i < n {
        if *s.add(i) == c as u8 {
            return s.add(i) as *mut c_void;
        }
        i += 1;
    }
    core::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

/// Unreachable: the binary is built with panic=abort, so no unwind ever runs.
/// It has to resolve because std references it from landing-pad metadata.
#[unsafe(no_mangle)]
pub extern "C" fn __CxxFrameHandler3() -> i32 {
    unsafe { ExitProcess(0xC0000409) }
}

#[unsafe(no_mangle)]
pub extern "C" fn _fltused() {}

// TLS directory support: MSVC's CRT defines these; std's TLS lowering
// references them even when no #[thread_local] is used.
// These MUST be writable. The loader stores the module's TLS slot index into
// _tls_index at process start (through _tls_used.address_of_index); if it
// lands in .rdata the very first thing the process does is fault on a
// read-only page -- which looks exactly like "the binary produced no output".
// The template must be well-formed: AddressOfIndex is a real pointer (with a
// relocation the loader fixes up under ASLR), and Start==End with zero fill
// describes the (empty) initialized TLS image.
#[unsafe(no_mangle)]
pub static mut _tls_index: u32 = 0;

#[repr(C)]
pub struct TlsTemplate {
    pub start_of_raw_data: *const u8,
    pub end_of_raw_data: *const u8,
    pub address_of_index: *const u32,
    pub address_of_callbacks: *const u64,
    pub size_of_zero_fill: u32,
    pub characteristics: u32,
}

#[unsafe(no_mangle)]
pub static mut _tls_used: TlsTemplate = TlsTemplate {
    start_of_raw_data: core::ptr::null(),
    end_of_raw_data: core::ptr::null(),
    address_of_index: core::ptr::addr_of!(_tls_index),
    address_of_callbacks: core::ptr::null(),
    size_of_zero_fill: 0,
    characteristics: 0,
};

const _: () = assert!(core::mem::size_of::<TlsTemplate>() == 40);

// `const type_info::'vftable'` — the MSVC RTTI vtable that std's panic
// machinery names in its type descriptor. With panic=abort no throw is ever
// constructed, so nothing dereferences it; it only has to exist so the
// reference resolves.
#[unsafe(no_mangle)]
pub static mut __RUSTC_TYPE_INFO_VFTABLE: [usize; 4] = [0; 4];

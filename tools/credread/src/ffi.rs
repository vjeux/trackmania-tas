//! Raw Win32 / COM / CNG FFI for the msvc cross-link.
//!
//! Every symbol in the extern blocks must exist in the generated import libs
//! (xbuild's symbol map — keep the two in sync). std's own imports are covered
//! by the starter lists there plus the fixpoint loop.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use core::ffi::c_void;

pub type HANDLE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type BOOL = i32;
pub type DWORD = u32;
pub type WORD = u16;
pub type SIZE_T = usize;
pub type HRESULT = i32;
pub type NTSTATUS = i32;
pub type BSTR = *mut u16;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;
pub type LPCSTR = *const u8;
pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;
pub type FARPROC = *mut c_void;
pub type BCRYPT_ALG_HANDLE = *mut c_void;
pub type BCRYPT_KEY_HANDLE = *mut c_void;

pub const TRUE: BOOL = 1;
pub const FALSE: BOOL = 0;
pub const INFINITE: DWORD = 0xFFFF_FFFF;
pub const WAIT_OBJECT_0: DWORD = 0;
pub const WAIT_TIMEOUT: DWORD = 0x102;
pub const MEM_COMMIT: DWORD = 0x1000;
pub const MEM_RESERVE: DWORD = 0x2000;
pub const PAGE_READWRITE: DWORD = 4;
pub const DLL_PROCESS_ATTACH: DWORD = 1;

pub const PROCESS_CREATE_THREAD: DWORD = 0x0002;
pub const PROCESS_VM_OPERATION: DWORD = 0x0008;
pub const PROCESS_VM_READ: DWORD = 0x0010;
pub const PROCESS_VM_WRITE: DWORD = 0x0020;
pub const PROCESS_QUERY_INFORMATION: DWORD = 0x0400;

pub const STARTF_USESHOWWINDOW: DWORD = 0x0001;
pub const SW_HIDE: WORD = 0;

pub const COINIT_APARTMENTTHREADED: DWORD = 0x2;
pub const CLSCTX_LOCAL_SERVER: DWORD = 4;
pub const RPC_C_AUTHN_DEFAULT: DWORD = 0xFFFF_FFFF;
pub const RPC_C_AUTHZ_DEFAULT: DWORD = 0xFFFF_FFFF;
pub const RPC_C_AUTHN_LEVEL_PKT_PRIVACY: DWORD = 6;
pub const RPC_C_IMP_LEVEL_IMPERSONATE: DWORD = 3;
pub const EOAC_DYNAMIC_CLOAKING: DWORD = 0x0040;

pub const S_OK: HRESULT = 0;
pub const STATUS_SUCCESS: NTSTATUS = 0;
pub const STATUS_AUTH_TAG_MISMATCH: NTSTATUS = 0xC000_A002u32 as i32;

// Elevator failure codes (elevator.h): MAKE_HRESULT(ERROR, ITF, 0xA0xx).
pub const E_VALIDATION_DID_NOT_PASS: HRESULT = 0x8004_A007u32 as i32;
pub const E_DECRYPT_USER_CTX: HRESULT = 0x8004_A003u32 as i32;
pub const E_DECRYPT_SYSTEM_CTX: HRESULT = 0x8004_A004u32 as i32;
pub const E_INVALID_VALIDATION_DATA: HRESULT = 0x8004_A00Bu32 as i32;
pub const E_COULD_NOT_OBTAIN_CALLING_PROCESS: HRESULT = 0x8004_A001u32 as i32;
// Success-with-reencrypt-hint: treat exactly like S_OK (do NOT re-encrypt).
pub const S_SHOULD_REENCRYPT: HRESULT = 0x0004_A001u32 as i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GUID {
    pub d1: u32,
    pub d2: u16,
    pub d3: u16,
    pub d4: [u8; 8],
}

/// COM class of the stable-Chrome elevation object (from the box registry:
/// HKCR\CLSID\{...}\AppID -> HKCR\AppID\{...}\LocalService =
/// GoogleChromeElevationService).
pub const CLSID_ELEVATOR_CHROME: GUID = GUID {
    d1: 0x708860E0,
    d2: 0xF641,
    d3: 0x4611,
    d4: [0x88, 0x95, 0x7D, 0x86, 0x7D, 0xD3, 0x67, 0x5B],
};

/// IElevator2Chrome (elevation_service_idl.idl). The v1 flavor IID
/// (IElevatorChrome) is stale system-wide: its typelib registration points at
/// a deleted 143 dir, and the CURRENT exe's typelib resource carries the v2
/// library guid. DecryptData is inherited at the same vtable slot (5).
pub const IID_IELEVATOR_CHROME: GUID = GUID {
    d1: 0x1BF5208B,
    d2: 0x295F,
    d3: 0x4992,
    d4: [0xB5, 0xF4, 0x3A, 0x9B, 0xB6, 0x49, 0x48, 0x38],
};

#[repr(C)]
pub struct DATA_BLOB {
    pub cbData: DWORD,
    pub pbData: *mut u8,
}

#[repr(C)]
pub struct BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
    pub cbSize: DWORD,
    pub dwInfoVersion: DWORD,
    pub pbNonce: *mut u8,
    pub cbNonce: DWORD,
    pub pbAuthData: *mut u8,
    pub cbAuthData: DWORD,
    pub pbTag: *mut u8,
    pub cbTag: DWORD,
    pub pbMacContext: *mut u8,
    pub cbMacContext: DWORD,
    pub cbAAD: DWORD,
    pub cbData: u64,
    pub dwFlags: DWORD,
}

#[repr(C)]
pub struct STARTUPINFOW {
    pub cb: DWORD,
    pub lpReserved: LPWSTR,
    pub lpDesktop: LPWSTR,
    pub lpTitle: LPWSTR,
    pub dwX: DWORD,
    pub dwY: DWORD,
    pub dwXSize: DWORD,
    pub dwYSize: DWORD,
    pub dwXCountChars: DWORD,
    pub dwYCountChars: DWORD,
    pub dwFillAttribute: DWORD,
    pub dwFlags: DWORD,
    pub wShowWindow: WORD,
    pub cbReserved2: WORD,
    pub lpReserved2: *mut u8,
    pub hStdInput: HANDLE,
    pub hStdOutput: HANDLE,
    pub hStdError: HANDLE,
}

#[repr(C)]
pub struct PROCESS_INFORMATION {
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub dwProcessId: DWORD,
    pub dwThreadId: DWORD,
}

#[repr(C)]
pub struct WIN32_FIND_DATAW {
    pub dwFileAttributes: DWORD,
    // FILETIME as u32 pairs: a u64 here would force 8-alignment and shift
    // every later field by 4 bytes (cFileName would read garbage).
    pub ftCreationTimeLow: DWORD,
    pub ftCreationTimeHigh: DWORD,
    pub ftLastAccessTimeLow: DWORD,
    pub ftLastAccessTimeHigh: DWORD,
    pub ftLastWriteTimeLow: DWORD,
    pub ftLastWriteTimeHigh: DWORD,
    pub nFileSizeHigh: DWORD,
    pub nFileSizeLow: DWORD,
    pub dwReserved0: DWORD,
    pub dwReserved1: DWORD,
    pub cFileName: [u16; 260],
    pub cAlternateFileName: [u16; 14],
}

pub const INVALID_FILE_ATTRIBUTES: DWORD = 0xFFFF_FFFF;
pub const FILE_ATTRIBUTE_DIRECTORY: DWORD = 0x10;
pub const HKEY_CURRENT_USER: HANDLE = 0x8000_0001 as HANDLE;
pub const KEY_WRITE: DWORD = 0x20006;
pub const REG_SZ: DWORD = 1;
pub const TYPE_E_CANTLOADLIBRARY: HRESULT = 0x80029C4Au32 as i32;

/// IElevator vtable slot for DecryptData: IUnknown(0..3) +
/// RunRecoveryCRXElevated(3) + EncryptData(4) + DecryptData(5).
pub const VTBL_DECRYPT_DATA: usize = 5;
pub const VTBL_RELEASE: usize = 2;

pub type DecryptDataFn =
    unsafe extern "system" fn(this: LPVOID, ct: BSTR, pt: *mut BSTR, le: *mut DWORD) -> HRESULT;
pub type ReleaseFn = unsafe extern "system" fn(this: LPVOID) -> u32;

extern "system" {
    // kernel32
    pub fn GetCommandLineW() -> LPWSTR;
    pub fn GetLastError() -> DWORD;
    pub fn Sleep(ms: DWORD);
    pub fn CloseHandle(h: HANDLE) -> BOOL;
    pub fn LocalFree(h: LPVOID) -> LPVOID;
    pub fn GetModuleHandleA(name: LPCSTR) -> HMODULE;
    pub fn GetProcAddress(h: HMODULE, name: LPCSTR) -> FARPROC;
    pub fn OpenProcess(access: DWORD, inherit: BOOL, pid: DWORD) -> HANDLE;
    pub fn VirtualAllocEx(
        h: HANDLE,
        addr: LPVOID,
        size: SIZE_T,
        alloc: DWORD,
        prot: DWORD,
    ) -> LPVOID;
    pub fn WriteProcessMemory(
        h: HANDLE,
        addr: LPVOID,
        buf: LPCVOID,
        n: SIZE_T,
        written: *mut SIZE_T,
    ) -> BOOL;
    pub fn CreateRemoteThread(
        h: HANDLE,
        attrs: LPVOID,
        stack: SIZE_T,
        start: LPVOID,
        param: LPVOID,
        flags: DWORD,
        tid: *mut DWORD,
    ) -> HANDLE;
    pub fn WaitForSingleObject(h: HANDLE, ms: DWORD) -> DWORD;
    pub fn GetExitCodeThread(h: HANDLE, code: *mut DWORD) -> BOOL;
    pub fn TerminateProcess(h: HANDLE, code: u32) -> BOOL;
    pub fn CreateProcessW(
        app: LPCWSTR,
        cmd: LPWSTR,
        pa: LPVOID,
        ta: LPVOID,
        inherit: BOOL,
        flags: DWORD,
        env: LPVOID,
        dir: LPCWSTR,
        si: *const STARTUPINFOW,
        pi: *mut PROCESS_INFORMATION,
    ) -> BOOL;
    pub fn CreateThread(
        attrs: LPVOID,
        stack: SIZE_T,
        start: LPVOID,
        param: LPVOID,
        flags: DWORD,
        tid: *mut DWORD,
    ) -> HANDLE;
    pub fn FindFirstFileW(pat: LPCWSTR, data: *mut WIN32_FIND_DATAW) -> HANDLE;
    pub fn FindNextFileW(h: HANDLE, data: *mut WIN32_FIND_DATAW) -> BOOL;
    pub fn FindClose(h: HANDLE) -> BOOL;
    pub fn GetFileAttributesW(p: LPCWSTR) -> DWORD;
    // advapi32 (registry)
    pub fn RegCreateKeyExW(
        hkey: HANDLE,
        sub: LPCWSTR,
        res: DWORD,
        class: LPVOID,
        opts: DWORD,
        access: DWORD,
        sec: LPVOID,
        out: *mut HANDLE,
        disp: *mut DWORD,
    ) -> i32;
    pub fn RegSetValueExW(
        hkey: HANDLE,
        name: LPCWSTR,
        res: DWORD,
        typ: DWORD,
        data: *const u8,
        len: DWORD,
    ) -> i32;
    pub fn RegCloseKey(hkey: HANDLE) -> i32;
    // shell32 (argv parsing; no std runtime init runs under our entry)
    pub fn CommandLineToArgvW(cmd: LPCWSTR, argc: *mut i32) -> *mut LPWSTR;
    // crypt32 (DPAPI)
    pub fn CryptUnprotectData(
        inp: *const DATA_BLOB,
        descr: *mut LPWSTR,
        entropy: *const DATA_BLOB,
        res: LPVOID,
        prompt: LPVOID,
        flags: DWORD,
        out: *mut DATA_BLOB,
    ) -> BOOL;
    // ole32 (COM)
    pub fn CoInitializeEx(res: LPVOID, coinit: DWORD) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoCreateInstance(
        clsid: *const GUID,
        outer: LPVOID,
        ctx: DWORD,
        iid: *const GUID,
        out: *mut LPVOID,
    ) -> HRESULT;
    pub fn CoSetProxyBlanket(
        proxy: LPVOID,
        authn: DWORD,
        authz: DWORD,
        princ: LPVOID,
        authnlevel: DWORD,
        implevel: DWORD,
        authinfo: LPVOID,
        caps: DWORD,
    ) -> HRESULT;
    // oleaut32 (BSTR)
    pub fn SysAllocStringByteLen(s: LPCSTR, len: u32) -> BSTR;
    pub fn SysFreeString(s: BSTR);
    pub fn SysStringByteLen(s: BSTR) -> u32;
    // bcrypt (CNG)
    pub fn BCryptOpenAlgorithmProvider(
        h: *mut BCRYPT_ALG_HANDLE,
        alg: LPCWSTR,
        impl_: LPCWSTR,
        flags: DWORD,
    ) -> NTSTATUS;
    pub fn BCryptSetProperty(
        h: LPVOID,
        prop: LPCWSTR,
        buf: *const u8,
        len: u32,
        flags: DWORD,
    ) -> NTSTATUS;
    pub fn BCryptGenerateSymmetricKey(
        h: BCRYPT_ALG_HANDLE,
        key: *mut BCRYPT_KEY_HANDLE,
        obj: *mut u8,
        objlen: u32,
        secret: *const u8,
        secretlen: u32,
        flags: DWORD,
    ) -> NTSTATUS;
    pub fn BCryptDecrypt(
        key: BCRYPT_KEY_HANDLE,
        inp: *const u8,
        inlen: u32,
        pad: LPVOID,
        iv: *mut u8,
        ivlen: u32,
        out: *mut u8,
        outlen: u32,
        reslen: *mut u32,
        flags: DWORD,
    ) -> NTSTATUS;
    pub fn BCryptDestroyKey(key: BCRYPT_KEY_HANDLE) -> NTSTATUS;
    pub fn BCryptCloseAlgorithmProvider(h: BCRYPT_ALG_HANDLE, flags: DWORD) -> NTSTATUS;
}

/// UTF-16 + NUL for Win32 string args.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

// Layout locks: repr(C) padding mistakes (e.g. a u64 after a u32 shifting
// every later field) silently corrupt FFI reads. Sizes are the SDK's.
const _: () = assert!(core::mem::size_of::<GUID>() == 16);
const _: () = assert!(core::mem::size_of::<DATA_BLOB>() == 16);
const _: () = assert!(core::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() == 88);
const _: () = assert!(core::mem::size_of::<STARTUPINFOW>() == 104);
const _: () = assert!(core::mem::size_of::<PROCESS_INFORMATION>() == 24);
const _: () = assert!(core::mem::size_of::<WIN32_FIND_DATAW>() == 592);

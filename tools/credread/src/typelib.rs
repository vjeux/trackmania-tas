//! Typelib self-heal for the elevation-service COM call.
//!
//! IElevatorChrome marshals through the automation proxy (PSOAInterface),
//! which needs the registered typelib. Chrome upgrades can leave the HKLM
//! registration pointing at a deleted version dir (observed: 143 gone, 153
//! installed) and then every CoCreateInstance fails with
//! TYPE_E_CANTLOADLIBRARY — including Chrome's own. The fix writes a per-user
//! (HKCU, no admin needed) TypeLib override pointing at the installed
//! elevation_service.exe. HKCU shadows HKLM in the merged HKCR view.

use crate::ffi::*;
use core::ptr::null_mut;

const APP_BASE: &str = "C:\\Program Files\\Google\\Chrome\\Application";
const TYPELIB_GUID: &str = "{1BF5208B-295F-4992-B5F4-3A9BB6494838}";

fn file_name_len(w: &[u16; 260]) -> usize {
    let mut n = 0;
    while n < 260 && w[n] != 0 {
        n += 1;
    }
    n
}

/// Locate the installed elevation_service.exe across version dirs.
pub fn find_elevation_service() -> Result<String, String> {
    unsafe {
        let pat = wide(&format!("{APP_BASE}\\*"));
        let mut data: WIN32_FIND_DATAW = core::mem::zeroed();
        let h = FindFirstFileW(pat.as_ptr(), &mut data);
        if h.is_null() || h as isize == -1 {
            return Err(format!(
                "FindFirstFileW({APP_BASE}) failed: gle=0x{:08x}",
                GetLastError()
            ));
        }
        loop {
            let n = file_name_len(&data.cFileName);
            let first = if n > 0 { data.cFileName[0] } else { 0 };
            let is_dir = data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
            if n > 0 && first != b'.' as u16 && is_dir {
                let dir = String::from_utf16_lossy(&data.cFileName[..n]);
                let cand = format!("{APP_BASE}\\{dir}\\elevation_service.exe");
                let cand_w = wide(&cand);
                if GetFileAttributesW(cand_w.as_ptr()) != INVALID_FILE_ATTRIBUTES {
                    FindClose(h);
                    return Ok(cand);
                }
            }
            if FindNextFileW(h, &mut data) == 0 {
                break;
            }
        }
        FindClose(h);
        Err("no elevation_service.exe under ".to_string() + APP_BASE)
    }
}

fn set_default(key_path: &str, value: &str) -> Result<(), String> {
    unsafe {
        let sub = wide(key_path);
        let mut h: HANDLE = null_mut();
        let mut disp: DWORD = 0;
        let rc = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            0,
            null_mut(),
            0,
            KEY_WRITE,
            null_mut(),
            &mut h,
            &mut disp,
        );
        if rc != 0 {
            return Err(format!("RegCreateKeyExW({key_path}) rc={rc}"));
        }
        let val = wide(value);
        let bytes = core::slice::from_raw_parts(val.as_ptr() as *const u8, val.len() * 2);
        let rc = RegSetValueExW(h, null_mut(), 0, REG_SZ, bytes.as_ptr(), bytes.len() as u32);
        RegCloseKey(h);
        if rc != 0 {
            return Err(format!("RegSetValueExW({key_path}) rc={rc}"));
        }
        Ok(())
    }
}

/// Write the HKCU TypeLib override; returns the exe path used.
pub fn ensure_elevator_typelib() -> Result<String, String> {
    let exe = find_elevation_service()?;
    let base = format!("Software\\Classes\\TypeLib\\{TYPELIB_GUID}");
    set_default(&format!("{base}\\1.0"), "Chrome Elevation Service TypeLib")?;
    set_default(&format!("{base}\\1.0\\0\\win64"), &exe)?;
    set_default(&format!("{base}\\1.0\\0\\win32"), &exe)?;
    Ok(exe)
}

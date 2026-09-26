//! AES-256-GCM decryption through Windows CNG (bcrypt.dll). No hand-rolled
//! crypto: the OS implementation is the reference.

use crate::ffi::*;
use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::null_mut;

fn wstr_len(w: &[u16]) -> u32 {
    (w.len() * 2) as u32
}

pub fn aes_gcm_decrypt(
    key: &[u8],
    nonce: &[u8],
    ct: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err(format!("bad key len {}", key.len()));
    }
    if nonce.len() != 12 {
        return Err(format!("bad nonce len {}", nonce.len()));
    }
    if tag.len() != 16 {
        return Err(format!("bad tag len {}", tag.len()));
    }
    unsafe {
        let mut halg: BCRYPT_ALG_HANDLE = null_mut();
        let aes = wide("AES");
        let st = BCryptOpenAlgorithmProvider(&mut halg, aes.as_ptr(), null_mut(), 0);
        if st != STATUS_SUCCESS {
            return Err(format!("BCryptOpenAlgorithmProvider: 0x{st:08x}"));
        }
        let chaining = wide("ChainingMode");
        let gcm = wide("ChainingModeGCM");
        let st = BCryptSetProperty(
            halg,
            chaining.as_ptr(),
            gcm.as_ptr() as *const u8,
            wstr_len(&gcm),
            0,
        );
        if st != STATUS_SUCCESS {
            BCryptCloseAlgorithmProvider(halg, 0);
            return Err(format!("BCryptSetProperty(GCM): 0x{st:08x}"));
        }
        let mut hkey: BCRYPT_KEY_HANDLE = null_mut();
        let st = BCryptGenerateSymmetricKey(
            halg,
            &mut hkey,
            null_mut(),
            0,
            key.as_ptr(),
            key.len() as u32,
            0,
        );
        if st != STATUS_SUCCESS {
            BCryptCloseAlgorithmProvider(halg, 0);
            return Err(format!("BCryptGenerateSymmetricKey: 0x{st:08x}"));
        }
        let mut auth = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
            cbSize: size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
            dwInfoVersion: 1,
            pbNonce: nonce.as_ptr() as *mut u8,
            cbNonce: nonce.len() as u32,
            pbAuthData: null_mut(),
            cbAuthData: 0,
            pbTag: tag.as_ptr() as *mut u8,
            cbTag: tag.len() as u32,
            pbMacContext: null_mut(),
            cbMacContext: 0,
            cbAAD: 0,
            cbData: 0,
            dwFlags: 0,
        };
        // GCM plaintext length == ciphertext length; single call is enough.
        let mut pt = vec![0u8; ct.len()];
        let mut outlen: u32 = 0;
        let st = BCryptDecrypt(
            hkey,
            ct.as_ptr(),
            ct.len() as u32,
            &mut auth as *mut _ as *mut c_void,
            null_mut(),
            0,
            pt.as_mut_ptr(),
            pt.len() as u32,
            &mut outlen,
            0,
        );
        BCryptDestroyKey(hkey);
        BCryptCloseAlgorithmProvider(halg, 0);
        if st == STATUS_AUTH_TAG_MISMATCH {
            return Err("gcm tag mismatch (wrong key or corrupt blob)".to_string());
        }
        if st != STATUS_SUCCESS {
            return Err(format!("BCryptDecrypt: 0x{st:08x}"));
        }
        pt.truncate(outlen as usize);
        Ok(pt)
    }
}

/// Split a Chrome `v10`/`v20` blob: prefix(3) | nonce(12) | ct | tag(16).
pub fn split_gcm_blob(blob: &[u8]) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>), String> {
    if blob.len() < 3 + 12 + 16 {
        return Err(format!("blob too short: {} bytes", blob.len()));
    }
    let (prefix, rest) = blob.split_at(3);
    let (nonce, rest) = rest.split_at(12);
    let (ct, tag) = rest.split_at(rest.len() - 16);
    Ok((
        prefix.to_vec(),
        nonce.to_vec(),
        ct.to_vec(),
        tag.to_vec(),
    ))
}

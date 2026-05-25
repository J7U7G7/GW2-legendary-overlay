use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

use crate::db::repository::Db;
use crate::error::{AppError, Result};

const KEY_SETTING: &str = "gw2_api_key_dpapi_b64";
const KEY_DESCRIPTION: &str = "gw2-overlay-api-key";

/// Validated GW2 API key. Never derives Debug/Display to prevent accidental logging.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    /// Validate the canonical GW2 API key format: 8-4-4-4-20-4-4-4-12 uppercase hex.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        let expected_lengths = [8usize, 4, 4, 4, 20, 4, 4, 4, 12];
        let segments: Vec<&str> = trimmed.split('-').collect();
        if segments.len() != expected_lengths.len() {
            return Err(AppError::BadKeyFormat);
        }
        for (seg, len) in segments.iter().zip(expected_lengths.iter()) {
            if seg.len() != *len || !seg.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(AppError::BadKeyFormat);
            }
        }
        Ok(Self(trimmed.to_ascii_uppercase()))
    }

    pub fn as_bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }

    /// Account UUID (first segment block) — safe to log.
    pub fn account_id(&self) -> &str {
        // Already validated: positions 0..36 are the first canonical UUID.
        &self.0[..36]
    }
}

pub fn store_api_key(db: &Db, key: &ApiKey) -> Result<()> {
    let blob = dpapi::protect(key.0.as_bytes())?;
    db.set_setting(KEY_SETTING, &B64.encode(&blob))
}

pub fn load_api_key(db: &Db) -> Result<Option<ApiKey>> {
    let Some(b64) = db.get_setting(KEY_SETTING)? else {
        tracing::info!(
            "load_api_key: no row in settings — user will be prompted to enter a key"
        );
        return Ok(None);
    };
    tracing::debug!(
        b64_len = b64.len(),
        "load_api_key: found stored row, attempting DPAPI unprotect"
    );
    let blob = match B64.decode(b64.as_bytes()) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "load_api_key: base64 decode failed");
            return Err(AppError::WinCrypto(format!("base64: {e}")));
        }
    };
    let plain = match dpapi::unprotect(&blob) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "load_api_key: DPAPI unprotect failed");
            return Err(e);
        }
    };
    let text = match String::from_utf8(plain) {
        Ok(t) => t,
        Err(_) => {
            tracing::error!("load_api_key: UTF-8 decode of decrypted blob failed");
            return Err(AppError::BadKeyFormat);
        }
    };
    let key = match ApiKey::parse(&text) {
        Ok(k) => k,
        Err(e) => {
            tracing::error!(error = %e, "load_api_key: stored key fails format validation");
            return Err(e);
        }
    };
    tracing::info!(
        account = key.account_id(),
        "load_api_key: loaded + decrypted successfully"
    );
    Ok(Some(key))
}

pub fn clear_api_key(db: &Db) -> Result<()> {
    db.with_conn(|c| {
        c.execute("DELETE FROM settings WHERE key = ?1", rusqlite::params![KEY_SETTING])?;
        Ok(())
    })
}

#[cfg(windows)]
mod dpapi {
    use std::ffi::c_void;
    use std::ptr;

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData,
    };

    use crate::error::{AppError, Result};
    use super::KEY_DESCRIPTION;

    fn description_wide() -> Vec<u16> {
        KEY_DESCRIPTION.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error(api: &str) -> AppError {
        // SAFETY: GetLastError is always safe to call.
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        AppError::WinCrypto(format!("{api} failed (code {code})"))
    }

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>> {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() };
        let mut desc = description_wide();

        // SAFETY: pointers are valid for the duration of the call; out_blob is
        // populated by Windows and we free its pbData via LocalFree below.
        let ok = unsafe {
            CryptProtectData(
                &in_blob,
                desc.as_mut_ptr(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null(),
                0,
                &mut out_blob,
            )
        };
        if ok == 0 {
            return Err(last_error("CryptProtectData"));
        }

        // SAFETY: pbData points to cbData bytes owned by Windows.
        let copy =
            unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
        // SAFETY: pbData was LocalAlloc-d by Windows; LocalFree releases it.
        unsafe {
            LocalFree(out_blob.pbData as *mut c_void);
        }
        Ok(copy)
    }

    pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>> {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: cipher.len() as u32,
            pbData: cipher.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() };
        let mut out_desc: *mut u16 = ptr::null_mut();

        // SAFETY: standard DPAPI usage with zeroed output blob and PWSTR sink.
        let ok = unsafe {
            CryptUnprotectData(
                &in_blob,
                &mut out_desc,
                ptr::null(),
                ptr::null_mut(),
                ptr::null(),
                0,
                &mut out_blob,
            )
        };
        if ok == 0 {
            return Err(last_error("CryptUnprotectData"));
        }

        // SAFETY: pbData points to cbData bytes owned by Windows.
        let copy =
            unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
        // SAFETY: both pbData and out_desc are LocalAlloc-d by Windows.
        unsafe {
            LocalFree(out_blob.pbData as *mut c_void);
            if !out_desc.is_null() {
                LocalFree(out_desc as *mut c_void);
            }
        }
        Ok(copy)
    }
}

#[cfg(not(windows))]
mod dpapi {
    use crate::error::{AppError, Result};

    pub fn protect(_plain: &[u8]) -> Result<Vec<u8>> {
        Err(AppError::WinCrypto("DPAPI only supported on Windows".into()))
    }

    pub fn unprotect(_cipher: &[u8]) -> Result<Vec<u8>> {
        Err(AppError::WinCrypto("DPAPI only supported on Windows".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_KEY: &str =
        "D15CCC05-29C4-C045-87C3-5473312233E8EF75EBEA-309B-4D11-AB3C-8AC6D81BEFE6";

    #[test]
    fn parses_canonical_key() {
        let key = ApiKey::parse(SAMPLE_KEY).unwrap();
        assert_eq!(key.account_id(), "D15CCC05-29C4-C045-87C3-5473312233E8");
        assert!(key.as_bearer().starts_with("Bearer D15CCC05"));
    }

    #[test]
    fn normalizes_lowercase_to_uppercase() {
        let key = ApiKey::parse(&SAMPLE_KEY.to_ascii_lowercase()).unwrap();
        assert_eq!(key.account_id(), "D15CCC05-29C4-C045-87C3-5473312233E8");
    }

    #[test]
    fn rejects_bad_format() {
        assert!(matches!(ApiKey::parse("not-a-key"), Err(AppError::BadKeyFormat)));
        assert!(matches!(ApiKey::parse(""), Err(AppError::BadKeyFormat)));
        assert!(matches!(
            ApiKey::parse("D15CCC05-29C4-C045-87C3-5473312233E8"),
            Err(AppError::BadKeyFormat)
        ));
        // Non-hex
        assert!(matches!(
            ApiKey::parse("Z15CCC05-29C4-C045-87C3-5473312233E8EF75EBEA-309B-4D11-AB3C-8AC6D81BEFE6"),
            Err(AppError::BadKeyFormat)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let plain = b"hello world";
        let cipher = dpapi::protect(plain).unwrap();
        assert_ne!(cipher, plain);
        let back = dpapi::unprotect(&cipher).unwrap();
        assert_eq!(back, plain);
    }

    #[cfg(windows)]
    #[test]
    fn store_and_load_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert!(load_api_key(&db).unwrap().is_none());
        let key = ApiKey::parse(SAMPLE_KEY).unwrap();
        store_api_key(&db, &key).unwrap();
        let loaded = load_api_key(&db).unwrap().expect("key should be present");
        assert_eq!(loaded.account_id(), key.account_id());
        clear_api_key(&db).unwrap();
        assert!(load_api_key(&db).unwrap().is_none());
    }
}

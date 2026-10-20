//! Windows Credential Manager backend for terminal provider profiles.
//!
//! The key is passed to CredWriteW through memory and never becomes a command
//! argument or an environment variable. The non-Windows stub keeps the parent
//! profile module portable while the target-specific implementation compiles
//! only on Windows.

use super::ProfileError;

#[cfg(not(windows))]
pub fn credential(
    _action: &str,
    _account: &str,
    _secret: Option<&str>,
) -> Result<String, ProfileError> {
    Err(ProfileError::Keychain(
        "Windows Credential Manager is only available on Windows".into(),
    ))
}

#[cfg(windows)]
mod platform {
    use super::ProfileError;
    use std::ptr::null_mut;
    use std::slice;

    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    const TARGET_PREFIX: &str = "SecondEgo/provider/";

    fn target_name(account: &str) -> Vec<u16> {
        TARGET_PREFIX
            .encode_utf16()
            .chain(account.encode_utf16())
            .chain(std::iter::once(0))
            .collect()
    }

    fn failure(action: &str) -> ProfileError {
        ProfileError::Keychain(format!("Windows Credential Manager rejected {action}"))
    }

    pub(super) fn credential(
        action: &str,
        account: &str,
        secret: Option<&str>,
    ) -> Result<String, ProfileError> {
        let target = target_name(account);
        match action {
            "save" => {
                let secret = secret.ok_or_else(|| {
                    ProfileError::Invalid("a key is required to save a profile".into())
                })?;
                let mut blob = secret.as_bytes().to_vec();
                let blob_size = u32::try_from(blob.len()).map_err(|_| {
                    ProfileError::Invalid(
                        "provider key is too large for Windows Credential Manager".into(),
                    )
                })?;
                let credential = CREDENTIALW {
                    Type: CRED_TYPE_GENERIC,
                    TargetName: target.as_ptr() as *mut u16,
                    CredentialBlobSize: blob_size,
                    CredentialBlob: blob.as_mut_ptr(),
                    Persist: CRED_PERSIST_LOCAL_MACHINE,
                    ..Default::default()
                };
                let written = unsafe { CredWriteW(&credential, 0) };
                if written == 0 {
                    return Err(failure("save"));
                }
                Ok(String::new())
            }
            "read" => {
                let mut credential: *mut CREDENTIALW = null_mut();
                let read =
                    unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) };
                if read == 0 {
                    return Err(failure("read"));
                }
                let result = unsafe {
                    let value = &*credential;
                    let bytes = slice::from_raw_parts(
                        value.CredentialBlob,
                        value.CredentialBlobSize as usize,
                    );
                    String::from_utf8(bytes.to_vec()).map_err(|_| {
                        ProfileError::Keychain("stored Windows credential is invalid".into())
                    })
                };
                unsafe { CredFree(credential.cast()) };
                result
            }
            "delete" => {
                let deleted = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
                if deleted == 0 && unsafe { GetLastError() } != ERROR_NOT_FOUND {
                    return Err(failure("delete"));
                }
                Ok(String::new())
            }
            _ => Err(ProfileError::Invalid("unknown credential action".into())),
        }
    }
}

#[cfg(windows)]
pub use platform::credential;

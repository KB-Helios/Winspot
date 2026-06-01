//! Builds a `SECURITY_ATTRIBUTES` that confines the Winspot named pipe to the
//! current user and `SYSTEM`, denying every other account.
//!
//! Without this, the pipe is created with the default ACL, which lets any
//! process on the machine connect to the daemon and drive privileged-looking
//! actions (open files, launch commands, read file previews). Restricting the
//! DACL to the owner SID makes "same user" the trust boundary, which matches
//! what a per-user launcher should expose.

use std::{ffi::c_void, io, mem, ptr};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

/// Owns the self-relative security descriptor allocated by the SDDL converter
/// and the `SECURITY_ATTRIBUTES` that points at it. The descriptor is freed with
/// `LocalFree` when this value is dropped, so it must outlive the pipe-creation
/// call that consumes [`Self::as_attributes_ptr`].
pub struct PipeSecurity {
    security_descriptor: *mut c_void,
    attributes: SECURITY_ATTRIBUTES,
}

impl PipeSecurity {
    /// Builds a descriptor granting full access to the current user and SYSTEM
    /// only (`D:P(A;;FA;;;<user-sid>)(A;;FA;;;SY)`), with inheritance disabled.
    pub fn current_user_only() -> io::Result<Self> {
        let sid = current_user_sid_string()?;
        let sddl = build_sddl(&sid);

        let mut security_descriptor: *mut c_void = ptr::null_mut();
        // SAFETY: `sddl` is a NUL-terminated UTF-16 string; on success the call
        // allocates `security_descriptor` with `LocalAlloc`, which Drop frees.
        let created = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut security_descriptor,
                ptr::null_mut(),
            )
        };
        if created == 0 {
            return Err(io::Error::last_os_error());
        }

        let attributes = SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security_descriptor,
            bInheritHandle: 0,
        };

        Ok(Self {
            security_descriptor,
            attributes,
        })
    }

    /// Pointer to the `SECURITY_ATTRIBUTES`, suitable for tokio's
    /// `create_with_security_attributes_raw`. Valid only while `self` is alive.
    pub fn as_attributes_ptr(&self) -> *const SECURITY_ATTRIBUTES {
        &self.attributes
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        if !self.security_descriptor.is_null() {
            // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
            unsafe {
                LocalFree(self.security_descriptor as HLOCAL);
            }
            self.security_descriptor = ptr::null_mut();
        }
    }
}

/// Returns the current process user's SID as a NUL-terminated UTF-16 string
/// (e.g. `S-1-5-21-…`).
fn current_user_sid_string() -> io::Result<Vec<u16>> {
    // SAFETY: each Win32 call is checked; handles/allocations are released on
    // every path before returning.
    unsafe {
        let mut token: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut needed: u32 = 0;
        // First call sizes the buffer; it is expected to "fail" with the size
        // written to `needed`.
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            let error = io::Error::last_os_error();
            CloseHandle(token);
            return Err(error);
        }

        let mut buffer = vec![0u8; needed as usize];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr() as *mut c_void,
            needed,
            &mut needed,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }

        let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut sid_string: *mut u16 = ptr::null_mut();
        if ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string) == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut wide = Vec::new();
        let mut cursor = sid_string;
        while *cursor != 0 {
            wide.push(*cursor);
            cursor = cursor.add(1);
        }
        wide.push(0);
        LocalFree(sid_string as HLOCAL);

        Ok(wide)
    }
}

/// Builds the SDDL descriptor string (UTF-16, NUL-terminated) for the given
/// user SID: a protected DACL granting full access to the user and SYSTEM.
fn build_sddl(sid_wide: &[u16]) -> Vec<u16> {
    let trimmed = sid_wide
        .split_last()
        .map(|(_, rest)| rest)
        .unwrap_or(sid_wide);
    let sid = String::from_utf16_lossy(trimmed);
    let sddl = format!("D:P(A;;FA;;;{sid})(A;;FA;;;SY)");
    sddl.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_current_user_security_descriptor() {
        let security = PipeSecurity::current_user_only().expect("build pipe security");
        assert!(!security.as_attributes_ptr().is_null());
    }

    #[test]
    fn sddl_includes_sid_and_system_ace() {
        let sid: Vec<u16> = "S-1-5-21-1\0".encode_utf16().collect();
        let sddl = String::from_utf16_lossy(&build_sddl(&sid));
        assert!(sddl.starts_with("D:P(A;;FA;;;S-1-5-21-1)"));
        assert!(sddl.contains("(A;;FA;;;SY)"));
        assert!(sddl.ends_with('\0'));
    }
}

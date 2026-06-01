//! Validation helpers for untrusted IPC input.
//!
//! Everything the daemon receives over the named pipe is attacker-controllable,
//! so before a result id is handed to the OS (e.g. to the Windows shell) it is
//! classified and validated here. The named-pipe ACL restricts *who* can talk
//! to the daemon; these helpers restrict *what* a permitted client can ask it
//! to do.

use std::{
    fmt,
    path::{Path, PathBuf},
};

/// URI schemes the launcher is allowed to hand to the shell. Anything outside
/// this set (`file://`, `javascript:`, `search-ms:`, custom protocol handlers,
/// …) is refused so a client cannot use Winspot as a confused deputy to invoke
/// arbitrary protocol handlers.
const ALLOWED_URI_PREFIXES: [&str; 3] = ["ms-settings:", "http://", "https://"];

/// A validated shell-open target decoded from a result id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    /// An existing filesystem path.
    Path(PathBuf),
    /// A URI whose scheme is in [`ALLOWED_URI_PREFIXES`].
    Uri(String),
}

/// Why an open target was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTargetError {
    /// The id had no `scheme:` separator or an empty target.
    Missing,
    /// The target contained control characters (argument/command injection).
    Invalid,
    /// The target used a URI scheme that is not on the allowlist.
    DisallowedScheme,
    /// The target looked like a path but no such file or directory exists.
    NotFound,
}

impl fmt::Display for OpenTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            OpenTargetError::Missing => "open target is missing",
            OpenTargetError::Invalid => "open target contains invalid characters",
            OpenTargetError::DisallowedScheme => "open target uses a disallowed URI scheme",
            OpenTargetError::NotFound => "open target does not exist",
        };
        f.write_str(message)
    }
}

impl std::error::Error for OpenTargetError {}

/// Decodes and validates the shell-open target carried by a result id (the part
/// after the leading `scheme:` tag, e.g. `file:C:\…` or `setting:ms-settings:…`).
///
/// Returns a [`OpenTarget::Uri`] for allowlisted schemes, a [`OpenTarget::Path`]
/// for existing filesystem paths, and an [`OpenTargetError`] for everything
/// else. Note this intentionally allows opening any *existing* path the caller
/// could already reach: cross-user access is prevented by the named-pipe ACL,
/// not here. The job of this function is to reject malformed input and
/// non-allowlisted protocol handlers.
pub fn classify_open_target(result_id: &str) -> Result<OpenTarget, OpenTargetError> {
    let (_, raw) = result_id.split_once(':').ok_or(OpenTargetError::Missing)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(OpenTargetError::Missing);
    }
    if raw.chars().any(char::is_control) {
        return Err(OpenTargetError::Invalid);
    }

    let lowered = raw.to_ascii_lowercase();
    if ALLOWED_URI_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
    {
        return Ok(OpenTarget::Uri(raw.to_string()));
    }

    if looks_like_uri_scheme(raw) {
        return Err(OpenTargetError::DisallowedScheme);
    }

    let path = Path::new(raw);
    if !path.exists() {
        return Err(OpenTargetError::NotFound);
    }
    Ok(OpenTarget::Path(path.to_path_buf()))
}

/// True when `raw` begins with a URI scheme (`scheme:`) rather than a Windows
/// filesystem path. A single-letter scheme immediately followed by a path
/// separator is treated as a drive path (`C:\…`, `C:/…`), not a URI.
fn looks_like_uri_scheme(raw: &str) -> bool {
    let Some(colon) = raw.find(':') else {
        return false;
    };
    let scheme = &raw[..colon];
    let mut scheme_chars = scheme.chars();
    let Some(first) = scheme_chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    if !scheme_chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')) {
        return false;
    }

    let rest = &raw[colon + 1..];
    // `C:\path` / `C:/path` are drive-qualified paths, not URIs.
    !(scheme.len() == 1 && rest.starts_with(['\\', '/']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlisted_uris_are_accepted() {
        assert_eq!(
            classify_open_target("setting:ms-settings:display"),
            Ok(OpenTarget::Uri("ms-settings:display".to_string()))
        );
        assert_eq!(
            classify_open_target("browser:https://example.com"),
            Ok(OpenTarget::Uri("https://example.com".to_string()))
        );
        assert_eq!(
            classify_open_target("browser:http://example.com"),
            Ok(OpenTarget::Uri("http://example.com".to_string()))
        );
    }

    #[test]
    fn disallowed_schemes_are_rejected() {
        for id in [
            "x:javascript:alert(1)",
            "x:file:///C:/Windows/System32/cmd.exe",
            "x:search-ms:query=secret",
            "x:vbscript:msgbox",
        ] {
            assert_eq!(
                classify_open_target(id),
                Err(OpenTargetError::DisallowedScheme),
                "expected {id} to be rejected"
            );
        }
    }

    #[test]
    fn missing_or_invalid_targets_are_rejected() {
        assert_eq!(
            classify_open_target("noseparator"),
            Err(OpenTargetError::Missing)
        );
        assert_eq!(classify_open_target("file:"), Err(OpenTargetError::Missing));
        assert_eq!(
            classify_open_target("file:   "),
            Err(OpenTargetError::Missing)
        );
        assert_eq!(
            classify_open_target("file:C:\\path\nwith-newline"),
            Err(OpenTargetError::Invalid)
        );
    }

    #[test]
    fn nonexistent_paths_are_rejected() {
        assert_eq!(
            classify_open_target("file:C:\\winspot\\definitely\\missing\\file.txt"),
            Err(OpenTargetError::NotFound)
        );
    }

    #[test]
    fn existing_paths_are_accepted() {
        let path = std::env::temp_dir().join(format!("winspot-open-{}.txt", std::process::id()));
        std::fs::write(&path, b"hello").expect("write temp file");
        let id = format!("file:{}", path.display());

        match classify_open_target(&id) {
            Ok(OpenTarget::Path(resolved)) => assert_eq!(resolved, path),
            other => panic!("expected validated path, got {other:?}"),
        }

        std::fs::remove_file(&path).expect("cleanup temp file");
    }

    #[test]
    fn drive_letters_are_not_mistaken_for_uri_schemes() {
        assert!(!looks_like_uri_scheme("C:\\Users\\me\\file.txt"));
        assert!(!looks_like_uri_scheme("C:/Users/me/file.txt"));
        assert!(looks_like_uri_scheme("javascript:alert(1)"));
        assert!(looks_like_uri_scheme("ms-settings:display"));
    }
}

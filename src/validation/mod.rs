//! # Validation Module
//!
//! Ensures only valid `.vlx` files with safe content reach the pipeline.
//! Validates file extension (case-insensitive), path, content (empty, whitespace-only,
//! UTF-8, size), and optionally suspicious characters.
//!
//! ## Usage
//!
//! ```ignore
//! use veltrix::validation::validate_vlx_file;
//! use veltrix::pipeline::{run_vlx_content, RunFlags};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let validated_content = validate_vlx_file("example.vlx")?;
//!     run_vlx_content(&validated_content, RunFlags { print_ast: false, debug: true, repl: false })?;
//!     Ok(())
//! }
//! ```

use crate::file_loader::{FileLoader, VeltrixError};
use std::error::Error;
use std::fmt;
use std::path::Path;

/// Validation error variants.
/// User-facing messages only; no paths or internal details exposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VeltrixValidationError {
    /// File path is missing or empty.
    MissingPath,
    /// File does not have the required `.vlx` extension (case-insensitive).
    InvalidExtension,
    /// File does not exist or cannot be accessed.
    FileNotFound,
    /// File is empty or contains only whitespace.
    EmptyFile,
    /// File contains invalid UTF-8 byte sequences.
    InvalidUTF8,
    /// File exceeds the maximum allowed size (1 MiB).
    FileTooLarge,
    /// File contains suspicious characters (e.g. null bytes, control chars).
    SuspiciousContent,
}

impl fmt::Display for VeltrixValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VeltrixValidationError::MissingPath => write!(f, "File path is missing or empty"),
            VeltrixValidationError::InvalidExtension => {
                write!(f, "Invalid file extension: expected .vlx")
            }
            VeltrixValidationError::FileNotFound => write!(f, "File not found"),
            VeltrixValidationError::EmptyFile => {
                write!(f, "File is empty or contains only whitespace")
            }
            VeltrixValidationError::InvalidUTF8 => write!(f, "File contains invalid UTF-8"),
            VeltrixValidationError::FileTooLarge => {
                write!(f, "File exceeds maximum allowed size")
            }
            VeltrixValidationError::SuspiciousContent => {
                write!(f, "File contains suspicious or non-printable content")
            }
        }
    }
}

impl Error for VeltrixValidationError {}

impl From<VeltrixError> for VeltrixValidationError {
    fn from(e: VeltrixError) -> Self {
        match e {
            VeltrixError::FileNotFound => VeltrixValidationError::FileNotFound,
            VeltrixError::InvalidExtension => VeltrixValidationError::InvalidExtension,
            VeltrixError::InvalidUTF8 => VeltrixValidationError::InvalidUTF8,
            VeltrixError::EmptyFile => VeltrixValidationError::EmptyFile,
            VeltrixError::FileTooLarge => VeltrixValidationError::FileTooLarge,
        }
    }
}

/// Checks content for suspicious characters that could trigger undefined behavior
/// in the lexer or parser (null bytes, C0/C1 control chars).
fn has_suspicious_content(content: &str) -> bool {
    for c in content.chars() {
        match c {
            '\0' => return true,
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => return true,
            '\x7F' => return true,
            c if (c as u32) >= 0x80 && (c as u32) <= 0x9F => return true,
            _ => continue,
        }
    }
    false
}

/// Validates a `.vlx` file and returns its content if all checks pass.
///
/// # Validation Steps
///
/// 1. **Path check**: Rejects empty or missing path.
/// 2. **Extension check**: Rejects non-`.vlx` extensions (case-insensitive; `.VLX` allowed).
/// 3. **File loading**: Uses `FileLoader` for existence, size (≤1 MiB), UTF-8, empty/whitespace rejection.
/// 4. **Suspicious content**: Rejects null bytes and control characters.
///
/// # Errors
///
/// Returns `VeltrixValidationError` on any validation failure. User-facing messages only;
/// no internal paths or details are exposed.
///
/// # Example
///
/// ```ignore
/// let content = validate_vlx_file("example.vlx")?;
/// run_vlx_content(&content, RunFlags::default())?;
/// ```
pub fn validate_vlx_file(path: &str) -> Result<String, VeltrixValidationError> {
    // Step 1: Reject missing or empty path
    if path.trim().is_empty() {
        return Err(VeltrixValidationError::MissingPath);
    }

    // Step 2: Validate extension (case-insensitive, fail-fast before I/O)
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !ext.eq_ignore_ascii_case("vlx") {
        return Err(VeltrixValidationError::InvalidExtension);
    }

    // Step 3: Load file via FileLoader (handles existence, size, UTF-8, empty/whitespace)
    let content = FileLoader::load_file(path).map_err(VeltrixValidationError::from)?;

    // Step 4: Check for suspicious content
    if has_suspicious_content(&content) {
        return Err(VeltrixValidationError::SuspiciousContent);
    }

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("veltrix_validation_tests")
    }

    fn setup_temp_dir() -> std::path::PathBuf {
        let dir = temp_dir();
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_valid_vlx_file_passes() {
        let dir = setup_temp_dir();
        let path = dir.join("example.vlx");
        let content = "let x = 42\nwrite x";
        let mut f = File::create(&path).expect("create test file");
        f.write_all(content.as_bytes()).expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_ok());
        assert_eq!(result.expect("content"), content);
    }

    #[test]
    fn test_non_vlx_file_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("script.txt");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"let x = 1").expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::InvalidExtension);
    }

    #[test]
    fn test_empty_file_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("empty.vlx");
        File::create(&path).expect("create empty file");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::EmptyFile);
    }

    #[test]
    fn test_whitespace_only_file_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("whitespace.vlx");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"   \n\t  \n ").expect("write whitespace");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::EmptyFile);
    }

    #[test]
    fn test_invalid_utf8_content_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("bad_utf8.vlx");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(&[0xFF, 0xFE, 0x00]).expect("write invalid utf8");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::InvalidUTF8);
    }

    #[test]
    fn test_oversized_file_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("large.vlx");
        let mut f = File::create(&path).expect("create test file");
        let padding = vec![b'a'; 1_048_577];
        f.write_all(&padding).expect("write large content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::FileTooLarge);
    }

    #[test]
    fn test_exactly_one_mib_boundary_passes() {
        let dir = setup_temp_dir();
        let path = dir.join("max.vlx");
        let mut f = File::create(&path).expect("create test file");
        let padding = vec![b'a'; 1_048_576];
        f.write_all(&padding).expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_ok());
        assert_eq!(result.expect("content").len(), 1_048_576);
    }

    #[test]
    fn test_mixed_case_extension_passes() {
        let dir = setup_temp_dir();
        let path = dir.join("script.VLX");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"let x = 1").expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_ok());
        assert_eq!(result.expect("content"), "let x = 1");
    }

    #[test]
    fn test_empty_path_fails() {
        let result = validate_vlx_file("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::MissingPath);
    }

    #[test]
    fn test_whitespace_path_fails() {
        let result = validate_vlx_file("   ");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::MissingPath);
    }

    #[test]
    fn test_nonexistent_file_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("does_not_exist.vlx");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::FileNotFound);
    }

    #[test]
    fn test_suspicious_content_null_byte_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("null.vlx");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"let x = \0 42").expect("write null byte");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::SuspiciousContent);
    }

    #[test]
    fn test_suspicious_content_control_char_fails() {
        let dir = setup_temp_dir();
        let path = dir.join("ctrl.vlx");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"let x = \x01 42").expect("write control char");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::SuspiciousContent);
    }

    #[test]
    fn test_valid_content_with_tabs_and_newlines_passes() {
        let dir = setup_temp_dir();
        let path = dir.join("valid.vlx");
        let content = "let x = 1\n\tlet y = 2\r\nwrite x + y";
        let mut f = File::create(&path).expect("create test file");
        f.write_all(content.as_bytes()).expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = validate_vlx_file(path_str);

        assert!(result.is_ok());
        assert_eq!(result.expect("content"), content);
    }

    #[test]
    fn test_extension_validation_before_io() {
        let result = validate_vlx_file("nonexistent.txt");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::InvalidExtension);
    }

    #[test]
    fn test_integration_with_file_loader() {
        let dir = setup_temp_dir();
        let path = dir.join("integration.vlx");
        let content = "let a = 1\nlet b = 2\nwrite a + b";
        let mut f = File::create(&path).expect("create test file");
        f.write_all(content.as_bytes()).expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let validated = validate_vlx_file(path_str).expect("validation should pass");
        let loaded = FileLoader::load_file(path_str).expect("loader should pass");

        assert_eq!(validated, loaded);
        assert_eq!(validated, content);
    }

    #[test]
    fn test_deterministic_fail_fast() {
        let result1 = validate_vlx_file("");
        let result2 = validate_vlx_file("");
        assert_eq!(result1.is_err(), result2.is_err());
        if let (Err(e1), Err(e2)) = (result1, result2) {
            assert_eq!(e1, e2);
        }
    }

    #[test]
    fn test_veltrix_validation_error_display() {
        let err = VeltrixValidationError::InvalidExtension;
        let s = format!("{}", err);
        assert!(s.contains(".vlx"));
    }
}

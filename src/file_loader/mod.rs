//! # File Loader Module
//!
//! Safely reads `.vlx` source files from disk and returns their content as UTF-8 strings.
//! Designed for the Veltrix compiler pipeline: lexer → parser → semantic analyzer → interpreter.
//!
//! ## Usage
//!
//! ```ignore
//! let content = FileLoader::load_file("example.vlx")?;
//! println!("File content:\n{}", content);
//! ```

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

/// Maximum allowed file size in bytes (1 MiB).
/// Prevents loading extremely large files that could exhaust memory.
const MAX_FILE_SIZE: u64 = 1_048_576;

/// Veltrix file loader error variants.
/// Error messages avoid exposing internal paths or file descriptors for security.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VeltrixError {
    /// The file does not exist or cannot be accessed.
    FileNotFound,
    /// The file does not have the required `.vlx` extension.
    InvalidExtension,
    /// The file contains invalid UTF-8 byte sequences.
    InvalidUTF8,
    /// The file is empty (zero bytes).
    EmptyFile,
    /// The file exceeds the maximum allowed size.
    FileTooLarge,
}

impl fmt::Display for VeltrixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VeltrixError::FileNotFound => write!(f, "File not found"),
            VeltrixError::InvalidExtension => {
                write!(f, "Invalid file extension: expected .vlx")
            }
            VeltrixError::InvalidUTF8 => write!(f, "File contains invalid UTF-8"),
            VeltrixError::EmptyFile => write!(f, "File is empty"),
            VeltrixError::FileTooLarge => write!(f, "File exceeds maximum allowed size"),
        }
    }
}

impl Error for VeltrixError {}

/// File loader for Veltrix source files.
pub struct FileLoader;

impl FileLoader {
    /// Loads a `.vlx` source file from disk and returns its content as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns `VeltrixError::InvalidExtension` if the file does not have the `.vlx` extension.
    /// Returns `VeltrixError::FileNotFound` if the file does not exist or cannot be accessed.
    /// Returns `VeltrixError::EmptyFile` if the file is empty.
    /// Returns `VeltrixError::FileTooLarge` if the file exceeds the maximum allowed size.
    /// Returns `VeltrixError::InvalidUTF8` if the file contains invalid UTF-8 byte sequences.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let content = FileLoader::load_file("example.vlx")?;
    /// println!("File content:\n{}", content);
    /// ```
    pub fn load_file(path: &str) -> Result<String, VeltrixError> {
        // 1. Validate extension first (fail-fast before I/O), case-insensitive
        let path_obj = Path::new(path);
        let ext = path_obj
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !ext.eq_ignore_ascii_case("vlx") {
            return Err(VeltrixError::InvalidExtension);
        }

        // 2. Check file metadata (existence and size)
        let metadata = fs::metadata(path).map_err(|_| VeltrixError::FileNotFound)?;

        if metadata.len() == 0 {
            return Err(VeltrixError::EmptyFile);
        }

        if metadata.len() > MAX_FILE_SIZE {
            return Err(VeltrixError::FileTooLarge);
        }

        // 3. Read and validate UTF-8
        let content = fs::read_to_string(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                VeltrixError::FileNotFound
            } else if e.kind() == std::io::ErrorKind::InvalidData {
                VeltrixError::InvalidUTF8
            } else {
                VeltrixError::FileNotFound
            }
        })?;

        // 4. Reject whitespace-only content
        if content.trim().is_empty() {
            return Err(VeltrixError::EmptyFile);
        }

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("veltrix_file_loader_tests")
    }

    fn setup_temp_dir() -> std::path::PathBuf {
        let dir = temp_dir();
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_valid_vlx_file() {
        let dir = setup_temp_dir();
        let path = dir.join("example.vlx");
        let content = "let x = 42\nwrite x";
        let mut f = File::create(&path).expect("create test file");
        f.write_all(content.as_bytes()).expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        assert_eq!(result.expect("content"), content);
    }

    #[test]
    fn test_nonexistent_file() {
        let dir = setup_temp_dir();
        let path = dir.join("does_not_exist.vlx");
        let path_str = path.to_str().expect("path to str");

        let result = FileLoader::load_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::FileNotFound);
    }

    #[test]
    fn test_wrong_file_extension() {
        let dir = setup_temp_dir();
        let path = dir.join("script.txt");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"let x = 1").expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::InvalidExtension);
    }

    #[test]
    fn test_extension_rs_rejected() {
        let result = FileLoader::load_file("main.rs");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::InvalidExtension);
    }

    #[test]
    fn test_extension_vlx_uppercase_accepted() {
        // Case-insensitive: .VLX accepted
        let dir = setup_temp_dir();
        let path = dir.join("script.VLX");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"let x = 1").expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_ok());
        assert_eq!(result.expect("content"), "let x = 1");
    }

    #[test]
    fn test_empty_file() {
        let dir = setup_temp_dir();
        let path = dir.join("empty.vlx");
        File::create(&path).expect("create empty file");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::EmptyFile);
    }

    #[test]
    fn test_whitespace_only_file() {
        let dir = setup_temp_dir();
        let path = dir.join("whitespace.vlx");
        let mut f = File::create(&path).expect("create test file");
        f.write_all(b"   \n\t  \n ").expect("write whitespace");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::EmptyFile);
    }

    #[test]
    fn test_invalid_utf8_content() {
        let dir = setup_temp_dir();
        let path = dir.join("bad_utf8.vlx");
        let mut f = File::create(&path).expect("create test file");
        // Invalid UTF-8: 0xFF 0xFE (not a valid UTF-8 sequence)
        f.write_all(&[0xFF, 0xFE, 0x00]).expect("write invalid utf8");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::InvalidUTF8);
    }

    #[test]
    fn test_file_too_large() {
        let dir = setup_temp_dir();
        let path = dir.join("large.vlx");
        let mut f = File::create(&path).expect("create test file");
        // Write slightly more than 1 MiB
        let padding = vec![b'a'; 1_048_577];
        f.write_all(&padding).expect("write large content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::FileTooLarge);
    }

    #[test]
    fn test_file_at_max_size_boundary() {
        let dir = setup_temp_dir();
        let path = dir.join("max.vlx");
        let mut f = File::create(&path).expect("create test file");
        let padding = vec![b'a'; MAX_FILE_SIZE as usize];
        f.write_all(&padding).expect("write content");
        f.sync_all().expect("sync");

        let path_str = path.to_str().expect("path to str");
        let result = FileLoader::load_file(path_str);

        assert!(result.is_ok());
        assert_eq!(result.expect("content").len(), MAX_FILE_SIZE as usize);
    }

    #[test]
    fn test_extension_validation_before_io() {
        // Invalid extension should fail without touching filesystem
        let result = FileLoader::load_file("nonexistent.txt");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixError::InvalidExtension);
    }
}

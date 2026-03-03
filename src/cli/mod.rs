//! # Veltrix CLI Module
//!
//! Command-line interface for the Veltrix V1 compiler.
//! Provides `veltrix run <file.vlx>` with optional flags: `--repl`, `--debug`, `--print-ast`.

use crate::validation::{validate_vlx_file, VeltrixValidationError};
use crate::interpreter::RuntimeError;
use crate::lexer::error::LexError;
use crate::parser::error::ParserError;
use crate::semantic::SemanticError;
use std::env;
use std::process;

/// Result of parsing CLI arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    /// Path to the .vlx file to run.
    pub file_path: String,
    /// Enter REPL after running the file.
    pub repl: bool,
    /// Print debug info (tokens and AST).
    pub debug: bool,
    /// Skip execution, print AST only.
    pub print_ast: bool,
}

/// CLI-specific error variants.
#[derive(Debug)]
pub enum CliError {
    /// Invalid command or arguments.
    InvalidArgs(String),
    /// File validation or loader error (does not expose internal paths).
    ValidationError(VeltrixValidationError),
    /// Lexer error.
    LexError(LexError),
    /// Parser error.
    ParseError(ParserError),
    /// Semantic analysis error.
    SemanticError(SemanticError),
    /// Runtime/interpreter error.
    RuntimeError(RuntimeError),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::InvalidArgs(msg) => write!(f, "{}", msg),
            CliError::ValidationError(e) => write!(f, "{}", e),
            CliError::LexError(e) => write!(f, "Lexer error at line {}, column {}: {}", e.line, e.column, e.message),
            CliError::ParseError(e) => write!(f, "Parse error at line {}, column {}: {}", e.line, e.column, e.message),
            CliError::SemanticError(e) => write!(f, "Semantic error: {}", e.message),
            CliError::RuntimeError(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for CliError {}

/// Usage string displayed when arguments are invalid.
const USAGE: &str = r#"Usage: veltrix run <file.vlx> [--repl] [--debug] [--print-ast]

Options:
  --repl       Enter REPL after running the file
  --debug      Print debug info (lexer tokens and AST)
  --print-ast  Skip execution, print AST only

Example:
  veltrix run example.vlx
  veltrix run example.vlx --debug
  veltrix run example.vlx --print-ast
  veltrix run example.vlx --repl
"#;

/// Parses command-line arguments into `CliArgs`.
/// Returns `Err(CliError::InvalidArgs)` on invalid or missing arguments.
pub fn parse_args() -> Result<CliArgs, CliError> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(CliError::InvalidArgs(format!(
            "Missing command.\n{}",
            USAGE
        )));
    }

    let command = &args[1];
    if command != "run" {
        return Err(CliError::InvalidArgs(format!(
            "Unknown command '{}'. Expected 'run'.\n{}",
            command, USAGE
        )));
    }

    if args.len() < 3 {
        return Err(CliError::InvalidArgs(format!(
            "Missing file path.\n{}",
            USAGE
        )));
    }

    // First non-flag arg after "run" is the file path
    let mut file_path: Option<String> = None;
    let mut repl = false;
    let mut debug = false;
    let mut print_ast = false;

    for arg in args.iter().skip(2) {
        match arg.as_str() {
            "--repl" => repl = true,
            "--debug" => debug = true,
            "--print-ast" => print_ast = true,
            s if s.starts_with('-') => {
                return Err(CliError::InvalidArgs(format!(
                    "Unknown flag '{}'.\n{}",
                    s, USAGE
                )));
            }
            s => {
                if file_path.is_some() {
                    return Err(CliError::InvalidArgs(format!(
                        "Multiple file paths given. Expected exactly one.\n{}",
                        USAGE
                    )));
                }
                file_path = Some(s.to_string());
            }
        }
    }

    let path = file_path.ok_or_else(|| {
        CliError::InvalidArgs(format!("Missing file path.\n{}", USAGE))
    })?;

    // Validate .vlx extension (case-insensitive) before calling validation
    let path_lower = path.to_lowercase();
    if !path_lower.ends_with(".vlx") {
        return Err(CliError::InvalidArgs(
            "Invalid file extension: expected .vlx".to_string(),
        ));
    }

    Ok(CliArgs {
        file_path: path,
        repl,
        debug,
        print_ast,
    })
}

/// Validates and loads a file, then runs the pipeline via `veltrix::pipeline::run_vlx_content`.
pub fn run(args: &CliArgs) -> Result<(), CliError> {
    let content = validate_vlx_file(&args.file_path).map_err(CliError::ValidationError)?;

    crate::pipeline::run_vlx_content(
        &content,
        crate::pipeline::RunFlags {
            print_ast: args.print_ast,
            debug: args.debug,
            repl: args.repl,
        },
    )
    .map_err(|e| match e {
        crate::pipeline::PipelineError::EmptyContent => {
            CliError::ValidationError(VeltrixValidationError::EmptyFile)
        }
        crate::pipeline::PipelineError::LexerError(le) => CliError::LexError(le),
        crate::pipeline::PipelineError::ParserError(pe) => CliError::ParseError(pe),
        crate::pipeline::PipelineError::SemanticError(se) => CliError::SemanticError(se),
        crate::pipeline::PipelineError::InterpreterError(re) => CliError::RuntimeError(re),
        crate::pipeline::PipelineError::ReplIoError(msg) => {
            CliError::InvalidArgs(format!("REPL I/O error: {}", msg))
        }
    })?;

    Ok(())
}

/// Main entry point: parses args, runs, and exits with appropriate code.
pub fn main() {
    match parse_args() {
        Ok(args) => match run(&args) {
            Ok(()) => process::exit(0),
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("veltrix_cli_tests")
    }

    fn setup_temp_dir() -> std::path::PathBuf {
        let dir = temp_dir();
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Helper to parse args as if they came from the command line.
    /// We cannot easily override env::args() in tests, so we test parse_args_logic directly
    /// by passing a slice.
    fn parse_args_from(args: &[&str]) -> Result<CliArgs, CliError> {
        if args.len() < 2 {
            return Err(CliError::InvalidArgs("Missing command.".to_string()));
        }
        if args[1] != "run" {
            return Err(CliError::InvalidArgs("Unknown command.".to_string()));
        }
        if args.len() < 3 {
            return Err(CliError::InvalidArgs("Missing file path.".to_string()));
        }

        let mut file_path: Option<String> = None;
        let mut repl = false;
        let mut debug = false;
        let mut print_ast = false;

        for arg in args.iter().skip(2) {
            match *arg {
                "--repl" => repl = true,
                "--debug" => debug = true,
                "--print-ast" => print_ast = true,
                s if s.starts_with('-') => {
                    return Err(CliError::InvalidArgs(format!("Unknown flag '{}'", s)));
                }
                s => {
                    if file_path.is_some() {
                        return Err(CliError::InvalidArgs("Multiple file paths given.".to_string()));
                    }
                    file_path = Some(s.to_string());
                }
            }
        }

        let path = file_path.ok_or_else(|| CliError::InvalidArgs("Missing file path.".to_string()))?;

        if !path.to_lowercase().ends_with(".vlx") {
            return Err(CliError::InvalidArgs(
                "Invalid file extension: expected .vlx".to_string(),
            ));
        }

        Ok(CliArgs {
            file_path: path,
            repl,
            debug,
            print_ast,
        })
    }

    #[test]
    fn test_parse_args_valid_file_no_flags() {
        let result = parse_args_from(&["veltrix", "run", "example.vlx"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.file_path, "example.vlx");
        assert!(!args.repl);
        assert!(!args.debug);
        assert!(!args.print_ast);
    }

    #[test]
    fn test_parse_args_valid_file_with_print_ast() {
        let result = parse_args_from(&["veltrix", "run", "example.vlx", "--print-ast"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.file_path, "example.vlx");
        assert!(args.print_ast);
    }

    #[test]
    fn test_parse_args_valid_file_with_debug() {
        let result = parse_args_from(&["veltrix", "run", "example.vlx", "--debug"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.file_path, "example.vlx");
        assert!(args.debug);
    }

    #[test]
    fn test_parse_args_valid_file_with_repl() {
        let result = parse_args_from(&["veltrix", "run", "example.vlx", "--repl"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.file_path, "example.vlx");
        assert!(args.repl);
    }

    #[test]
    fn test_parse_args_combined_flags() {
        let result = parse_args_from(&["veltrix", "run", "a.vlx", "--debug", "--print-ast", "--repl"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.file_path, "a.vlx");
        assert!(args.debug);
        assert!(args.print_ast);
        assert!(args.repl);
    }

    #[test]
    fn test_parse_args_missing_file_path() {
        let result = parse_args_from(&["veltrix", "run"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_args_invalid_flag() {
        let result = parse_args_from(&["veltrix", "run", "a.vlx", "--unknown"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::InvalidArgs(_)));
    }

    #[test]
    fn test_parse_args_invalid_extension() {
        let result = parse_args_from(&["veltrix", "run", "script.txt"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::InvalidArgs(_)));
    }

    #[test]
    fn test_parse_args_invalid_command() {
        let result = parse_args_from(&["veltrix", "build", "a.vlx"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_args_multiple_files() {
        let result = parse_args_from(&["veltrix", "run", "a.vlx", "b.vlx"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_valid_file_no_flags() {
        let dir = setup_temp_dir();
        let path = dir.join("test.vlx");
        let content = "write 42";
        let mut f = File::create(&path).expect("create test file");
        f.write_all(content.as_bytes()).expect("write");
        f.sync_all().expect("sync");

        let args = CliArgs {
            file_path: path.to_str().expect("path").to_string(),
            repl: false,
            debug: false,
            print_ast: false,
        };

        let result = run(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_valid_file_print_ast() {
        let dir = setup_temp_dir();
        let path = dir.join("ast.vlx");
        let content = "let x = 5";
        let mut f = File::create(&path).expect("create");
        f.write_all(content.as_bytes()).expect("write");
        f.sync_all().expect("sync");

        let args = CliArgs {
            file_path: path.to_str().expect("path").to_string(),
            repl: false,
            debug: false,
            print_ast: true,
        };

        let result = run(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_valid_file_debug() {
        let dir = setup_temp_dir();
        let path = dir.join("debug.vlx");
        let content = "let x = 1";
        let mut f = File::create(&path).expect("create");
        f.write_all(content.as_bytes()).expect("write");
        f.sync_all().expect("sync");

        let args = CliArgs {
            file_path: path.to_str().expect("path").to_string(),
            repl: false,
            debug: true,
            print_ast: false,
        };

        let result = run(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_missing_file() {
        let dir = setup_temp_dir();
        let path = dir.join("nonexistent.vlx");

        let args = CliArgs {
            file_path: path.to_str().expect("path").to_string(),
            repl: false,
            debug: false,
            print_ast: false,
        };

        let result = run(&args);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CliError::ValidationError(VeltrixValidationError::FileNotFound)
        ));
    }

    #[test]
    fn test_run_invalid_extension_propagates_from_validation() {
        let dir = setup_temp_dir();
        let path = dir.join("script.txt");
        let mut f = File::create(&path).expect("create");
        f.write_all(b"let x = 1").expect("write");
        f.sync_all().expect("sync");

        let result = validate_vlx_file(path.to_str().expect("path"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), VeltrixValidationError::InvalidExtension);
    }

    #[test]
    fn test_flags_deterministic() {
        // Flags in different order should yield same result
        let r1 = parse_args_from(&["veltrix", "run", "a.vlx", "--debug", "--repl"]);
        let r2 = parse_args_from(&["veltrix", "run", "a.vlx", "--repl", "--debug"]);
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        let a1 = r1.unwrap();
        let a2 = r2.unwrap();
        assert_eq!(a1.file_path, a2.file_path);
        assert_eq!(a1.debug, a2.debug);
        assert_eq!(a1.repl, a2.repl);
    }
}

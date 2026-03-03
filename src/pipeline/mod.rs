//! # Pipeline Module
//!
//! Processes `.vlx` file content through the Veltrix compiler stages:
//!
//! **FileLoader** (optional, when loading from path) → **Lexer** → **Parser** →
//! **Semantic Analyzer** → **Interpreter**
//!
//! ## Stages
//!
//! 1. **Lexer**: Tokenizes source into tokens
//! 2. **Parser**: Builds an AST from tokens
//! 3. **Semantic Analyzer**: Validates scope, symbols, and type-correctness
//! 4. **Interpreter**: Executes the validated AST
//!
//! ## Usage
//!
//! ```ignore
//! use veltrix::file_loader::FileLoader;
//! use veltrix::pipeline::{run_vlx_content, RunFlags};
//!
//! fn main() -> Result<(), veltrix::pipeline::PipelineError> {
//!     let content = FileLoader::load_file("example.vlx")?;
//!     run_vlx_content(&content, RunFlags {
//!         print_ast: false,
//!         debug: true,
//!         repl: false,
//!     })?;
//!     Ok(())
//! }
//! ```

use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::lexer::error::LexError;
use crate::lexer::Lexer;
use crate::native::NativeFunctionRegistry;
use crate::parser::error::ParserError;
use crate::parser::Parser;
use crate::repl::Repl;
use crate::semantic::{SemanticAnalyzer, SemanticError};
use std::sync::Arc;
use std::error::Error;
use std::fmt;
use std::io;

/// Flags controlling pipeline behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunFlags {
    /// Skip execution and print AST only.
    pub print_ast: bool,
    /// Print lexer tokens and AST for debugging.
    pub debug: bool,
    /// Start REPL after execution with current interpreter state.
    pub repl: bool,
}

impl Default for RunFlags {
    fn default() -> Self {
        Self {
            print_ast: false,
            debug: false,
            repl: false,
        }
    }
}

/// Result of successful pipeline execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    /// The last value produced by execution (or Nil if none).
    pub last_value: Value,
}

impl ExecutionResult {
    /// Creates a new execution result.
    pub fn new(last_value: Value) -> Self {
        Self { last_value }
    }
}

/// Pipeline error wrapping errors from each stage.
/// User-facing messages only; no internal structures exposed.
#[derive(Debug)]
pub enum PipelineError {
    /// Content is empty or whitespace-only.
    EmptyContent,
    /// Lexer stage failed.
    LexerError(LexError),
    /// Parser stage failed.
    ParserError(ParserError),
    /// Semantic analysis failed.
    SemanticError(SemanticError),
    /// Interpreter/runtime error.
    InterpreterError(RuntimeError),
    /// REPL I/O error (when --repl is set).
    ReplIoError(String),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::EmptyContent => write!(f, "Input is empty"),
            PipelineError::LexerError(e) => {
                write!(f, "Lexer error at line {}, column {}: {}", e.line, e.column, e.message)
            }
            PipelineError::ParserError(e) => {
                write!(f, "Parser error at line {}, column {}: {}", e.line, e.column, e.message)
            }
            PipelineError::SemanticError(e) => {
                write!(f, "Semantic error at line {}, column {}: {}", e.line, e.column, e.message)
            }
            PipelineError::InterpreterError(e) => write!(f, "{}", e),
            PipelineError::ReplIoError(msg) => write!(f, "REPL I/O error: {}", msg),
        }
    }
}

impl Error for PipelineError {}

/// Runs `.vlx` content through the compiler pipeline.
///
/// Stages: Lexer → Parser → Semantic Analyzer → Interpreter
///
/// # Flags
///
/// - **print_ast**: Skips execution and prints the AST. Returns `Ok` with `ExecutionResult::new(Value::Nil)`.
/// - **debug**: Prints lexer tokens and AST before execution.
/// - **repl**: After successful execution, starts the REPL with the current interpreter state.
///
/// # Errors
///
/// Returns `PipelineError` on:
/// - Empty content
/// - Lexer, parser, semantic, or interpreter errors
/// - REPL I/O error (when repl flag is set)
///
/// # Example
///
/// ```ignore
/// let content = "let x = 42\nwrite x";
/// let flags = RunFlags { print_ast: false, debug: false, repl: false };
/// let result = run_vlx_content(content, flags)?;
/// ```
pub fn run_vlx_content(content: &str, flags: RunFlags) -> Result<ExecutionResult, PipelineError> {
    // Stage 0: Reject empty content
    if content.trim().is_empty() {
        return Err(PipelineError::EmptyContent);
    }

    // Stage 1: Lexer
    let mut lexer = Lexer::new(content);
    let tokens = lexer.tokenize().map_err(PipelineError::LexerError)?;

    if flags.debug {
        println!("--- Lexer Tokens ---");
        for (i, t) in tokens.iter().enumerate() {
            println!("  [{}] {:?}", i, t);
        }
        println!();
    }

    // Stage 2: Parser
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().map_err(PipelineError::ParserError)?;

    if flags.debug {
        println!("--- AST ---");
        println!("{:#?}", program);
        println!();
    }

    if flags.print_ast {
        println!("{:#?}", program);
        return Ok(ExecutionResult::new(Value::Nil));
    }

    // Stage 3: Semantic Analyzer (registry injected explicitly; empty until built-ins added)
    let native_registry = Arc::new(NativeFunctionRegistry::empty());
    let mut analyzer = SemanticAnalyzer::with_native_registry(native_registry);
    analyzer.analyze_program(&program).map_err(PipelineError::SemanticError)?;

    // Stage 4: Interpreter
    let mut interpreter = Interpreter::new();
    let last_value = interpreter
        .execute_program(&program.statements)
        .map_err(PipelineError::InterpreterError)?;

    if flags.repl {
        let mut repl = Repl::with_interpreter(interpreter);
        repl.start().map_err(|e: io::Error| {
            PipelineError::ReplIoError(e.to_string())
        })?;
    }

    Ok(ExecutionResult::new(last_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Value;

    fn run_content(content: &str, print_ast: bool, debug: bool, repl: bool) -> Result<ExecutionResult, PipelineError> {
        run_vlx_content(
            content,
            RunFlags {
                print_ast,
                debug,
                repl,
            },
        )
    }

    // =========================================================================
    // MINIMAL VALID CONTENT
    // =========================================================================

    #[test]
    fn test_minimal_valid_content() {
        let result = run_content("let x = 42", false, false, false);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Nil);
    }

    #[test]
    fn test_valid_content_with_expression() {
        let result = run_content("let x = 5\nx + 3", false, false, false);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Integer(8));
    }

    #[test]
    fn test_write_statement_executes() {
        let result = run_content("write 42", false, false, false);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Nil);
    }

    // =========================================================================
    // EMPTY CONTENT
    // =========================================================================

    #[test]
    fn test_empty_content_returns_error() {
        let result = run_content("", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::EmptyContent));
    }

    #[test]
    fn test_whitespace_only_returns_error() {
        let result = run_content("   \n  \n  ", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::EmptyContent));
    }

    #[test]
    fn test_newlines_only_returns_error() {
        let result = run_content("\n\n\n", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::EmptyContent));
    }

    #[test]
    fn test_tabs_only_returns_error() {
        let result = run_content("\t\t\t", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::EmptyContent));
    }

    // =========================================================================
    // INVALID SYNTAX - PARSER ERROR
    // =========================================================================

    #[test]
    fn test_invalid_syntax_parser_error() {
        let result = run_content("let x = ", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::ParserError(_)));
    }

    #[test]
    fn test_standalone_expression_semantic_error() {
        let result = run_content("x + 5", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::SemanticError(_)));
    }

    #[test]
    fn test_unmatched_paren_parser_error() {
        let result = run_content("let x = (1 + 2", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::ParserError(_)));
    }

    // =========================================================================
    // LEXER ERROR
    // =========================================================================

    #[test]
    fn test_unexpected_character_lexer_error() {
        let result = run_content("let ^ = 5", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::LexerError(_)));
    }

    #[test]
    fn test_unterminated_string_lexer_error() {
        let result = run_content("let s = \"hello\nworld\"", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::LexerError(_)));
    }

    // =========================================================================
    // SEMANTIC ERROR
    // =========================================================================

    #[test]
    fn test_undeclared_identifier_semantic_error() {
        let result = run_content("let x = y", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::SemanticError(_)));
    }

    #[test]
    fn test_duplicate_variable_semantic_error() {
        let result = run_content("let x = 1\nlet x = 2", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::SemanticError(_)));
    }

    // =========================================================================
    // INTERPRETER ERROR
    // =========================================================================

    #[test]
    fn test_division_by_zero_interpreter_error() {
        // Semantic analysis passes (x and y declared), but runtime fails
        let result = run_content("let x = 10\nlet y = 0\nlet z = x / y", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::InterpreterError(_)));
    }

    #[test]
    fn test_type_error_interpreter_error() {
        let result = run_content("let x = 5\nlet y = \"hello\"\nlet z = x + y", false, false, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PipelineError::InterpreterError(_)));
    }

    // =========================================================================
    // PRINT_AST FLAG
    // =========================================================================

    #[test]
    fn test_print_ast_skips_execution() {
        let result = run_content("let x = 42\nwrite x", true, false, false);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Nil);
    }

    #[test]
    fn test_print_ast_valid_syntax_no_execution() {
        let result = run_content("write 999", true, false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_ast_with_debug() {
        let result = run_content("let x = 1", true, true, false);
        assert!(result.is_ok());
    }

    // =========================================================================
    // DEBUG FLAG
    // =========================================================================

    #[test]
    fn test_debug_prints_tokens_and_ast() {
        let result = run_content("let x = 1", false, true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_debug_combined_with_execution() {
        let result = run_content("let x = 5\nx", false, true, false);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Integer(5));
    }

    // =========================================================================
    // FLAGS DETERMINISTIC
    // =========================================================================

    #[test]
    fn test_flags_deterministic_order() {
        let content = "let x = 1";
        let r1 = run_content(content, false, true, false);
        let r2 = run_content(content, false, true, false);
        assert_eq!(r1.is_ok(), r2.is_ok());
        if let (Ok(a), Ok(b)) = (r1, r2) {
            assert_eq!(a.last_value, b.last_value);
        }
    }

    #[test]
    fn test_all_flags_false_executes() {
        let result = run_content("let x = 10", false, false, false);
        assert!(result.is_ok());
    }

    // =========================================================================
    // INTEGRATION - FILE LOADER SIMULATION
    // =========================================================================

    #[test]
    fn test_integration_content_from_file_loader() {
        // Simulates: content = FileLoader::load_file("example.vlx")?;
        let content = "let a = 1\nlet b = 2\nwrite a + b";
        let result = run_vlx_content(
            content,
            RunFlags {
                print_ast: false,
                debug: false,
                repl: false,
            },
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Nil);
    }

    #[test]
    fn test_function_declaration_and_call() {
        let content = r#"
function add(a, b):
    return a + b
add(3, 4)
"#;
        let result = run_content(content.trim(), false, false, false);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.last_value, Value::Integer(7));
    }

    #[test]
    fn test_run_flags_default() {
        let flags = RunFlags::default();
        assert!(!flags.print_ast);
        assert!(!flags.debug);
        assert!(!flags.repl);
    }

    #[test]
    fn test_execution_result_new() {
        let res = ExecutionResult::new(Value::Integer(42));
        assert_eq!(res.last_value, Value::Integer(42));
    }

    #[test]
    fn test_pipeline_error_empty_content_display() {
        let err = PipelineError::EmptyContent;
        let s = format!("{}", err);
        assert!(s.contains("empty"));
    }

    #[test]
    fn test_pipeline_error_lexer_display() {
        let inner = LexError {
            message: "bad token".to_string(),
            line: 1,
            column: 5,
        };
        let err = PipelineError::LexerError(inner);
        let s = format!("{}", err);
        assert!(s.contains("Lexer"));
        assert!(s.contains("bad token"));
    }
}

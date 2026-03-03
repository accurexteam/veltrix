use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ParserError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parser error at line {}, column {}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for ParserError {}

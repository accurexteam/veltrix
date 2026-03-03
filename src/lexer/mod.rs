pub mod error;
pub mod token;

use error::LexError;
use token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    current: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
    at_line_start: bool,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            source: input.chars().collect(),
            current: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
            at_line_start: true,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            if self.at_line_start {
                let mut spaces = 0;
                
                // Count leading spaces
                while let Some(c) = self.peek() {
                    if c == ' ' {
                        spaces += 1;
                        self.advance();
                    } else if c == '\t' {
                        let col = self.column;
                        return Err(self.error_at("Tabs are strictly forbidden. Use spaces for indentation.", self.line, col));
                    } else {
                        break;
                    }
                }

                if let Some(c) = self.peek() {
                    if c == '\n' || c == '\r' || c == '#' {
                        if c == '#' {
                            self.skip_comment();
                        } else if c == '\r' {
                            self.advance();
                            if self.peek() == Some('\n') {
                                self.advance();
                            }
                            self.line += 1;
                            self.column = 1;
                            self.at_line_start = true;
                        } else if c == '\n' {
                            self.advance();
                            self.line += 1;
                            self.column = 1;
                            self.at_line_start = true;
                        }
                        continue;
                    }
                }

                if !self.is_at_end() {
                    let mut stack_top = 0;
                    if let Some(&top) = self.indent_stack.last() {
                        stack_top = top;
                    }

                    if spaces > stack_top {
                        if tokens.is_empty() && stack_top == 0 {
                            return Err(self.error_at("File starting with indentation is not allowed", self.line, 1));
                        }

                        self.indent_stack.push(spaces);
                        tokens.push(Token {
                            kind: TokenKind::Indent,
                            line: self.line,
                            column: self.column,
                        });
                    } else if spaces < stack_top {
                        let mut found_match = false;
                        while let Some(&current_top) = self.indent_stack.last() {
                            if current_top > spaces {
                                self.indent_stack.pop();
                                tokens.push(Token {
                                    kind: TokenKind::Dedent,
                                    line: self.line,
                                    column: self.column,
                                });
                            } else if current_top == spaces {
                                found_match = true;
                                break;
                            } else {
                                break;
                            }
                        }
                        if !found_match {
                            return Err(self.error_at("Inconsistent indentation", self.line, self.column));
                        }
                    }
                    self.at_line_start = false;
                }
            }

            if self.is_at_end() {
                break;
            }

            if let Some(c) = self.advance() {
                match c {
                    ' ' | '\r' => {
                        // Ignore normal spaces / carriage returns inside line
                    }
                    '\t' => {
                        let col = self.column - 1;
                        return Err(self.error_at("Tabs are strictly forbidden. Use spaces for indentation.", self.line, col));
                    }
                    '\n' => {
                        tokens.push(self.make_token(TokenKind::Newline));
                        self.line += 1;
                        self.column = 1;
                        self.at_line_start = true;
                    }
                    '#' => {
                        self.skip_comment();
                    }
                    '=' => {
                        let start_col = self.column - 1;
                        if self.match_next('=') {
                            tokens.push(Token { kind: TokenKind::Equal, line: self.line, column: start_col });
                        } else {
                            tokens.push(Token { kind: TokenKind::Assign, line: self.line, column: start_col });
                        }
                    }
                    '!' => {
                        let start_col = self.column - 1;
                        if self.match_next('=') {
                            tokens.push(Token { kind: TokenKind::NotEqual, line: self.line, column: start_col });
                        } else {
                            return Err(self.error_at("Unexpected character '!'", self.line, start_col));
                        }
                    }
                    '>' => {
                        let start_col = self.column - 1;
                        if self.match_next('=') {
                            tokens.push(Token { kind: TokenKind::GreaterEqual, line: self.line, column: start_col });
                        } else {
                            tokens.push(Token { kind: TokenKind::GreaterThan, line: self.line, column: start_col });
                        }
                    }
                    '<' => {
                        let start_col = self.column - 1;
                        if self.match_next('=') {
                            tokens.push(Token { kind: TokenKind::LessEqual, line: self.line, column: start_col });
                        } else {
                            tokens.push(Token { kind: TokenKind::LessThan, line: self.line, column: start_col });
                        }
                    }
                    '+' => tokens.push(self.make_token(TokenKind::Plus)),
                    '-' => tokens.push(self.make_token(TokenKind::Minus)),
                    '*' => tokens.push(self.make_token(TokenKind::Multiply)),
                    '/' => tokens.push(self.make_token(TokenKind::Divide)),
                    '%' => tokens.push(self.make_token(TokenKind::Modulo)),
                    '(' => tokens.push(self.make_token(TokenKind::LParen)),
                    ')' => tokens.push(self.make_token(TokenKind::RParen)),
                    '[' => tokens.push(self.make_token(TokenKind::LBracket)),
                    ']' => tokens.push(self.make_token(TokenKind::RBracket)),
                    ',' => tokens.push(self.make_token(TokenKind::Comma)),
                    ':' => tokens.push(self.make_token(TokenKind::Colon)),
                    '"' => {
                        tokens.push(self.string()?);
                    }
                    _ => {
                        if c.is_ascii_alphabetic() || c == '_' {
                            tokens.push(self.identifier()?);
                        } else if c.is_ascii_digit() {
                            tokens.push(self.number()?);
                        } else {
                            let col = self.column - 1;
                            return Err(self.error_at(&format!("Unexpected character '{}'", c), self.line, col));
                        }
                    }
                }
            }
        }

        if !self.at_line_start {
            tokens.push(Token {
                kind: TokenKind::Newline,
                line: self.line,
                column: self.column,
            });
        }

        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            tokens.push(Token {
                kind: TokenKind::Dedent,
                line: self.line,
                column: self.column,
            });
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            line: self.line,
            // Adjust column for EOF
            column: self.column,
        });

        Ok(tokens)
    }

    fn skip_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn match_next(&mut self, expected: char) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        self.column += 1;
        true
    }

    fn identifier(&mut self) -> Result<Token, LexError> {
        let start = self.current - 1;
        let start_column = self.column - 1;
        
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let text: String = self.source[start..self.current].iter().collect();

        // Check keyword
        let kind = match text.as_str() {
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "while" => TokenKind::While,
            "function" => TokenKind::Func,
            "return" => TokenKind::Return,
            "write" => TokenKind::Write,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => TokenKind::Identifier(text),
        };

        Ok(Token {
            kind,
            line: self.line,
            column: start_column,
        })
    }

    fn number(&mut self) -> Result<Token, LexError> {
        let start = self.current - 1;
        let start_column = self.column - 1;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c.is_ascii_alphabetic() || c == '_' {
                return Err(self.error_at("Invalid number literal", self.line, start_column));
            } else {
                break;
            }
        }
        
        // Ensure not a float
        if let Some(c) = self.peek() {
            if c == '.' {
                return Err(self.error_at("Float numbers are not supported in v0.1", self.line, start_column));
            }
        }

        let text: String = self.source[start..self.current].iter().collect();
        let value = text.parse::<i64>().map_err(|_| self.error_at("Integer overflow: number literal is too large to fit in i64", self.line, start_column))?;

        Ok(Token {
            kind: TokenKind::Number(value),
            line: self.line,
            column: start_column,
        })
    }

    fn string(&mut self) -> Result<Token, LexError> {
        let start = self.current;
        let start_column = self.column - 1;

        while let Some(c) = self.peek() {
            if c == '"' {
                break;
            } else if c == '\n' {
                return Err(self.error_at("Unterminated string literal", self.line, start_column));
            }
            self.advance();
        }

        if self.is_at_end() {
            return Err(self.error_at("Unterminated string literal", self.line, start_column));
        }

        // Consume closing '"'
        self.advance();

        let value: String = self.source[start..self.current - 1].iter().collect();

        Ok(Token {
            kind: TokenKind::String(value),
            line: self.line,
            column: start_column,
        })
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> Option<char> {
        if self.is_at_end() {
            None
        } else {
            let c = self.source[self.current];
            self.current += 1;
            self.column += 1;
            Some(c)
        }
    }

    fn peek(&self) -> Option<char> {
        if self.is_at_end() {
            None
        } else {
            Some(self.source[self.current])
        }
    }

    fn make_token(&self, kind: TokenKind) -> Token {
        Token {
            kind,
            line: self.line,
            column: if self.column > 1 { self.column - 1 } else { 1 },
        }
    }

    fn error_at(&self, message: &str, line: usize, column: usize) -> LexError {
        LexError {
            message: message.to_string(),
            line,
            column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::{Token, TokenKind};

    fn collect_tokens(source: &str) -> Result<Vec<Token>, LexError> {
        let mut lexer = Lexer::new(source);
        lexer.tokenize()
    }

    fn check_kinds(tokens: &[Token], expected: &[TokenKind]) {
        assert_eq!(
            tokens.len(),
            expected.len(),
            "Token count mismatch. Expected {}, got {}",
            expected.len(),
            tokens.len()
        );
        for (i, (token, kind)) in tokens.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                token.kind, *kind,
                "Token kind mismatch at index {}. Expected {:?}, got {:?}",
                i, kind, token.kind
            );
        }
    }

    // ---------------------------------------------------------
    // 1. Basic Tokenization
    // ---------------------------------------------------------

    #[test]
    fn test_basic_identifier() -> Result<(), String> {
        let source = "let x = 10";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Let,
                TokenKind::Identifier("x".to_string()),
                TokenKind::Assign,
                TokenKind::Number(10),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_keywords() -> Result<(), String> {
        let source = "if else for in while function return write true false and or not";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::If,
                TokenKind::Else,
                TokenKind::For,
                TokenKind::In,
                TokenKind::While,
                TokenKind::Func,
                TokenKind::Return,
                TokenKind::Write,
                TokenKind::True,
                TokenKind::False,
                TokenKind::And,
                TokenKind::Or,
                TokenKind::Not,
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_integers() -> Result<(), String> {
        let source = "42 0 999999";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Number(42),
                TokenKind::Number(0),
                TokenKind::Number(999999),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_strings() -> Result<(), String> {
        let source = "\"hello\" \"world 123 !@#\"";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::String("hello".to_string()),
                TokenKind::String("world 123 !@#".to_string()),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_operators() -> Result<(), String> {
        let source = "= + - * / % > < == != >= <= ( ) [ ] ,";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Assign,
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Multiply,
                TokenKind::Divide,
                TokenKind::Modulo,
                TokenKind::GreaterThan,
                TokenKind::LessThan,
                TokenKind::Equal,
                TokenKind::NotEqual,
                TokenKind::GreaterEqual,
                TokenKind::LessEqual,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::Comma,
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_newlines_and_comments() -> Result<(), String> {
        let source = "let a = 1\n# comment here\nlet b = 2";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Let,
                TokenKind::Identifier("a".to_string()),
                TokenKind::Assign,
                TokenKind::Number(1),
                TokenKind::Newline,
                TokenKind::Let,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Assign,
                TokenKind::Number(2),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    // ---------------------------------------------------------
    // 2. Indentation System
    // ---------------------------------------------------------

    #[test]
    fn test_single_indent() -> Result<(), String> {
        let source = "if true\n    write 1";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::If,
                TokenKind::True,
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Write,
                TokenKind::Number(1),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_nested_indentation() -> Result<(), String> {
        let source = "a\n  b\n    c\n  d\ne";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Identifier("a".to_string()),
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("c".to_string()),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Identifier("d".to_string()),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Identifier("e".to_string()),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_multiple_dedents_at_eof() -> Result<(), String> {
        let source = "a\n  b\n    c";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Identifier("a".to_string()),
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("c".to_string()),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Dedent,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_empty_lines_inside_blocks() -> Result<(), String> {
        let source = "a\n  b\n\n  c\n";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Identifier("a".to_string()),
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Newline,
                TokenKind::Identifier("c".to_string()),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_indent_after_newline_only() -> Result<(), String> {
        // Checking that a file starting with indent errors
        let source = "  a";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "File starting with indentation is not allowed");
        Ok(())
    }

    #[test]
    fn test_mixed_indentation_error() -> Result<(), String> {
        let source = "a\n    b\n  c";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Inconsistent indentation");
        assert_eq!(err.line, 3);
        assert_eq!(err.column, 3);
        Ok(())
    }

    // ---------------------------------------------------------
    // 3. Error Handling
    // ---------------------------------------------------------

    #[test]
    fn test_unexpected_character() -> Result<(), String> {
        let source = "let ^ = 5";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Unexpected character '^'");
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 5);
        Ok(())
    }

    #[test]
    fn test_unterminated_string_error() -> Result<(), String> {
        let source = "let s = \"hello\nworld\"";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Unterminated string literal");
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 9);
        Ok(())
    }

    #[test]
    fn test_invalid_numeric_format_float() -> Result<(), String> {
        let source = "let x = 3.14";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Float numbers are not supported in v0.1");
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 9);
        Ok(())
    }

    #[test]
    fn test_invalid_numeric_format_alpha() -> Result<(), String> {
        let source = "let x = 123abc";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Invalid number literal");
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 9);
        Ok(())
    }

    // ---------------------------------------------------------
    // 4. Edge Cases
    // ---------------------------------------------------------

    #[test]
    fn test_empty_file() -> Result<(), String> {
        let source = "";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(&tokens, &[TokenKind::Eof]);
        assert_eq!(tokens[0].line, 1);
        Ok(())
    }

    #[test]
    fn test_file_with_only_whitespace() -> Result<(), String> {
        let source = "   \n  \n    ";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(&tokens, &[TokenKind::Eof]); // since blank lines are skipped
        Ok(())
    }

    #[test]
    fn test_file_ending_without_newline() -> Result<(), String> {
        let source = "let x = 1";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        // lexer appends newline
        check_kinds(
            &tokens,
            &[
                TokenKind::Let,
                TokenKind::Identifier("x".to_string()),
                TokenKind::Assign,
                TokenKind::Number(1),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_trailing_spaces() -> Result<(), String> {
        let source = "let x = 1   ";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Let,
                TokenKind::Identifier("x".to_string()),
                TokenKind::Assign,
                TokenKind::Number(1),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    #[test]
    fn test_tabs_are_forbidden() -> Result<(), String> {
        let source = "let x = 1\n\tlet y = 2";
        let result = collect_tokens(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Tabs are strictly forbidden. Use spaces for indentation.");
        assert_eq!(err.line, 2);
        assert_eq!(err.column, 1);
        Ok(())
    }

    #[test]
    fn test_multiple_consecutive_newlines() -> Result<(), String> {
        let source = "a\n\n\nb";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Identifier("a".to_string()),
                TokenKind::Newline,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        Ok(())
    }

    // ---------------------------------------------------------
    // 5. EOF Behavior
    // ---------------------------------------------------------

    #[test]
    fn test_eof_no_phantom_tokens() -> Result<(), String> {
        let source = "a\n  b";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Identifier("a".to_string()),
                TokenKind::Newline,
                TokenKind::Indent,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Newline,
                TokenKind::Dedent,
                TokenKind::Eof,
            ],
        );
        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
        Ok(())
    }

    #[test]
    fn test_windows_crlf() -> Result<(), String> {
        let source = "a\r\nb";
        let tokens = collect_tokens(source).map_err(|e| e.message)?;
        check_kinds(
            &tokens,
            &[
                TokenKind::Identifier("a".to_string()),
                TokenKind::Newline,
                TokenKind::Identifier("b".to_string()),
                TokenKind::Newline,
                TokenKind::Eof,
            ],
        );
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[2].line, 2);
        Ok(())
    }
}

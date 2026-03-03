#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // 1. Keywords
    Let,
    If,
    Else,
    For,
    In,
    While,
    Func,
    Return,
    Write,
    True,
    False,
    And,
    Or,
    Not,

    // 2. Literals
    Identifier(String),
    Number(i64),
    String(String),

    // 3. Operators
    Assign,       // =
    Plus,         // +
    Minus,        // -
    Multiply,     // *
    Divide,       // /
    Modulo,       // %
    GreaterThan,  // >
    LessThan,     // <
    Equal,        // ==
    NotEqual,     // !=
    GreaterEqual, // >=
    LessEqual,    // <=

    // 4. Delimiters
    LParen,       // (
    RParen,       // )
    LBracket,     // [
    RBracket,     // ]
    Comma,        // ,
    Colon,        // :

    // 5. Structural Tokens
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

impl TokenKind {
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Self::Let
                | Self::If
                | Self::Else
                | Self::For
                | Self::In
                | Self::While
                | Self::Func
                | Self::Return
                | Self::Write
                | Self::True
                | Self::False
                | Self::And
                | Self::Or
                | Self::Not
        )
    }

    pub fn is_operator(&self) -> bool {
        matches!(
            self,
            Self::Assign
                | Self::Plus
                | Self::Minus
                | Self::Multiply
                | Self::Divide
                | Self::Modulo
                | Self::GreaterThan
                | Self::LessThan
                | Self::Equal
                | Self::NotEqual
                | Self::GreaterEqual
                | Self::LessEqual
        )
    }

    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Self::Identifier(_)
                | Self::Number(_)
                | Self::String(_)
                | Self::True
                | Self::False
        )
    }
}

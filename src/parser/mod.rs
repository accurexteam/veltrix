pub mod ast;
pub mod error;

use crate::lexer::token::{Token, TokenKind};
use ast::{Program, Expression};
use error::ParserError;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest = 0,
    Or = 1,
    And = 2,
    Equality = 3,
    Comparison = 4,
    Term = 5,
    Factor = 6,
    Unary = 7,
    Primary = 8,
}

pub struct Parser {
    pub tokens: Vec<Token>,
    pub position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, position: 0 }
    }

    pub fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    pub fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    pub fn get_precedence(&self) -> Precedence {
        if let Some(token) = self.current() {
            match token.kind {
                TokenKind::Or => Precedence::Or,
                TokenKind::And => Precedence::And,
                TokenKind::Equal | TokenKind::NotEqual => Precedence::Equality,
                TokenKind::LessThan | TokenKind::GreaterThan | TokenKind::LessEqual | TokenKind::GreaterEqual => Precedence::Comparison,
                TokenKind::Plus | TokenKind::Minus => Precedence::Term,
                TokenKind::Multiply | TokenKind::Divide => Precedence::Factor,
                _ => Precedence::Lowest,
            }
        } else {
            Precedence::Lowest
        }
    }

    pub fn match_token(&mut self, kind: &TokenKind) -> bool {
        if let Some(token) = self.current() {
            // Because token.kind could hold data, strict equality is used here if they match perfectly.
            if &token.kind == kind {
                self.advance();
                return true;
            }
        }
        false
    }

    pub fn expect(&mut self, kind: TokenKind) -> Result<&Token, ParserError> {
        if self.match_token(&kind) {
            if let Some(token) = self.tokens.get(self.position - 1) {
                return Ok(token);
            }
        }

        let (line, column) = if let Some(token) = self.current() {
            (token.line, token.column)
        } else if let Some(last) = self.tokens.last() {
            (last.line, last.column)
        } else {
            (0, 0)
        };

        Err(ParserError {
            message: format!("Expected {:?}", kind),
            line,
            column,
        })
    }

    pub fn parse_program(&mut self) -> Result<Program, ParserError> {
        let mut statements = Vec::new();
        let mut last_position = self.position;

        while let Some(token) = self.current() {
            if token.kind == TokenKind::Eof {
                break;
            }
            if token.kind == TokenKind::Newline {
                self.advance();
                continue;
            }

            // Safety check: ensure we always make progress
            let stmt = self.parse_statement()?;
            if self.position == last_position {
                let (line, column) = if let Some(t) = self.current() {
                    (t.line, t.column)
                } else {
                    self.get_last_position()
                };
                return Err(ParserError {
                    message: "Parser stuck at token - cannot make progress".to_string(),
                    line,
                    column,
                });
            }
            last_position = self.position;
            statements.push(stmt);
        }

        Ok(Program { statements })
    }

    pub fn parse_statement(&mut self) -> Result<ast::Statement, ParserError> {
        let token = match self.current() {
            Some(t) => t.clone(),
            None => {
                let (line, column) = self.get_last_position();
                return Err(ParserError {
                    message: "Unexpected EOF while parsing statement".to_string(),
                    line,
                    column,
                });
            }
        };

        match &token.kind {
            TokenKind::Let => self.parse_let_statement(),
            TokenKind::Write => self.parse_write_statement(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Func => self.parse_function_declaration(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Else => {
                Err(ParserError {
                    message: "else without matching if".to_string(),
                    line: token.line,
                    column: token.column,
                })
            }
            TokenKind::Identifier(_) => {
                if let Some(next) = self.tokens.get(self.position + 1) {
                    if next.kind == TokenKind::Assign {
                        return self.parse_assignment_statement();
                    }
                }
                let expr = self.parse_expression(Precedence::Lowest)?;
                Ok(ast::Statement::Expression(expr))
            }
            TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::True
            | TokenKind::False
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::Minus
            | TokenKind::Not => {
                let expr = self.parse_expression(Precedence::Lowest)?;
                Ok(ast::Statement::Expression(expr))
            }
            _ => {
                Err(ParserError {
                    message: "Unexpected token: does not start a valid statement".to_string(),
                    line: token.line,
                    column: token.column,
                })
            }
        }
    }

    fn get_last_position(&self) -> (usize, usize) {
        if let Some(token) = self.tokens.last() {
            (token.line, token.column)
        } else {
            (0, 0)
        }
    }

    pub fn parse_expression(&mut self, precedence: Precedence) -> Result<Expression, ParserError> {
        let mut left_exp = self.parse_prefix()?;

        while let Some(token) = self.current() {
            // Don't cross statement boundaries
            match token.kind {
                TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof | TokenKind::Colon => break,
                _ => {}
            }

            let next_precedence = self.get_precedence();

            if next_precedence <= precedence {
                break;
            }

            left_exp = self.parse_infix(left_exp)?;
        }

        Ok(left_exp)
    }

    fn parse_prefix(&mut self) -> Result<Expression, ParserError> {
        let token = match self.current() {
            Some(t) => t.clone(),
            None => {
                let (line, column) = self.get_last_position();
                return Err(ParserError {
                    message: "Unexpected EOF while parsing expression".to_string(),
                    line,
                    column,
                });
            }
        };

        match &token.kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                // Check if this is a function call (identifier followed by '(')
                if let Some(token) = self.current() {
                    if token.kind == TokenKind::LParen {
                        return self.parse_function_call_expression(name);
                    }
                }
                Ok(Expression::Identifier(name))
            }
            TokenKind::Number(value) => {
                let val = *value;
                self.advance();
                Ok(Expression::Integer(val))
            }
            TokenKind::String(value) => {
                let val = value.clone();
                self.advance();
                Ok(Expression::String(val))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression::Boolean(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::Boolean(false))
            }
            TokenKind::LParen => {
                self.advance(); // Consume '('
                let exp = self.parse_expression(Precedence::Lowest)?;
                
                if let Some(next) = self.current() {
                    if next.kind == TokenKind::RParen {
                        self.advance(); // Consume ')'
                    } else {
                        return Err(ParserError {
                            message: format!("Expected ')', found {:?}", next.kind),
                            line: next.line,
                            column: next.column,
                        });
                    }
                } else {
                    let (line, column) = self.get_last_position();
                    return Err(ParserError {
                        message: "Expected ')', found EOF".to_string(),
                        line,
                        column,
                    });
                }
                
                Ok(exp)
            }
            TokenKind::LBracket => {
                self.advance(); // Consume '['
                let mut elements = Vec::new();
                
                if let Some(t) = self.current() {
                    if t.kind == TokenKind::RBracket {
                        self.advance(); // empty array
                        return Ok(Expression::Array(elements));
                    }
                }
                
                loop {
                    if self.current().is_none() {
                        let (line, column) = self.get_last_position();
                        return Err(ParserError {
                            message: "Expected ']' or expression, found EOF".to_string(),
                            line,
                            column,
                        });
                    }
                    
                    elements.push(self.parse_expression(Precedence::Lowest)?);
                    
                    if let Some(t) = self.current() {
                        if t.kind == TokenKind::Comma {
                            self.advance(); // Consume ','
                        } else if t.kind == TokenKind::RBracket {
                            self.advance(); // Consume ']'
                            break;
                        } else {
                            return Err(ParserError {
                                message: format!("Expected ',' or ']', found {:?}", t.kind),
                                line: t.line,
                                column: t.column,
                            });
                        }
                    } else {
                        let (line, column) = self.get_last_position();
                        return Err(ParserError {
                            message: "Expected ',' or ']', found EOF".to_string(),
                            line,
                            column,
                        });
                    }
                }
                Ok(Expression::Array(elements))
            }
            TokenKind::Minus | TokenKind::Not => {
                let operator = if token.kind == TokenKind::Minus { "-" } else { "not" }.to_string();
                self.advance(); // consume operator
                
                let right = self.parse_expression(Precedence::Unary)?;
                
                Ok(Expression::Unary(ast::UnaryExpression {
                    operator,
                    right: Box::new(right),
                }))
            }
            // Detect binary operators at expression start (double operator pattern)
            TokenKind::Plus | TokenKind::Multiply | TokenKind::Divide | TokenKind::Modulo => {
                Err(ParserError {
                    message: "Unexpected operator: missing left operand".to_string(),
                    line: token.line,
                    column: token.column,
                })
            }
            _ => {
                Err(ParserError {
                    message: format!("Unexpected token for expression prefix: {:?}", token.kind),
                    line: token.line,
                    column: token.column,
                })
            }
        }
    }

    fn parse_infix(&mut self, left: Expression) -> Result<Expression, ParserError> {
        let token = match self.current() {
            Some(t) => t.clone(),
            None => {
                let (line, column) = self.get_last_position();
                return Err(ParserError {
                    message: "Unexpected EOF during infix parsing".to_string(),
                    line,
                    column,
                });
            }
        };

        let operator = match &token.kind {
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Multiply => "*",
            TokenKind::Divide => "/",
            TokenKind::Equal => "==",
            TokenKind::NotEqual => "!=",
            TokenKind::LessThan => "<",
            TokenKind::GreaterThan => ">",
            TokenKind::LessEqual => "<=",
            TokenKind::GreaterEqual => ">=",
            TokenKind::And => "and",
            TokenKind::Or => "or",
            _ => {
                return Err(ParserError {
                    message: format!("Unexpected infix operator: {:?}", token.kind),
                    line: token.line,
                    column: token.column,
                });
            }
        }.to_string();

        let precedence = self.get_precedence();
        self.advance(); // consume operator

        // Detect double operator: check if next token is also a binary operator
        if let Some(next) = self.current() {
            match next.kind {
                TokenKind::Plus | TokenKind::Multiply | TokenKind::Divide | TokenKind::Modulo |
                TokenKind::Equal | TokenKind::NotEqual | TokenKind::LessThan | TokenKind::GreaterThan |
                TokenKind::LessEqual | TokenKind::GreaterEqual | TokenKind::And | TokenKind::Or => {
                    return Err(ParserError {
                        message: "Double operator detected: missing expression between operators".to_string(),
                        line: next.line,
                        column: next.column,
                    });
                }
                _ => {}
            }
        }

        let right = self.parse_expression(precedence)?;

        Ok(Expression::Binary(ast::BinaryExpression {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }))
    }

    fn parse_let_statement(&mut self) -> Result<ast::Statement, ParserError> {
        let token = self.current().cloned();
        let (line, column) = token.as_ref().map(|t| (t.line, t.column)).unwrap_or_else(|| self.get_last_position());

        self.advance(); // consume 'let'

        // Expect identifier
        let name = match self.current() {
            Some(t) => match &t.kind {
                TokenKind::Identifier(n) => {
                    let name = n.clone();
                    self.advance();
                    name
                }
                _ => {
                    return Err(ParserError {
                        message: format!("Expected identifier after 'let', found {:?}", t.kind),
                        line: t.line,
                        column: t.column,
                    });
                }
            },
            None => {
                return Err(ParserError {
                    message: "Expected identifier after 'let', found EOF".to_string(),
                    line,
                    column,
                });
            }
        };

        // Expect '='
        self.expect(TokenKind::Assign)?;

        // Parse expression
        let value = self.parse_expression(Precedence::Lowest)?;

        Ok(ast::Statement::Let(ast::LetStatement { name, value }))
    }

    fn parse_assignment_statement(&mut self) -> Result<ast::Statement, ParserError> {
        let token = self.current().cloned();
        let (line, column) = token.as_ref().map(|t| (t.line, t.column)).unwrap_or_else(|| self.get_last_position());

        // Get identifier name
        let name = match &token {
            Some(t) => match &t.kind {
                TokenKind::Identifier(n) => n.clone(),
                _ => {
                    return Err(ParserError {
                        message: format!("Expected identifier for assignment, found {:?}", t.kind),
                        line: t.line,
                        column: t.column,
                    });
                }
            },
            None => {
                return Err(ParserError {
                    message: "Expected identifier for assignment, found EOF".to_string(),
                    line,
                    column,
                });
            }
        };

        self.advance(); // consume identifier
        self.advance(); // consume '='

        // Parse expression
        let value = self.parse_expression(Precedence::Lowest)?;

        Ok(ast::Statement::Assignment(ast::AssignmentStatement { name, value }))
    }

    fn parse_write_statement(&mut self) -> Result<ast::Statement, ParserError> {
        self.advance(); // consume 'write'

        // Parse expression
        let value = self.parse_expression(Precedence::Lowest)?;

        Ok(ast::Statement::Write(ast::WriteStatement { value }))
    }

    fn parse_if_statement(&mut self) -> Result<ast::Statement, ParserError> {
        self.advance(); // consume 'if'

        // Parse condition expression
        let condition = self.parse_expression(Precedence::Lowest)?;

        // Expect colon
        self.expect(TokenKind::Colon)?;

        // Expect INDENT and parse consequence block
        let consequence = self.parse_block()?;

        // Check for optional else
        let mut alternative = None;
        if let Some(token) = self.current() {
            if token.kind == TokenKind::Else {
                self.advance(); // consume 'else'

                // Expect colon
                self.expect(TokenKind::Colon)?;

                // Expect INDENT and parse else block
                alternative = Some(self.parse_block()?);
            }
        }

        Ok(ast::Statement::If(ast::IfStatement {
            condition,
            consequence,
            alternative,
        }))
    }

    fn parse_while_statement(&mut self) -> Result<ast::Statement, ParserError> {
        self.advance(); // consume 'while'

        // Parse condition expression
        let condition = self.parse_expression(Precedence::Lowest)?;

        // Expect colon
        self.expect(TokenKind::Colon)?;

        // Expect INDENT and parse body block
        let body = self.parse_block()?;

        Ok(ast::Statement::While(ast::WhileStatement { condition, body }))
    }

    fn parse_for_statement(&mut self) -> Result<ast::Statement, ParserError> {
        let token = self.current().cloned();
        let (line, column) = token.as_ref().map(|t| (t.line, t.column)).unwrap_or_else(|| self.get_last_position());

        self.advance(); // consume 'for'

        // Expect identifier
        let item = match self.current() {
            Some(t) => match &t.kind {
                TokenKind::Identifier(n) => {
                    let name = n.clone();
                    self.advance();
                    name
                }
                _ => {
                    return Err(ParserError {
                        message: format!("Expected identifier after 'for', found {:?}", t.kind),
                        line: t.line,
                        column: t.column,
                    });
                }
            },
            None => {
                return Err(ParserError {
                    message: "Expected identifier after 'for', found EOF".to_string(),
                    line,
                    column,
                });
            }
        };

        // Expect 'in' keyword
        self.expect(TokenKind::In)?;

        // Parse list expression
        let list = self.parse_expression(Precedence::Lowest)?;

        // Expect colon
        self.expect(TokenKind::Colon)?;

        // Expect INDENT and parse body block
        let body = self.parse_block()?;

        Ok(ast::Statement::For(ast::ForStatement { item, list, body }))
    }

    fn parse_function_declaration(&mut self) -> Result<ast::Statement, ParserError> {
        let token = self.current().cloned();
        let (line, column) = token.as_ref().map(|t| (t.line, t.column)).unwrap_or_else(|| self.get_last_position());

        self.advance(); // consume 'function'

        // Expect identifier for function name
        let name = match self.current() {
            Some(t) => match &t.kind {
                TokenKind::Identifier(n) => {
                    let name = n.clone();
                    self.advance();
                    name
                }
                _ => {
                    return Err(ParserError {
                        message: format!("Expected function name after 'function', found {:?}", t.kind),
                        line: t.line,
                        column: t.column,
                    });
                }
            },
            None => {
                return Err(ParserError {
                    message: "Expected function name after 'function', found EOF".to_string(),
                    line,
                    column,
                });
            }
        };

        // Validate function name is not a reserved keyword
        if TokenKind::is_keyword(&TokenKind::Identifier(name.clone())) {
            return Err(ParserError {
                message: format!("Function name '{}' is a reserved keyword", name),
                line,
                column,
            });
        }

        // Expect '('
        self.expect(TokenKind::LParen)?;

        // Parse parameter list (zero or more parameters)
        let mut params: Vec<String> = Vec::new();

        // Check if there are any parameters or if it's empty ()
        if let Some(token) = self.current() {
            if token.kind != TokenKind::RParen {
                // Parse at least one parameter
                loop {
                    let param_token = self.current().cloned();
                    match param_token {
                        Some(t) => match &t.kind {
                            TokenKind::Identifier(n) => {
                                let param_name = n.clone();
                                // Validate parameter name is not a keyword
                                if TokenKind::is_keyword(&TokenKind::Identifier(param_name.clone())) {
                                    return Err(ParserError {
                                        message: format!("Parameter name '{}' is a reserved keyword", param_name),
                                        line: t.line,
                                        column: t.column,
                                    });
                                }
                                self.advance();
                                params.push(param_name);
                            }
                            TokenKind::Comma => {
                                return Err(ParserError {
                                    message: "Unexpected comma: expected parameter name before comma".to_string(),
                                    line: t.line,
                                    column: t.column,
                                });
                            }
                            _ => {
                                return Err(ParserError {
                                    message: format!("Expected parameter name, found {:?}", t.kind),
                                    line: t.line,
                                    column: t.column,
                                });
                            }
                        },
                        None => {
                            return Err(ParserError {
                                message: "Expected parameter name or ')', found EOF".to_string(),
                                line,
                                column,
                            });
                        }
                    }

                    // Check for next token after parameter name
                    match self.current() {
                        Some(t) => {
                            if t.kind == TokenKind::Comma {
                                self.advance(); // consume ','
                                // Check if there's another parameter after comma
                                if let Some(next) = self.current() {
                                    if next.kind == TokenKind::RParen {
                                        // Trailing comma is allowed: function(a, b,) {}
                                        break;
                                    }
                                    if next.kind == TokenKind::Comma {
                                        return Err(ParserError {
                                            message: "Unexpected double comma in parameter list".to_string(),
                                            line: next.line,
                                            column: next.column,
                                        });
                                    }
                                } else {
                                    return Err(ParserError {
                                        message: "Expected parameter name after comma, found EOF".to_string(),
                                        line,
                                        column,
                                    });
                                }
                                // Continue to parse next parameter
                            } else if t.kind == TokenKind::RParen {
                                // End of parameter list
                                break;
                            } else {
                                return Err(ParserError {
                                    message: format!("Expected ',' or ')' in parameter list, found {:?}", t.kind),
                                    line: t.line,
                                    column: t.column,
                                });
                            }
                        }
                        None => {
                            return Err(ParserError {
                                message: "Expected ',' or ')' in parameter list, found EOF".to_string(),
                                line,
                                column,
                            });
                        }
                    }
                }
            }
        }

        // Expect ')'
        self.expect(TokenKind::RParen)?;

        // Expect colon
        self.expect(TokenKind::Colon)?;

        // Expect INDENT and parse body block
        let body = self.parse_block()?;

        Ok(ast::Statement::FunctionDeclaration(ast::FunctionDeclaration { name, params, body }))
    }

    fn parse_return_statement(&mut self) -> Result<ast::Statement, ParserError> {
        self.advance(); // consume 'return'

        // Check if there's an expression (optional)
        let token = self.current().cloned();
        let value = match &token {
            Some(t) => {
                // Don't parse expression if we hit NEWLINE, DEDENT, or EOF
                match t.kind {
                    TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof => None,
                    _ => Some(self.parse_expression(Precedence::Lowest)?),
                }
            }
            None => None,
        };

        Ok(ast::Statement::Return(ast::ReturnStatement { value }))
    }

    fn parse_function_call_expression(&mut self, name: String) -> Result<Expression, ParserError> {
        self.advance(); // consume '('

        let mut arguments = Vec::new();

        // Check if empty argument list
        if let Some(token) = self.current() {
            if token.kind == TokenKind::RParen {
                self.advance(); // consume ')'
                return Ok(Expression::FunctionCall(ast::FunctionCallExpression {
                    name,
                    arguments,
                }));
            }
        }

        // Parse argument list (one or more arguments)
        loop {
            let arg = self.parse_expression(Precedence::Lowest)?;
            arguments.push(arg);

            match self.current() {
                Some(t) => {
                    if t.kind == TokenKind::Comma {
                        self.advance(); // consume ','
                        // Check for trailing comma
                        if let Some(next) = self.current() {
                            if next.kind == TokenKind::RParen {
                                self.advance(); // consume ')'
                                break;
                            }
                        }
                    } else if t.kind == TokenKind::RParen {
                        self.advance(); // consume ')'
                        break;
                    } else {
                        return Err(ParserError {
                            message: format!("Expected ',' or ')' in argument list, found {:?}", t.kind),
                            line: t.line,
                            column: t.column,
                        });
                    }
                }
                None => {
                    let (line, column) = self.get_last_position();
                    return Err(ParserError {
                        message: "Expected ',' or ')' in argument list, found EOF".to_string(),
                        line,
                        column,
                    });
                }
            }
        }

        Ok(Expression::FunctionCall(ast::FunctionCallExpression {
            name,
            arguments,
        }))
    }

    fn parse_block(&mut self) -> Result<Vec<ast::Statement>, ParserError> {
        let (line, column) = self.current().map(|t| (t.line, t.column)).unwrap_or_else(|| self.get_last_position());

        // Skip newlines between colon and indented block
        while let Some(t) = self.current() {
            if t.kind == TokenKind::Newline {
                self.advance();
            } else {
                break;
            }
        }

        // Expect INDENT
        let indent_token = self.current().cloned();
        match &indent_token {
            Some(t) => {
                if t.kind != TokenKind::Indent {
                    return Err(ParserError {
                        message: format!("Expected indented block, found {:?}", t.kind),
                        line: t.line,
                        column: t.column,
                    });
                }
                self.advance(); // consume INDENT
            }
            None => {
                return Err(ParserError {
                    message: "Expected indented block, found EOF".to_string(),
                    line,
                    column,
                });
            }
        };

        let mut statements = Vec::new();
        let mut last_position = self.position;

        // Parse statements until DEDENT
        loop {
            let current_token = self.current().cloned();
            match &current_token {
                Some(t) => {
                    let t_line = t.line;
                    let t_column = t.column;
                    match t.kind {
                        TokenKind::Dedent => {
                            self.advance(); // consume DEDENT
                            break;
                        }
                        TokenKind::Eof => {
                            return Err(ParserError {
                                message: "Expected end of block (DEDENT), found EOF".to_string(),
                                line: t_line,
                                column: t_column,
                            });
                        }
                        TokenKind::Newline => {
                            self.advance(); // skip newlines in block
                            last_position = self.position;
                        }
                        _ => {
                            let stmt = self.parse_statement()?;
                            // Safety check: ensure we make progress
                            if self.position == last_position {
                                return Err(ParserError {
                                    message: "Parser stuck in block".to_string(),
                                    line: t_line,
                                    column: t_column,
                                });
                            }
                            last_position = self.position;
                            statements.push(stmt);
                        }
                    }
                }
                None => {
                    return Err(ParserError {
                        message: "Expected end of block (DEDENT), found EOF".to_string(),
                        line,
                        column,
                    });
                }
            }
        }

        Ok(statements)
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_source(source: &str) -> Result<Program, ParserError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| ParserError {
            message: e.message,
            line: e.line,
            column: e.column,
        })?;
        let mut parser = Parser::new(tokens);
        parser.parse_program()
    }

    // =========================================================================
    // VALID SYNTAX TESTS
    // =========================================================================

    #[test]
    fn test_let_declaration() {
        let result = parse_source("let x = 5");
        assert!(result.is_ok(), "let declaration should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Let(let_stmt) => {
                assert_eq!(let_stmt.name, "x");
            }
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_reassignment() {
        let result = parse_source("let x = 5\nx = 10");
        assert!(result.is_ok(), "reassignment should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 2);
        match &program.statements[1] {
            ast::Statement::Assignment(assign) => {
                assert_eq!(assign.name, "x");
            }
            _ => panic!("Expected Assignment statement"),
        }
    }

    #[test]
    fn test_write_statement() {
        let result = parse_source("write 42");
        assert!(result.is_ok(), "write statement should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Write(_) => {}
            _ => panic!("Expected Write statement"),
        }
    }

    #[test]
    fn test_nested_if_else() {
        let source = "if true:\n    if false:\n        write 1\n    else:\n        write 2";
        let result = parse_source(source);
        assert!(result.is_ok(), "nested if/else should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
    }

    #[test]
    fn test_nested_loops() {
        let source = "for i in [1, 2]:\n    for j in [3, 4]:\n        write i";
        let result = parse_source(source);
        assert!(result.is_ok(), "nested loops should parse: {:?}", result.err());
    }

    #[test]
    fn test_function_declaration() {
        let source = "function foo():\n    write 1";
        let result = parse_source(source);
        assert!(result.is_ok(), "function declaration should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::FunctionDeclaration(func) => {
                assert_eq!(func.name, "foo");
                assert_eq!(func.params.len(), 0);
                assert_eq!(func.body.len(), 1);
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_function_with_single_param() {
        let source = "function greet(name):\n    write name";
        let result = parse_source(source);
        assert!(result.is_ok(), "function with param should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::FunctionDeclaration(func) => {
                assert_eq!(func.name, "greet");
                assert_eq!(func.params, vec!["name"]);
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_function_with_multiple_params() {
        let source = "function add(a, b):\n    return a + b";
        let result = parse_source(source);
        assert!(result.is_ok(), "function with multiple params should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::FunctionDeclaration(func) => {
                assert_eq!(func.name, "add");
                assert_eq!(func.params, vec!["a", "b"]);
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_function_with_trailing_comma() {
        let source = "function foo(a, b,):\n    write 1";
        let result = parse_source(source);
        assert!(result.is_ok(), "function with trailing comma should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::FunctionDeclaration(func) => {
                assert_eq!(func.params, vec!["a", "b"]);
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_return_without_expression() {
        let source = "function foo():\n    return";
        let result = parse_source(source);
        assert!(result.is_ok(), "return without expression should parse: {:?}", result.err());
    }

    #[test]
    fn test_return_with_expression() {
        let source = "function foo():\n    return 42";
        let result = parse_source(source);
        assert!(result.is_ok(), "return with expression should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::FunctionDeclaration(func) => {
                match &func.body[0] {
                    ast::Statement::Return(ret) => {
                        assert!(ret.value.is_some());
                    }
                    _ => panic!("Expected Return statement"),
                }
            }
            _ => panic!("Expected FunctionDeclaration"),
        }
    }

    #[test]
    fn test_for_loop_with_array() {
        let source = "for x in [1, 2, 3]:\n    write x";
        let result = parse_source(source);
        assert!(result.is_ok(), "for loop with array should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::For(for_stmt) => {
                assert_eq!(for_stmt.item, "x");
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_while_loop() {
        let source = "while true:\n    write 1";
        let result = parse_source(source);
        assert!(result.is_ok(), "while loop should parse: {:?}", result.err());
        let program = result.unwrap();
        match &program.statements[0] {
            ast::Statement::While(_) => {}
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_complex_expression_inside_if() {
        let source = "if (x + 5) > 10 and y < 20:\n    write 1";
        let result = parse_source(source);
        assert!(result.is_ok(), "complex expression in if should parse: {:?}", result.err());
    }

    #[test]
    fn test_nested_blocks_3_levels() {
        let source = "if true:\n    if true:\n        if true:\n            write 1";
        let result = parse_source(source);
        assert!(result.is_ok(), "3-level nested blocks should parse: {:?}", result.err());
    }

    #[test]
    fn test_empty_file() {
        let result = parse_source("");
        assert!(result.is_ok(), "empty file should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 0);
    }

    #[test]
    fn test_multiple_statements() {
        let source = "let a = 1\nlet b = 2\nwrite a + b";
        let result = parse_source(source);
        assert!(result.is_ok(), "multiple statements should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 3);
    }

    #[test]
    fn test_boolean_literals() {
        let source = "let t = true\nlet f = false";
        let result = parse_source(source);
        assert!(result.is_ok(), "boolean literals should parse: {:?}", result.err());
    }

    #[test]
    fn test_string_literal() {
        let source = "let s = \"hello world\"";
        let result = parse_source(source);
        assert!(result.is_ok(), "string literal should parse: {:?}", result.err());
    }

    #[test]
    fn test_empty_array() {
        let source = "let arr = []";
        let result = parse_source(source);
        assert!(result.is_ok(), "empty array should parse: {:?}", result.err());
    }

    #[test]
    fn test_unary_operators() {
        let source = "let a = -5\nlet b = not true";
        let result = parse_source(source);
        assert!(result.is_ok(), "unary operators should parse: {:?}", result.err());
    }

    // =========================================================================
    // INVALID SYNTAX TESTS
    // =========================================================================

    #[test]
    fn test_expression_statement_binary() {
        let result = parse_source("5 + 3");
        assert!(result.is_ok(), "expression statement 5 + 3 should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Expression(expr) => {
                assert!(matches!(expr, ast::Expression::Binary(_)));
            }
            _ => panic!("Expected Expression statement"),
        }
    }

    #[test]
    fn test_expression_statement_identifier() {
        let result = parse_source("x");
        assert!(result.is_ok(), "expression statement x should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Expression(expr) => {
                assert!(matches!(expr, ast::Expression::Identifier(_)));
            }
            _ => panic!("Expected Expression statement"),
        }
    }

    #[test]
    fn test_expression_statement_function_call() {
        let result = parse_source("foo()");
        assert!(result.is_ok(), "expression statement foo() should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Expression(expr) => {
                assert!(matches!(expr, ast::Expression::FunctionCall(_)));
            }
            _ => panic!("Expected Expression statement"),
        }
    }

    #[test]
    fn test_expression_statement_unary() {
        let result = parse_source("-5");
        assert!(result.is_ok(), "expression statement -5 should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Expression(expr) => assert!(matches!(expr, ast::Expression::Unary(_))),
            _ => panic!("Expected Expression statement"),
        }
    }

    #[test]
    fn test_expression_statement_nested() {
        let result = parse_source("(5 + 3) * 2");
        assert!(result.is_ok(), "expression statement (5 + 3) * 2 should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Expression(_) => {}
            _ => panic!("Expected Expression statement"),
        }
    }

    #[test]
    fn test_expression_statement_true_nil() {
        let result = parse_source("true");
        assert!(result.is_ok(), "expression statement true should parse: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 1);
        match &program.statements[0] {
            ast::Statement::Expression(expr) => assert!(matches!(expr, ast::Expression::Boolean(true))),
            _ => panic!("Expected Expression statement"),
        }
    }

    #[test]
    fn test_invalid_standalone_rparen() {
        let result = parse_source(")");
        assert!(result.is_err(), "standalone ) should error");
    }

    #[test]
    fn test_invalid_standalone_leading_operator() {
        let result = parse_source("+ 5");
        assert!(result.is_err(), "leading + should error");
    }

    #[test]
    fn test_let_missing_identifier() {
        let result = parse_source("let = 5");
        assert!(result.is_err(), "let without identifier should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected identifier after 'let'"), 
            "Error should indicate missing identifier: {}", err.message);
    }

    #[test]
    fn test_let_missing_equals() {
        let result = parse_source("let x 5");
        assert!(result.is_err(), "let without = should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected Assign"), 
            "Error should indicate missing =: {}", err.message);
    }

    #[test]
    fn test_if_missing_colon() {
        let result = parse_source("if x > 5\n    write 1");
        assert!(result.is_err(), "if without colon should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected Colon") || err.message.contains("Expected indented block"),
            "Error should indicate missing colon: {}", err.message);
    }

    #[test]
    fn test_else_without_if() {
        let result = parse_source("else:\n    write 1");
        assert!(result.is_err(), "else without if should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("else without matching if"),
            "Error should indicate else without if: {}", err.message);
    }

    #[test]
    fn test_for_missing_identifier() {
        let result = parse_source("for in [1,2]:\n    write 1");
        assert!(result.is_err(), "for without identifier should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected identifier after 'for'"),
            "Error should indicate missing identifier: {}", err.message);
    }

    #[test]
    fn test_function_missing_name() {
        let result = parse_source("function ():\n    write 1");
        assert!(result.is_err(), "function without name should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected function name after 'function'"),
            "Error should indicate missing function name: {}", err.message);
    }

    #[test]
    fn test_function_with_keyword_name() {
        let result = parse_source("function let():\n    write 1");
        assert!(result.is_err(), "function with keyword name should error");
    }

    #[test]
    fn test_function_double_comma() {
        let result = parse_source("function foo(a,,b):\n    write 1");
        assert!(result.is_err(), "double comma in params should error");
    }

    #[test]
    fn test_function_unmatched_paren() {
        let result = parse_source("function bar(a, b");
        assert!(result.is_err(), "unmatched paren should error");
    }

    #[test]
    fn test_function_missing_colon() {
        let result = parse_source("function foo()\n    write 1");
        assert!(result.is_err(), "function without colon should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected Colon"),
            "Error should indicate missing colon: {}", err.message);
    }

    #[test]
    fn test_while_missing_colon() {
        let result = parse_source("while true\n    write 1");
        assert!(result.is_err(), "while without colon should error");
    }

    #[test]
    fn test_double_operator() {
        let result = parse_source("let x = 1 + * 2");
        assert!(result.is_err(), "double operator should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Unexpected operator") || err.message.contains("Double operator"),
            "Error should indicate operator issue: {}", err.message);
    }

    #[test]
    fn test_unmatched_parentheses() {
        let result = parse_source("let x = (1 + 2");
        assert!(result.is_err(), "unmatched ( should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected ')'"),
            "Error should indicate missing ): {}", err.message);
    }

    #[test]
    fn test_unmatched_bracket() {
        let result = parse_source("let x = [1, 2");
        assert!(result.is_err(), "unmatched [ should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected ',' or ']'"),
            "Error should indicate missing ]: {}", err.message);
    }

    #[test]
    fn test_missing_colon_in_function() {
        let result = parse_source("function foo()\n    write 1");
        assert!(result.is_err(), "function without colon should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected Colon"),
            "Error should indicate missing colon: {}", err.message);
    }

    #[test]
    fn test_unexpected_token() {
        let result = parse_source("+ 5");
        assert!(result.is_err(), "unexpected operator at start should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Unexpected token") || err.message.contains("Unexpected operator"),
            "Error should indicate unexpected token: {}", err.message);
    }

    #[test]
    fn test_double_plus_error() {
        let result = parse_source("let x = 1 ++ 2");
        assert!(result.is_err(), "double plus should error");
    }

    #[test]
    fn test_missing_indent() {
        let result = parse_source("if true:\nwrite 1");
        assert!(result.is_err(), "missing indent should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected indented block"),
            "Error should indicate missing indent: {}", err.message);
    }

    #[test]
    fn test_empty_parens_error() {
        let result = parse_source("()");
        assert!(result.is_err(), "standalone empty parens should error");
    }

    #[test]
    fn test_operator_at_expression_start() {
        let result = parse_source("let x = * 5");
        assert!(result.is_err(), "operator at expression start should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Unexpected operator") || err.message.contains("Unexpected token"),
            "Error should indicate unexpected operator: {}", err.message);
    }

    #[test]
    fn test_if_without_condition() {
        let result = parse_source("if:\n    write 1");
        assert!(result.is_err(), "if without condition should error");
    }

    #[test]
    fn test_for_without_in() {
        let result = parse_source("for x [1,2]:\n    write 1");
        assert!(result.is_err(), "for without in should error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Expected In"),
            "Error should indicate missing 'in': {}", err.message);
    }

    #[test]
    fn test_assignment_to_non_identifier() {
        let result = parse_source("5 = 10");
        assert!(result.is_err(), "assignment to literal should error");
    }

    #[test]
    fn test_deeply_nested_blocks() {
        let source = "if true:\n    if true:\n        if true:\n            if true:\n                if true:\n                    write 1";
        let result = parse_source(source);
        assert!(result.is_ok(), "5-level nested blocks should parse: {:?}", result.err());
    }

    #[test]
    fn test_eof_in_expression() {
        let result = parse_source("let x = 1 +");
        assert!(result.is_err(), "EOF in expression should error");
    }

    #[test]
    fn test_error_line_column() {
        let result = parse_source("let x = 5\nlet = 10");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.line, 2, "Error should be on line 2");
        assert!(err.column > 0, "Error should have column > 0");
    }
}

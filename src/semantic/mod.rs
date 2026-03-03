use crate::native::{NativeFunctionRegistry, NativeFunctionValidationError};
use crate::parser::ast::{Expression, FunctionDeclaration, FunctionCallExpression, Program, Statement};
use std::collections::HashMap;
use std::sync::Arc;

/// Immutable function table that stores function declarations at global scope.
/// Uses Arc to avoid cloning the entire AST during storage.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTable {
    functions: HashMap<String, Arc<FunctionDeclaration>>,
}

impl FunctionTable {
    /// Creates a new empty function table.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Registers a function declaration in the table.
    /// Returns an error if a function with the same name already exists.
    pub fn register(
        &mut self,
        declaration: FunctionDeclaration,
    ) -> Result<(), SemanticError> {
        let name = declaration.name.clone();

        if self.functions.contains_key(&name) {
            return Err(SemanticError {
                message: format!("Function '{}' already declared", name),
                line: 0,
                column: 0,
            });
        }

        self.functions.insert(name, Arc::new(declaration));
        Ok(())
    }

    /// Looks up a function by name.
    /// Returns Some(Arc<FunctionDeclaration>) if found, None otherwise.
    pub fn lookup(&self, name: &str) -> Option<Arc<FunctionDeclaration>> {
        self.functions.get(name).cloned()
    }

    /// Returns true if a function with the given name exists in the table.
    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Returns the number of functions in the table.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Returns true if the table contains no functions.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Returns an iterator over function names.
    pub fn function_names(&self) -> impl Iterator<Item = &String> {
        self.functions.keys()
    }
}

impl Default for FunctionTable {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Symbol {
    Variable,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

fn semantic_error_from_native_validation(e: NativeFunctionValidationError) -> SemanticError {
    match e {
        NativeFunctionValidationError::FunctionNotFound { name, line, column } => SemanticError {
            message: format!("Unknown function '{}'", name),
            line,
            column,
        },
        NativeFunctionValidationError::WrongArgumentCount {
            name,
            expected,
            actual,
            line,
            column,
        } => SemanticError {
            message: format!(
                "Function '{}' expects {} argument(s), got {}",
                name, expected, actual
            ),
            line,
            column,
        },
    }
}

#[allow(dead_code)]
pub struct SemanticAnalyzer {
    scopes: Vec<HashMap<String, Symbol>>,
    in_function: bool,
    function_table: FunctionTable,
    native_registry: Option<Arc<NativeFunctionRegistry>>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut scopes = Vec::new();
        scopes.push(HashMap::new());
        Self {
            scopes,
            in_function: false,
            function_table: FunctionTable::new(),
            native_registry: None,
        }
    }

    /// Creates an analyzer with the given native function registry.
    /// The registry is read-only and injected explicitly; no global state.
    pub fn with_native_registry(registry: Arc<NativeFunctionRegistry>) -> Self {
        let mut scopes = Vec::new();
        scopes.push(HashMap::new());
        Self {
            scopes,
            in_function: false,
            function_table: FunctionTable::new(),
            native_registry: Some(registry),
        }
    }

    pub fn function_table(&self) -> &FunctionTable {
        &self.function_table
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) -> Result<(), SemanticError> {
        if self.scopes.len() <= 1 {
            return Err(SemanticError {
                message: "Cannot exit global scope".to_string(),
                line: 0,
                column: 0,
            });
        }
        self.scopes.pop();
        Ok(())
    }

    fn declare_symbol(
        &mut self,
        name: &str,
        symbol: Symbol,
        line: usize,
        column: usize,
    ) -> Result<(), SemanticError> {
        let current_scope = self.scopes.last_mut().ok_or(SemanticError {
            message: "No scope available".to_string(),
            line,
            column,
        })?;

        if current_scope.contains_key(name) {
            return Err(SemanticError {
                message: format!("Symbol '{}' already declared in this scope", name),
                line,
                column,
            });
        }

        current_scope.insert(name.to_string(), symbol);
        Ok(())
    }

    fn resolve_symbol(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }

    fn validate_function_call(
        &self,
        call: &FunctionCallExpression,
        line: usize,
        column: usize,
    ) -> Result<(), SemanticError> {
        let name = &call.name;
        let arg_count = call.arguments.len();

        // First check if it's a native function
        if let Some(ref reg) = self.native_registry {
            if reg.is_native(name) {
                return reg
                    .validate_call(name, arg_count, line, column)
                    .map_err(|e| semantic_error_from_native_validation(e));
            }
        }

        // Check the function table for statically defined functions
        if let Some(func) = self.function_table.lookup(name) {
            let expected = func.params.len();
            if arg_count != expected {
                return Err(SemanticError {
                    message: format!(
                        "Function '{}' expects {} argument(s), got {}",
                        name, expected, arg_count
                    ),
                    line,
                    column,
                });
            }
            return Ok(());
        }

        // Check the symbol table for first-class functions (variables holding function values)
        // Since we can't call resolve_symbol on &self, we check if the name exists in scopes
        for scope in self.scopes.iter().rev() {
            if scope.contains_key(name) {
                // Variable exists, assume it could be a function
                // We can't statically check arity for first-class functions
                return Ok(());
            }
        }

        Err(SemanticError {
            message: format!("Unknown function '{}'", name),
            line,
            column,
        })
    }

    pub fn analyze_program(&mut self, program: &Program) -> Result<(), SemanticError> {
        for stmt in &program.statements {
            self.analyze_statement(stmt)?;
        }
        Ok(())
    }

    fn analyze_statement(&mut self, stmt: &Statement) -> Result<(), SemanticError> {
        match stmt {
            Statement::Let(let_stmt) => {
                // Analyze the initializer FIRST so let x = x fails (x not yet declared)
                self.analyze_expression(&let_stmt.value, 1, 1)?;
                self.declare_symbol(&let_stmt.name, Symbol::Variable, 1, 1)
            }
            Statement::Expression(expr) => {
                self.analyze_expression(expr, 1, 1)
            }
            Statement::Block(stmts) => {
                self.enter_scope();
                for stmt in stmts {
                    self.analyze_statement(stmt)?;
                }
                self.exit_scope()
            }
            Statement::If(if_stmt) => {
                // Analyze condition FIRST
                self.analyze_expression(&if_stmt.condition, 1, 1)?;

                // Analyze consequence - NO implicit scope (blocks handle their own)
                for stmt in &if_stmt.consequence {
                    self.analyze_statement(stmt)?;
                }

                // Analyze alternative if present - NO implicit scope
                if let Some(ref alternative) = &if_stmt.alternative {
                    for stmt in alternative {
                        self.analyze_statement(stmt)?;
                    }
                }

                Ok(())
            }
            Statement::While(while_stmt) => {
                // Analyze condition FIRST
                self.analyze_expression(&while_stmt.condition, 1, 1)?;

                // Analyze body - NO implicit scope (blocks handle their own)
                for stmt in &while_stmt.body {
                    self.analyze_statement(stmt)?;
                }

                Ok(())
            }
            Statement::For(for_stmt) => {
                // For loop creates its own scope
                self.enter_scope();

                // Declare the loop variable FIRST
                self.declare_symbol(&for_stmt.item, Symbol::Variable, 1, 1)?;

                // Analyze the list expression
                self.analyze_expression(&for_stmt.list, 1, 1)?;

                // Analyze body
                for stmt in &for_stmt.body {
                    self.analyze_statement(stmt)?;
                }

                self.exit_scope()
            }
            Statement::FunctionDeclaration(func_decl) => {
                // Register the function in the global function table
                self.function_table.register(func_decl.clone())?;
                // Also register the function name as a variable so it can be resolved
                self.declare_symbol(&func_decl.name, Symbol::Variable, 1, 1)
            }
            // For other statement types, we'll just return Ok for now
            // (return, assignment, write - to be implemented in later phases)
            _ => Ok(()),
        }
    }

    /// Analyze an expression for semantic correctness.
    /// The line and column should reflect where this expression occurs in source.
    pub fn analyze_expression(
        &mut self,
        expr: &Expression,
        line: usize,
        column: usize,
    ) -> Result<(), SemanticError> {
        match expr {
            Expression::Identifier(name) => {
                if self.resolve_symbol(name).is_none() {
                    return Err(SemanticError {
                        message: format!("Undeclared identifier '{}'", name),
                        line,
                        column,
                    });
                }
                Ok(())
            }
            Expression::Integer(_) | Expression::String(_) | Expression::Boolean(_) => Ok(()),
            Expression::Array(elements) => {
                for (idx, element) in elements.iter().enumerate() {
                    self.analyze_expression(element, line, column.saturating_add(idx))?;
                }
                Ok(())
            }
            Expression::Unary(unary) => {
                self.analyze_expression(&unary.right, line, column)
            }
            Expression::Binary(binary) => {
                self.analyze_expression(&binary.left, line, column)?;
                self.analyze_expression(&binary.right, line, column)
            }
            Expression::FunctionCall(call) => {
                self.validate_function_call(call, line, column)?;
                for arg in &call.arguments {
                    self.analyze_expression(arg, line, column)?;
                }
                Ok(())
            }
            Expression::Assignment(assign) => {
                // Assignment expressions can create new variables (like 'let' but simpler syntax)
                // If the variable doesn't exist, declare it as a variable
                if self.resolve_symbol(&assign.name).is_none() {
                    self.declare_symbol(&assign.name, Symbol::Variable, line, column)?;
                }
                // Analyze the value being assigned
                self.analyze_expression(&assign.value, line, column)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{BinaryExpression, LetStatement, UnaryExpression};

    #[test]
    fn test_duplicate_in_same_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("First declaration should succeed");

        let result = analyzer.declare_symbol("x", Symbol::Variable, 2, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("already declared"));
    }

    #[test]
    fn test_shadowing_allowed() {
        let mut analyzer = SemanticAnalyzer::new();

        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Global declaration should succeed");

        analyzer.enter_scope();

        let result = analyzer.declare_symbol("x", Symbol::Variable, 2, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_from_outer_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration should succeed");

        analyzer.enter_scope();

        let result = analyzer.resolve_symbol("x");
        assert!(result.is_some());
        matches!(result.unwrap(), Symbol::Variable);
    }

    #[test]
    fn test_exit_global_scope_error() {
        let mut analyzer = SemanticAnalyzer::new();

        let result = analyzer.exit_scope();
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("global scope"));
    }

    // =========================================================================
    // EXPRESSION ANALYSIS TESTS
    // =========================================================================

    #[test]
    fn test_undeclared_identifier_error() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::Identifier("x".to_string());

        let result = analyzer.analyze_expression(&expr, 1, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 1);
    }

    #[test]
    fn test_declared_identifier_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration should succeed");

        let expr = Expression::Identifier("x".to_string());
        let result = analyzer.analyze_expression(&expr, 2, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_literal_integer_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::Integer(42);

        let result = analyzer.analyze_expression(&expr, 1, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_literal_string_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::String("hello".to_string());

        let result = analyzer.analyze_expression(&expr, 1, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_literal_boolean_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::Boolean(true);

        let result = analyzer.analyze_expression(&expr, 1, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_binary_with_declared_variables_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration of x should succeed");
        analyzer
            .declare_symbol("y", Symbol::Variable, 1, 5)
            .expect("Declaration of y should succeed");

        // Expression: x + (y + 3)
        let inner_binary = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("y".to_string())),
            operator: "+".to_string(),
            right: Box::new(Expression::Integer(3)),
        });
        let outer_binary = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "+".to_string(),
            right: Box::new(inner_binary),
        });

        let result = analyzer.analyze_expression(&outer_binary, 2, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_binary_undeclared_left_operand_error() {
        let mut analyzer = SemanticAnalyzer::new();

        // Expression: x + 5 (x not declared)
        let binary = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "+".to_string(),
            right: Box::new(Expression::Integer(5)),
        });

        let result = analyzer.analyze_expression(&binary, 2, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_nested_binary_undeclared_right_operand_error() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration of x should succeed");

        // Expression: x + y (y not declared)
        let binary = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "+".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });

        let result = analyzer.analyze_expression(&binary, 2, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("y"));
    }

    #[test]
    fn test_unary_expression_with_declared_variable_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration should succeed");

        // Expression: -x
        let unary = Expression::Unary(UnaryExpression {
            operator: "-".to_string(),
            right: Box::new(Expression::Identifier("x".to_string())),
        });

        let result = analyzer.analyze_expression(&unary, 2, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unary_expression_undeclared_operand_error() {
        let mut analyzer = SemanticAnalyzer::new();

        // Expression: -x (x not declared)
        let unary = Expression::Unary(UnaryExpression {
            operator: "-".to_string(),
            right: Box::new(Expression::Identifier("x".to_string())),
        });

        let result = analyzer.analyze_expression(&unary, 2, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_array_with_declared_elements_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration of x should succeed");
        analyzer
            .declare_symbol("y", Symbol::Variable, 1, 5)
            .expect("Declaration of y should succeed");

        // Expression: [x, y, 3]
        let arr = Expression::Array(vec![
            Expression::Identifier("x".to_string()),
            Expression::Identifier("y".to_string()),
            Expression::Integer(3),
        ]);

        let result = analyzer.analyze_expression(&arr, 2, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_with_undeclared_element_error() {
        let mut analyzer = SemanticAnalyzer::new();

        // Expression: [x, 3] (x not declared)
        let arr = Expression::Array(vec![
            Expression::Identifier("x".to_string()),
            Expression::Integer(3),
        ]);

        let result = analyzer.analyze_expression(&arr, 2, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_deeply_nested_expression_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("a", Symbol::Variable, 1, 1)
            .expect("Declaration should succeed");

        // Expression: ((a))
        // Grouping is transparent in the AST, so we model it as nested unary
        // Actually, let's create a deeply nested binary structure
        // Expression: a + (a + (a + a))
        let inner = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("a".to_string())),
            operator: "+".to_string(),
            right: Box::new(Expression::Identifier("a".to_string())),
        });
        let middle = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("a".to_string())),
            operator: "+".to_string(),
            right: Box::new(inner),
        });
        let outer = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("a".to_string())),
            operator: "+".to_string(),
            right: Box::new(middle),
        });

        let result = analyzer.analyze_expression(&outer, 2, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_identifier_from_outer_scope_in_expression_ok() {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer
            .declare_symbol("x", Symbol::Variable, 1, 1)
            .expect("Declaration should succeed");

        analyzer.enter_scope();

        let expr = Expression::Identifier("x".to_string());
        let result = analyzer.analyze_expression(&expr, 3, 1);
        assert!(result.is_ok(), "Should resolve identifier from outer scope");
    }

    #[test]
    fn test_fail_fast_first_error_reported() {
        let mut analyzer = SemanticAnalyzer::new();

        // Expression: x + y (both undeclared, should report x first)
        let binary = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "+".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });

        let result = analyzer.analyze_expression(&binary, 2, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("x"));
        // Should not mention y since we fail fast on x
        assert!(!err.message.contains("y"));
    }

    // =========================================================================
    // STATEMENT ANALYSIS TESTS
    // =========================================================================

    #[test]
    fn test_duplicate_variable_same_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5; let x = 10;
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(10),
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("already declared"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_shadowing_nested_scope_allowed() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5; { let x = 10; }
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::Block(vec![
                    Statement::Let(LetStatement {
                        name: "x".to_string(),
                        value: Expression::Integer(10),
                    }),
                ]),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Shadowing in nested scope should be allowed");
    }

    #[test]
    fn test_outer_variable_used_inside_block() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5; { x; }
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::Block(vec![
                    Statement::Expression(Expression::Identifier("x".to_string())),
                ]),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Using outer variable inside block should be allowed");
    }

    #[test]
    fn test_undeclared_variable_in_block() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: { x; }
        let program = Program {
            statements: vec![
                Statement::Block(vec![
                    Statement::Expression(Expression::Identifier("x".to_string())),
                ]),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_let_with_initializer_valid() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5;
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Let with valid initializer should succeed");
    }

    #[test]
    fn test_let_with_self_reference_invalid() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = x; (x not declared before)
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Identifier("x".to_string()),
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err(), "Self-reference without outer declaration should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_let_with_outer_variable_reference_valid() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5; let y = x;
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::Let(LetStatement {
                    name: "y".to_string(),
                    value: Expression::Identifier("x".to_string()),
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Referencing outer variable in initializer should succeed");
    }

    #[test]
    fn test_deeply_nested_blocks() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5; { { { x; } } }
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::Block(vec![
                    Statement::Block(vec![
                        Statement::Block(vec![
                            Statement::Expression(Expression::Identifier("x".to_string())),
                        ]),
                    ]),
                ]),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Deeply nested blocks should resolve variables from outer scopes");
    }

    #[test]
    fn test_empty_block() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: { }
        let program = Program {
            statements: vec![
                Statement::Block(vec![]),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Empty blocks should be valid");
    }

    #[test]
    fn test_block_inside_block() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x = 5; { let y = x; { y; } }
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::Block(vec![
                    Statement::Let(LetStatement {
                        name: "y".to_string(),
                        value: Expression::Identifier("x".to_string()),
                    }),
                    Statement::Block(vec![
                        Statement::Expression(Expression::Identifier("y".to_string())),
                    ]),
                ]),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Block inside block should work correctly");
    }

    // =========================================================================
    // CONTROL FLOW ANALYSIS TESTS
    // =========================================================================

    #[test]
    fn test_if_with_undeclared_condition() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: if (x) {}
        let program = Program {
            statements: vec![
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Identifier("x".to_string()),
                    consequence: vec![],
                    alternative: None,
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_if_with_valid_condition() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x; if (x) {}
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Boolean(true),
                }),
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Identifier("x".to_string()),
                    consequence: vec![],
                    alternative: None,
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "If with valid condition should succeed");
    }

    #[test]
    fn test_if_without_block_does_not_create_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: if (true) let x = 1; x;
        // With correct semantics: if does NOT create scope, so x IS visible
        let program = Program {
            statements: vec![
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Boolean(true),
                    consequence: vec![
                        Statement::Let(LetStatement {
                            name: "x".to_string(),
                            value: Expression::Integer(1),
                        }),
                    ],
                    alternative: None,
                }),
                Statement::Expression(Expression::Identifier("x".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "If without block should NOT create scope - x should be visible");
    }

    #[test]
    fn test_if_with_block_does_create_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: if (true) { let x = 1; } x;
        // Block creates scope, so x is NOT visible outside
        let program = Program {
            statements: vec![
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Boolean(true),
                    consequence: vec![
                        Statement::Block(vec![
                            Statement::Let(LetStatement {
                                name: "x".to_string(),
                                value: Expression::Integer(1),
                            }),
                        ]),
                    ],
                    alternative: None,
                }),
                Statement::Expression(Expression::Identifier("x".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err(), "If with block SHOULD isolate - x should NOT be visible");
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_nested_if_inside_block() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x; if (x) { if (x) {} }
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Boolean(true),
                }),
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Identifier("x".to_string()),
                    consequence: vec![
                        Statement::If(crate::parser::ast::IfStatement {
                            condition: Expression::Identifier("x".to_string()),
                            consequence: vec![],
                            alternative: None,
                        }),
                    ],
                    alternative: None,
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Nested if inside block should work");
    }

    #[test]
    fn test_while_undeclared_condition() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: while (x) {}
        let program = Program {
            statements: vec![
                Statement::While(crate::parser::ast::WhileStatement {
                    condition: Expression::Identifier("x".to_string()),
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_while_valid_outer_variable() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x; while (x) {}
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Boolean(true),
                }),
                Statement::While(crate::parser::ast::WhileStatement {
                    condition: Expression::Identifier("x".to_string()),
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "While with valid outer variable should succeed");
    }

    #[test]
    fn test_while_body_referencing_outer_variable() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: let x; while (true) { x; }
        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "x".to_string(),
                    value: Expression::Integer(5),
                }),
                Statement::While(crate::parser::ast::WhileStatement {
                    condition: Expression::Boolean(true),
                    body: vec![
                        Statement::Expression(Expression::Identifier("x".to_string())),
                    ],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "While body referencing outer variable should succeed");
    }

    #[test]
    fn test_while_without_block_does_not_create_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: while (true) let x = 1; x;
        // With correct semantics: while does NOT create scope, so x IS visible
        let program = Program {
            statements: vec![
                Statement::While(crate::parser::ast::WhileStatement {
                    condition: Expression::Boolean(true),
                    body: vec![
                        Statement::Let(LetStatement {
                            name: "x".to_string(),
                            value: Expression::Integer(1),
                        }),
                    ],
                }),
                Statement::Expression(Expression::Identifier("x".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "While without block should NOT create scope - x should be visible");
    }

    #[test]
    fn test_while_with_block_does_create_scope() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: while (true) { let x = 1; } x;
        // Block creates scope, so x is NOT visible outside
        let program = Program {
            statements: vec![
                Statement::While(crate::parser::ast::WhileStatement {
                    condition: Expression::Boolean(true),
                    body: vec![
                        Statement::Block(vec![
                            Statement::Let(LetStatement {
                                name: "x".to_string(),
                                value: Expression::Integer(1),
                            }),
                        ]),
                    ],
                }),
                Statement::Expression(Expression::Identifier("x".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err(), "While with block SHOULD isolate - x should NOT be visible");
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_for_loop_scope_isolation() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: for i in [] {}; i;
        let program = Program {
            statements: vec![
                Statement::For(crate::parser::ast::ForStatement {
                    item: "i".to_string(),
                    list: Expression::Array(vec![]),
                    body: vec![],
                }),
                Statement::Expression(Expression::Identifier("i".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("i"));
    }

    #[test]
    fn test_for_loop_variable_visible_inside() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: for i in [] { i; }
        let program = Program {
            statements: vec![
                Statement::For(crate::parser::ast::ForStatement {
                    item: "i".to_string(),
                    list: Expression::Array(vec![]),
                    body: vec![
                        Statement::Expression(Expression::Identifier("i".to_string())),
                    ],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "For loop variable should be visible inside body");
    }

    #[test]
    fn test_nested_for_loops_valid() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: for i in [] { for j in [] {} }
        let program = Program {
            statements: vec![
                Statement::For(crate::parser::ast::ForStatement {
                    item: "i".to_string(),
                    list: Expression::Array(vec![]),
                    body: vec![
                        Statement::For(crate::parser::ast::ForStatement {
                            item: "j".to_string(),
                            list: Expression::Array(vec![]),
                            body: vec![
                                Statement::Expression(Expression::Identifier("i".to_string())),
                                Statement::Expression(Expression::Identifier("j".to_string())),
                            ],
                        }),
                    ],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Nested for loops with distinct variables should work");
    }

    #[test]
    fn test_for_with_undeclared_list_variable() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: for i in arr {}
        let program = Program {
            statements: vec![
                Statement::For(crate::parser::ast::ForStatement {
                    item: "i".to_string(),
                    list: Expression::Identifier("arr".to_string()),
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("arr"));
    }

    #[test]
    fn test_if_else_without_blocks_no_scope_isolation() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: if (true) let x = 1; else let y = 2; x; y;
        // Without blocks: both x and y should be visible after if/else
        let program = Program {
            statements: vec![
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Boolean(true),
                    consequence: vec![
                        Statement::Let(LetStatement {
                            name: "x".to_string(),
                            value: Expression::Integer(1),
                        }),
                    ],
                    alternative: Some(vec![
                        Statement::Let(LetStatement {
                            name: "y".to_string(),
                            value: Expression::Integer(2),
                        }),
                    ]),
                }),
                Statement::Expression(Expression::Identifier("x".to_string())),
                Statement::Expression(Expression::Identifier("y".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Without blocks, both x and y should be visible after if/else");
    }

    #[test]
    fn test_if_else_with_blocks_scope_isolated() {
        let mut analyzer = SemanticAnalyzer::new();

        // Program: if (true) { let x = 1; } else { let y = 2; } x;
        // With blocks: x should NOT be visible outside
        let program = Program {
            statements: vec![
                Statement::If(crate::parser::ast::IfStatement {
                    condition: Expression::Boolean(true),
                    consequence: vec![
                        Statement::Block(vec![
                            Statement::Let(LetStatement {
                                name: "x".to_string(),
                                value: Expression::Integer(1),
                            }),
                        ]),
                    ],
                    alternative: Some(vec![
                        Statement::Block(vec![
                            Statement::Let(LetStatement {
                                name: "y".to_string(),
                                value: Expression::Integer(2),
                            }),
                        ]),
                    ]),
                }),
                Statement::Expression(Expression::Identifier("x".to_string())),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undeclared identifier"));
        assert!(err.message.contains("x"));
    }

    // =========================================================================
    // FUNCTION TABLE TESTS
    // =========================================================================

    #[test]
    fn test_function_table_new_empty() {
        let table = FunctionTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_function_table_default_empty() {
        let table: FunctionTable = Default::default();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_function_table_register_single() {
        let mut table = FunctionTable::new();

        let func = FunctionDeclaration {
            name: "add".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            body: vec![],
        };

        let result = table.register(func);
        assert!(result.is_ok());
        assert_eq!(table.len(), 1);
        assert!(table.contains("add"));
    }

    #[test]
    fn test_function_table_register_zero_params() {
        let mut table = FunctionTable::new();

        let func = FunctionDeclaration {
            name: "greet".to_string(),
            params: vec![],
            body: vec![],
        };

        let result = table.register(func);
        assert!(result.is_ok());
        assert_eq!(table.len(), 1);
        assert!(table.contains("greet"));

        let retrieved = table.lookup("greet").unwrap();
        assert!(retrieved.params.is_empty());
    }

    #[test]
    fn test_function_table_register_duplicate_error() {
        let mut table = FunctionTable::new();

        let func1 = FunctionDeclaration {
            name: "add".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            body: vec![],
        };

        let func2 = FunctionDeclaration {
            name: "add".to_string(),
            params: vec!["x".to_string(), "y".to_string()],
            body: vec![],
        };

        table.register(func1).expect("First registration should succeed");
        let result = table.register(func2);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("add"));
        assert!(err.message.contains("already declared"));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_function_table_register_multiple() {
        let mut table = FunctionTable::new();

        let func1 = FunctionDeclaration {
            name: "add".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            body: vec![],
        };

        let func2 = FunctionDeclaration {
            name: "greet".to_string(),
            params: vec!["name".to_string()],
            body: vec![],
        };

        let func3 = FunctionDeclaration {
            name: "no_params".to_string(),
            params: vec![],
            body: vec![],
        };

        table.register(func1).unwrap();
        table.register(func2).unwrap();
        table.register(func3).unwrap();

        assert_eq!(table.len(), 3);
        assert!(table.contains("add"));
        assert!(table.contains("greet"));
        assert!(table.contains("no_params"));
    }

    #[test]
    fn test_function_table_lookup_existing() {
        let mut table = FunctionTable::new();

        let func = FunctionDeclaration {
            name: "calculate".to_string(),
            params: vec!["x".to_string()],
            body: vec![Statement::Return(crate::parser::ast::ReturnStatement { value: None })],
        };

        table.register(func.clone()).unwrap();

        let retrieved = table.lookup("calculate");
        assert!(retrieved.is_some());

        let arc_func = retrieved.unwrap();
        assert_eq!(arc_func.name, "calculate");
        assert_eq!(arc_func.params, vec!["x"]);
        assert_eq!(arc_func.body.len(), 1);
    }

    #[test]
    fn test_function_table_lookup_nonexistent() {
        let table = FunctionTable::new();
        let retrieved = table.lookup("nonexistent");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_function_table_contains() {
        let mut table = FunctionTable::new();

        let func = FunctionDeclaration {
            name: "test".to_string(),
            params: vec![],
            body: vec![],
        };

        assert!(!table.contains("test"));
        table.register(func).unwrap();
        assert!(table.contains("test"));
    }

    #[test]
    fn test_function_table_function_names() {
        let mut table = FunctionTable::new();

        table.register(FunctionDeclaration {
            name: "alpha".to_string(),
            params: vec![],
            body: vec![],
        }).unwrap();

        table.register(FunctionDeclaration {
            name: "beta".to_string(),
            params: vec![],
            body: vec![],
        }).unwrap();

        let names: Vec<&String> = table.function_names().collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&&"alpha".to_string()));
        assert!(names.contains(&&"beta".to_string()));
    }

    #[test]
    fn test_function_table_arc_preserves_body() {
        let mut table = FunctionTable::new();

        let body = vec![
            Statement::Let(LetStatement {
                name: "result".to_string(),
                value: Expression::Integer(42),
            }),
            Statement::Return(crate::parser::ast::ReturnStatement {
                value: Some(Expression::Identifier("result".to_string())),
            }),
        ];

        let func = FunctionDeclaration {
            name: "compute".to_string(),
            params: vec!["input".to_string()],
            body,
        };

        table.register(func).unwrap();

        let retrieved = table.lookup("compute").unwrap();
        assert_eq!(retrieved.body.len(), 2);
    }

    // =========================================================================
    // SEMANTIC ANALYZER FUNCTION REGISTRATION TESTS
    // =========================================================================

    #[test]
    fn test_analyze_single_function_declaration() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "add".to_string(),
                    params: vec!["a".to_string(), "b".to_string()],
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        assert_eq!(analyzer.function_table().len(), 1);
        assert!(analyzer.function_table().contains("add"));
    }

    #[test]
    fn test_analyze_zero_parameter_function() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "greet".to_string(),
                    params: vec![],
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        assert_eq!(analyzer.function_table().len(), 1);

        let func = analyzer.function_table().lookup("greet").unwrap();
        assert!(func.params.is_empty());
    }

    #[test]
    fn test_analyze_multiple_function_declarations() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "add".to_string(),
                    params: vec!["a".to_string(), "b".to_string()],
                    body: vec![],
                }),
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "greet".to_string(),
                    params: vec!["name".to_string()],
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        assert_eq!(analyzer.function_table().len(), 2);
        assert!(analyzer.function_table().contains("add"));
        assert!(analyzer.function_table().contains("greet"));
    }

    #[test]
    fn test_analyze_duplicate_function_error() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "add".to_string(),
                    params: vec!["a".to_string(), "b".to_string()],
                    body: vec![],
                }),
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "add".to_string(),
                    params: vec!["x".to_string(), "y".to_string()],
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("add"));
        assert!(err.message.contains("already declared"));
    }

    #[test]
    fn test_analyze_empty_body_function() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "empty".to_string(),
                    params: vec![],
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        assert_eq!(analyzer.function_table().len(), 1);

        let func = analyzer.function_table().lookup("empty").unwrap();
        assert!(func.body.is_empty());
    }

    #[test]
    fn test_analyze_function_with_body_content() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "compute".to_string(),
                    params: vec!["x".to_string()],
                    body: vec![
                        Statement::Return(crate::parser::ast::ReturnStatement {
                            value: Some(Expression::Integer(42)),
                        }),
                    ],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        assert_eq!(analyzer.function_table().len(), 1);

        let func = analyzer.function_table().lookup("compute").unwrap();
        assert_eq!(func.body.len(), 1);
    }

    #[test]
    fn test_analyze_mixed_program_functions_and_variables() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::Let(LetStatement {
                    name: "global_var".to_string(),
                    value: Expression::Integer(10),
                }),
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "helper".to_string(),
                    params: vec![],
                    body: vec![],
                }),
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![],
                }),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        assert_eq!(analyzer.function_table().len(), 2);
        assert!(analyzer.function_table().contains("helper"));
        assert!(analyzer.function_table().contains("main"));
    }

    // =========================================================================
    // NATIVE FUNCTION REGISTRY INTEGRATION TESTS
    // =========================================================================

    #[test]
    fn test_native_function_call_correct_args_ok() {
        let mut builder = crate::native::NativeFunctionRegistryBuilder::new();
        builder.register("foo", 2).expect("register should succeed");
        let registry = std::sync::Arc::new(builder.build());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry);

        let program = Program {
            statements: vec![
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "foo".to_string(),
                    arguments: vec![
                        Expression::Integer(1),
                        Expression::Integer(2),
                    ],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "Native function call with correct args should succeed");
    }

    #[test]
    fn test_native_function_call_wrong_arg_count_error() {
        let mut builder = crate::native::NativeFunctionRegistryBuilder::new();
        builder.register("bar", 1).expect("register should succeed");
        let registry = std::sync::Arc::new(builder.build());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry);

        let program = Program {
            statements: vec![
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "bar".to_string(),
                    arguments: vec![
                        Expression::Integer(1),
                        Expression::Integer(2),
                    ],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("bar"));
        assert!(err.message.contains("expects 1"));
        assert!(err.message.contains("got 2"));
    }

    #[test]
    fn test_unknown_function_error() {
        let registry = std::sync::Arc::new(crate::native::NativeFunctionRegistry::empty());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry);

        let program = Program {
            statements: vec![
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "nonexistent".to_string(),
                    arguments: vec![],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown function"));
        assert!(err.message.contains("nonexistent"));
    }

    #[test]
    fn test_user_function_call_with_empty_native_registry_ok() {
        let registry = std::sync::Arc::new(crate::native::NativeFunctionRegistry::empty());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry);

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "add".to_string(),
                    params: vec!["a".to_string(), "b".to_string()],
                    body: vec![],
                }),
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "add".to_string(),
                    arguments: vec![
                        Expression::Integer(3),
                        Expression::Integer(4),
                    ],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok(), "User function call with empty native registry should succeed");
    }

    #[test]
    fn test_user_function_wrong_arg_count_error() {
        let registry = std::sync::Arc::new(crate::native::NativeFunctionRegistry::empty());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry);

        let program = Program {
            statements: vec![
                Statement::FunctionDeclaration(FunctionDeclaration {
                    name: "add".to_string(),
                    params: vec!["a".to_string(), "b".to_string()],
                    body: vec![],
                }),
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "add".to_string(),
                    arguments: vec![Expression::Integer(3)],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("add"));
        assert!(err.message.contains("expects 2"));
        assert!(err.message.contains("got 1"));
    }

    #[test]
    fn test_analyzer_without_native_registry_unknown_function_error() {
        let mut analyzer = SemanticAnalyzer::new();

        let program = Program {
            statements: vec![
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "unknown".to_string(),
                    arguments: vec![],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown function"));
    }

    #[test]
    fn test_native_function_zero_args_ok() {
        let mut builder = crate::native::NativeFunctionRegistryBuilder::new();
        builder.register("noargs", 0).expect("register should succeed");
        let registry = std::sync::Arc::new(builder.build());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry);

        let program = Program {
            statements: vec![
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "noargs".to_string(),
                    arguments: vec![],
                })),
            ],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deterministic_native_validation() {
        let mut builder = crate::native::NativeFunctionRegistryBuilder::new();
        builder.register("det", 1).expect("register should succeed");
        let registry = std::sync::Arc::new(builder.build());
        let mut analyzer = SemanticAnalyzer::with_native_registry(registry.clone());

        let program = Program {
            statements: vec![
                Statement::Expression(Expression::FunctionCall(FunctionCallExpression {
                    name: "det".to_string(),
                    arguments: vec![Expression::Integer(42)],
                })),
            ],
        };

        let r1 = analyzer.analyze_program(&program);
        let mut analyzer2 = SemanticAnalyzer::with_native_registry(registry);
        let r2 = analyzer2.analyze_program(&program);
        assert_eq!(r1.is_ok(), r2.is_ok());
        assert!(r1.is_ok());
    }

    #[test]
    fn test_function_table_clone_preserves_data() {
        let mut table = FunctionTable::new();

        table.register(FunctionDeclaration {
            name: "original".to_string(),
            params: vec!["p".to_string()],
            body: vec![],
        }).unwrap();

        let cloned = table.clone();

        assert_eq!(cloned.len(), 1);
        assert!(cloned.contains("original"));

        // Modifying original should not affect clone
        table.register(FunctionDeclaration {
            name: "new".to_string(),
            params: vec![],
            body: vec![],
        }).unwrap();

        assert_eq!(table.len(), 2);
        assert_eq!(cloned.len(), 1);
        assert!(!cloned.contains("new"));
    }
}

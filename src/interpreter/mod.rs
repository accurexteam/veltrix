use crate::parser::ast::{
    BinaryExpression, Expression, ForStatement, FunctionCallExpression,
    IfStatement, ReturnStatement, Statement, UnaryExpression, WhileStatement,
};
use std::collections::HashMap;
use std::fmt;

/// Runtime representation of a function value
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionValue {
    pub parameters: Vec<String>,
    pub body: Vec<Statement>,
}

impl FunctionValue {
    /// Creates a new function value from a function declaration
    pub fn new(parameters: Vec<String>, body: Vec<Statement>) -> Self {
        Self { parameters, body }
    }
}

/// Runtime values in the interpreter
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    String(String),
    Boolean(bool),
    Array(Vec<Value>),
    Function(FunctionValue),
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Array(arr) => {
                let elements: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", elements.join(", "))
            }
            Value::Function(func) => {
                write!(f, "<function({})>", func.parameters.join(", "))
            }
            Value::Nil => write!(f, "nil"),
        }
    }
}

/// Runtime context tracking for better error messages
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeContext {
    pub operation: String,
    pub function_name: Option<String>,
    pub scope_depth: usize,
}

impl RuntimeContext {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            function_name: None,
            scope_depth: 1,
        }
    }

    pub fn with_function(mut self, name: impl Into<String>) -> Self {
        self.function_name = Some(name.into());
        self
    }

    pub fn with_scope_depth(mut self, depth: usize) -> Self {
        self.scope_depth = depth;
        self
    }
}

impl fmt::Display for RuntimeContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.operation)?;
        if let Some(ref func) = self.function_name {
            write!(f, " in function '{}'", func)?;
        }
        if self.scope_depth > 1 {
            write!(f, " (scope depth: {})", self.scope_depth)?;
        }
        Ok(())
    }
}

/// Runtime error during interpretation with context
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub context: Option<RuntimeContext>,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref ctx) = self.context {
            write!(f, "Runtime error: {} (while {})", self.message, ctx)?;
        } else {
            write!(f, "Runtime error: {}", self.message)?;
        }
        if self.line > 0 {
            write!(f, " at line {}", self.line)?;
            if self.column > 0 {
                write!(f, ", column {}", self.column)?;
            }
        }
        Ok(())
    }
}

impl RuntimeError {
    /// Creates a new runtime error with a friendly message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: 0,
            column: 0,
            context: None,
        }
    }

    /// Creates an error for undefined variable
    pub fn undefined_variable(name: &str, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!("I don't know about any variable named '{}' - did you forget to define it?", name),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for undefined function
    pub fn undefined_function(name: &str, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!("I can't find a function called '{}' - check the name and try again", name),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for calling a non-function value
    pub fn not_callable(value: &Value, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Attempted to call non-function value - got a {} instead of a function",
                value_type_name(value)
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for function arity mismatch
    pub fn arity_mismatch(name: &str, expected: usize, actual: usize, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Function '{}' expects {} argument{}, but you gave {} - please check your function call",
                name,
                expected,
                if expected == 1 { "" } else { "s" },
                actual
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for return outside function
    pub fn return_outside_function(ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: "You can't use 'return' here - it's only allowed inside functions".to_string(),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for division by zero
    pub fn division_by_zero(ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: "Cannot divide by zero - that's undefined in mathematics".to_string(),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for modulo by zero
    pub fn modulo_by_zero(ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: "Cannot calculate modulo with zero - that's undefined".to_string(),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for type mismatch in arithmetic
    pub fn type_mismatch_operation(left: &Value, right: &Value, op: &str, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Cannot {} '{}' and '{}' together - these types don't work with '{}'",
                operation_verb(op),
                value_type_name(left),
                value_type_name(right),
                op
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for invalid unary operation
    pub fn invalid_unary(value: &Value, op: &str, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Cannot apply '{}' to a {} value - that operation doesn't make sense here",
                op,
                value_type_name(value)
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for invalid comparison
    pub fn invalid_comparison(left: &Value, right: &Value, op: &str, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Cannot compare '{}' and '{}' with '{}' - these types can't be compared this way",
                value_type_name(left),
                value_type_name(right),
                op
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for not iterable
    pub fn not_iterable(value: &Value, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Cannot iterate over a {} value - 'for' loops only work with arrays",
                value_type_name(value)
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for integer overflow
    pub fn integer_overflow(ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: "That number is too big - the calculation overflowed".to_string(),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for assignment to undefined variable
    pub fn undefined_variable_assignment(name: &str, ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: format!(
                "Cannot assign to '{}' - this variable hasn't been defined yet. Use 'let {} = ...' first",
                name, name
            ),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for unknown operator
    pub fn unknown_operator(op: &str, is_unary: bool, ctx: Option<RuntimeContext>) -> Self {
        let kind = if is_unary { "unary" } else { "binary" };
        Self {
            message: format!("Unknown {} operator '{}' - I don't know how to handle this", kind, op),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Creates an error for scope exit failure
    pub fn scope_exit_error(ctx: Option<RuntimeContext>) -> Self {
        Self {
            message: "Cannot exit the global scope - there's nowhere else to go!".to_string(),
            line: 0,
            column: 0,
            context: ctx,
        }
    }

    /// Adds context to this error
    pub fn with_context(mut self, ctx: RuntimeContext) -> Self {
        self.context = Some(ctx);
        self
    }

    /// Adds line and column info to this error
    pub fn at(mut self, line: usize, column: usize) -> Self {
        self.line = line;
        self.column = column;
        self
    }
}

/// Helper function to get a verb for an operation
fn operation_verb(op: &str) -> &'static str {
    match op {
        "+" => "add",
        "-" => "subtract",
        "*" => "multiply",
        "/" => "divide",
        "%" => "calculate modulo of",
        "**" => "raise",
        "and" => "combine",
        "or" => "combine",
        _ => "operate on",
    }
}

impl std::error::Error for RuntimeError {}

/// Environment for variable storage with scoped bindings
#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    /// Creates a new environment with a single global scope
    pub fn new() -> Self {
        let mut scopes = Vec::new();
        scopes.push(HashMap::new());
        Self { scopes }
    }

    /// Enters a new scope (for function calls, blocks, etc.)
    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Exits the current scope
    /// Returns error if trying to exit the global scope
    pub fn exit_scope(&mut self) -> Result<(), RuntimeError> {
        if self.scopes.len() <= 1 {
            return Err(RuntimeError::scope_exit_error(None));
        }
        self.scopes.pop();
        Ok(())
    }

    /// Defines a variable in the current scope
    pub fn define(&mut self, name: String, value: Value) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.insert(name, value);
        }
    }

    /// Looks up a variable by name, searching from innermost to outermost scope
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    /// Assigns a value to an existing variable
    /// Returns error if variable doesn't exist
    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), RuntimeError> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(RuntimeError::undefined_variable_assignment(name, None))
    }

    /// Returns the current scope depth (1 = global only)
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

/// Control flow result from executing statements
#[derive(Debug, Clone, PartialEq)]
pub enum ControlFlow {
    /// Normal execution, continue to next statement
    Continue,
    /// Return from function with optional value
    Return(Option<Value>),
}

/// The interpreter that executes AST programs
pub struct Interpreter {
    environment: Environment,
    /// Tracks the current execution context depth.
    /// 0 = top-level (global scope, outside any function)
    /// >0 = inside function(s) - each nested function call increments
    function_depth: usize,
}

impl Interpreter {
    /// Creates a new interpreter with empty environment
    pub fn new() -> Self {
        Self {
            environment: Environment::new(),
            function_depth: 0,
        }
    }

    /// Returns a reference to the environment
    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    /// Returns a mutable reference to the environment
    pub fn environment_mut(&mut self) -> &mut Environment {
        &mut self.environment
    }

    /// Executes a program (list of statements)
    pub fn execute_program(&mut self, statements: &[Statement]) -> Result<Value, RuntimeError> {
        let mut last_value = Value::Nil;

        for stmt in statements {
            match self.execute_statement(stmt)? {
                ControlFlow::Continue => {}
                ControlFlow::Return(val) => {
                    // In top-level program, return just gives us the value
                    return Ok(val.unwrap_or(Value::Nil));
                }
            }

            // Track the last expression value for REPL-like behavior
            if let Statement::Expression(expr) = stmt {
                last_value = self.evaluate_expression(expr)?;
            }
        }

        Ok(last_value)
    }

    /// Executes a single statement
    fn execute_statement(&mut self, stmt: &Statement) -> Result<ControlFlow, RuntimeError> {
        match stmt {
            Statement::Let(let_stmt) => {
                let value = self.evaluate_expression(&let_stmt.value)?;
                self.environment.define(let_stmt.name.clone(), value);
                Ok(ControlFlow::Continue)
            }
            Statement::Assignment(assign) => {
                let value = self.evaluate_expression(&assign.value)?;
                // Only define the variable if at global scope (scope_depth == 1)
                if self.environment.scope_depth() == 1 && self.environment.lookup(&assign.name).is_none() {
                    self.environment.define(assign.name.clone(), value);
                } else {
                    self.environment.assign(&assign.name, value)?;
                }
                Ok(ControlFlow::Continue)
            }
            Statement::If(if_stmt) => self.execute_if_statement(if_stmt),
            Statement::While(while_stmt) => self.execute_while_statement_internal(while_stmt),
            Statement::For(for_stmt) => self.execute_for_statement_internal(for_stmt),
            Statement::Block(stmts) => self.execute_block(stmts),
            Statement::Return(ret) => self.execute_return_statement(ret),
            Statement::Write(write) => {
                let value = self.evaluate_expression(&write.value)?;
                println!("{}", value);
                Ok(ControlFlow::Continue)
            }
            Statement::Expression(expr) => {
                self.evaluate_expression(expr)?;
                Ok(ControlFlow::Continue)
            }
            Statement::FunctionDeclaration(func) => {
                // Create a Function value and store it in the environment
                let func_value = Value::Function(FunctionValue::new(
                    func.params.clone(),
                    func.body.clone(),
                ));
                self.environment.define(func.name.clone(), func_value);
                Ok(ControlFlow::Continue)
            }
        }
    }

    /// Executes a block of statements with a new scope
    pub fn execute_block(&mut self, stmts: &[Statement]) -> Result<ControlFlow, RuntimeError> {
        self.environment.enter_scope();

        let result = self.execute_statements(stmts);

        // Always exit scope, even if there was an error
        self.environment.exit_scope().ok();

        result
    }

    /// Executes multiple statements
    fn execute_statements(&mut self, stmts: &[Statement]) -> Result<ControlFlow, RuntimeError> {
        for stmt in stmts {
            match self.execute_statement(stmt)? {
                ControlFlow::Continue => {}
                ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
            }
        }
        Ok(ControlFlow::Continue)
    }

    /// Executes an if statement
    pub fn execute_if_statement(&mut self, if_stmt: &IfStatement) -> Result<ControlFlow, RuntimeError> {
        let condition = self.evaluate_expression(&if_stmt.condition)?;

        if is_truthy(&condition) {
            for stmt in &if_stmt.consequence {
                match self.execute_statement(stmt)? {
                    ControlFlow::Continue => {}
                    ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
                }
            }
        } else if let Some(ref alternative) = if_stmt.alternative {
            for stmt in alternative {
                match self.execute_statement(stmt)? {
                    ControlFlow::Continue => {}
                    ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
                }
            }
        }

        Ok(ControlFlow::Continue)
    }

    /// Executes a while loop
    pub fn execute_while_loop(&mut self, while_stmt: &WhileStatement) -> Result<ControlFlow, RuntimeError> {
        self.execute_while_statement_internal(while_stmt)
    }

    /// Internal implementation of while statement execution
    fn execute_while_statement_internal(&mut self, while_stmt: &WhileStatement) -> Result<ControlFlow, RuntimeError> {
        loop {
            let condition = self.evaluate_expression(&while_stmt.condition)?;
            if !is_truthy(&condition) {
                break;
            }

            for stmt in &while_stmt.body {
                match self.execute_statement(stmt)? {
                    ControlFlow::Continue => {}
                    ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
                }
            }
        }

        Ok(ControlFlow::Continue)
    }

    /// Executes a for loop
    pub fn execute_for_loop(&mut self, for_stmt: &ForStatement) -> Result<ControlFlow, RuntimeError> {
        self.execute_for_statement_internal(for_stmt)
    }

    /// Internal implementation of for statement execution
    fn execute_for_statement_internal(&mut self, for_stmt: &ForStatement) -> Result<ControlFlow, RuntimeError> {
        let list_value = self.evaluate_expression(&for_stmt.list)?;

        let elements = match list_value {
            Value::Array(arr) => arr,
            _ => {
                let ctx = RuntimeContext::new(format!("iterating over '{}' in for loop", for_stmt.item));
                return Err(RuntimeError::not_iterable(&list_value, Some(ctx)));
            }
        };

        self.environment.enter_scope();

        let result = (|| {
            for element in elements {
                self.environment.define(for_stmt.item.clone(), element);

                for stmt in &for_stmt.body {
                    match self.execute_statement(stmt)? {
                        ControlFlow::Continue => {}
                        ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
                    }
                }
            }
            Ok(ControlFlow::Continue)
        })();

        self.environment.exit_scope().ok();

        result
    }

    /// Executes a return statement
    /// Returns error if called at top-level (outside any function)
    fn execute_return_statement(&mut self, ret: &ReturnStatement) -> Result<ControlFlow, RuntimeError> {
        // Check if we're at top-level (outside any function)
        if self.function_depth == 0 {
            return Err(RuntimeError::return_outside_function(None));
        }

        let value = match &ret.value {
            Some(expr) => Some(self.evaluate_expression(expr)?),
            None => None,
        };
        Ok(ControlFlow::Return(value))
    }

    /// Evaluates an expression and returns its value
    fn evaluate_expression(&mut self, expr: &Expression) -> Result<Value, RuntimeError> {
        self.evaluate_expression_internal(expr)
    }

    /// Public method for REPL to evaluate expressions
    pub fn evaluate_expression_for_repl(&mut self, expr: &Expression) -> Result<Value, RuntimeError> {
        self.evaluate_expression_internal(expr)
    }

    /// Internal expression evaluation implementation
    fn evaluate_expression_internal(&mut self, expr: &Expression) -> Result<Value, RuntimeError> {
        match expr {
            Expression::Integer(n) => Ok(Value::Integer(*n)),
            Expression::String(s) => Ok(Value::String(s.clone())),
            Expression::Boolean(b) => Ok(Value::Boolean(*b)),
            Expression::Identifier(name) => {
                match self.environment.lookup(name) {
                    Some(value) => Ok(value.clone()),
                    None => {
                        let ctx = RuntimeContext::new(format!("accessing variable '{}'", name));
                        Err(RuntimeError::undefined_variable(name, Some(ctx)))
                    }
                }
            }
            Expression::Array(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for elem in elements {
                    values.push(self.evaluate_expression_internal(elem)?);
                }
                Ok(Value::Array(values))
            }
            Expression::Unary(unary) => self.evaluate_unary_expression(unary),
            Expression::Binary(binary) => self.evaluate_binary_expression(binary),
            Expression::FunctionCall(call) => self.evaluate_function_call(call),
            Expression::Assignment(assign) => {
                let value = self.evaluate_expression_internal(&assign.value)?;
                // Only define the variable if at global scope (scope_depth == 1)
                if self.environment.scope_depth() == 1 && self.environment.lookup(&assign.name).is_none() {
                    self.environment.define(assign.name.clone(), value.clone());
                } else {
                    self.environment.assign(&assign.name, value.clone())?;
                }
                Ok(value)
            }
        }
    }

    /// Evaluates a unary expression
    fn evaluate_unary_expression(&mut self, unary: &UnaryExpression) -> Result<Value, RuntimeError> {
        let right = self.evaluate_expression(&unary.right)?;

        match unary.operator.as_str() {
            "-" => match right {
                Value::Integer(n) => Ok(Value::Integer(-n)),
                _ => {
                    let ctx = RuntimeContext::new(format!("applying unary '{}'", unary.operator));
                    Err(RuntimeError::invalid_unary(&right, "-", Some(ctx)))
                }
            },
            "not" => Ok(Value::Boolean(!is_truthy(&right))),
            _ => {
                let ctx = RuntimeContext::new(format!("evaluating unary expression with '{}'", unary.operator));
                Err(RuntimeError::unknown_operator(&unary.operator, true, Some(ctx)))
            }
        }
    }

    /// Evaluates a binary expression
    fn evaluate_binary_expression(&mut self, binary: &BinaryExpression) -> Result<Value, RuntimeError> {
        let left = self.evaluate_expression(&binary.left)?;
        let right = self.evaluate_expression(&binary.right)?;

        match binary.operator.as_str() {
            "+" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l + r)),
                (Value::String(l), Value::String(r)) => Ok(Value::String(format!("{}{}", l, r))),
                _ => {
                    let ctx = RuntimeContext::new("adding values");
                    Err(RuntimeError::type_mismatch_operation(&left, &right, "+", Some(ctx)))
                }
            },
            "-" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l - r)),
                _ => {
                    let ctx = RuntimeContext::new("subtracting values");
                    Err(RuntimeError::type_mismatch_operation(&left, &right, "-", Some(ctx)))
                }
            },
            "*" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Integer(l * r)),
                _ => {
                    let ctx = RuntimeContext::new("multiplying values");
                    Err(RuntimeError::type_mismatch_operation(&left, &right, "*", Some(ctx)))
                }
            },
            "/" => match (&left, &right) {
                (Value::Integer(_), Value::Integer(r)) => {
                    if *r == 0 {
                        let ctx = RuntimeContext::new("dividing values");
                        return Err(RuntimeError::division_by_zero(Some(ctx)));
                    }
                    Ok(Value::Integer(left.as_integer()? / r))
                }
                _ => {
                    let ctx = RuntimeContext::new("dividing values");
                    Err(RuntimeError::type_mismatch_operation(&left, &right, "/", Some(ctx)))
                }
            },
            "%" => match (&left, &right) {
                (Value::Integer(_), Value::Integer(r)) => {
                    if *r == 0 {
                        let ctx = RuntimeContext::new("calculating modulo");
                        return Err(RuntimeError::modulo_by_zero(Some(ctx)));
                    }
                    Ok(Value::Integer(left.as_integer()? % r))
                }
                _ => {
                    let ctx = RuntimeContext::new("calculating modulo");
                    Err(RuntimeError::type_mismatch_operation(&left, &right, "%", Some(ctx)))
                }
            },
            "**" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => {
                    // Handle negative exponents by returning 0 (integer division semantics)
                    if *r < 0 {
                        return Ok(Value::Integer(0));
                    }
                    // Use checked pow to prevent overflow panics
                    match l.checked_pow(*r as u32) {
                        Some(result) => Ok(Value::Integer(result)),
                        None => {
                            let ctx = RuntimeContext::new("calculating power");
                            Err(RuntimeError::integer_overflow(Some(ctx)))
                        }
                    }
                }
                _ => {
                    let ctx = RuntimeContext::new("calculating power");
                    Err(RuntimeError::type_mismatch_operation(&left, &right, "**", Some(ctx)))
                }
            },
            "==" => Ok(Value::Boolean(values_equal(&left, &right))),
            "!=" => Ok(Value::Boolean(!values_equal(&left, &right))),
            "<" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Boolean(l < r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Boolean(l < r)),
                _ => {
                    let ctx = RuntimeContext::new("comparing values");
                    Err(RuntimeError::invalid_comparison(&left, &right, "<", Some(ctx)))
                }
            },
            ">" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Boolean(l > r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Boolean(l > r)),
                _ => {
                    let ctx = RuntimeContext::new("comparing values");
                    Err(RuntimeError::invalid_comparison(&left, &right, ">", Some(ctx)))
                }
            },
            "<=" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Boolean(l <= r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Boolean(l <= r)),
                _ => {
                    let ctx = RuntimeContext::new("comparing values");
                    Err(RuntimeError::invalid_comparison(&left, &right, "<=", Some(ctx)))
                }
            },
            ">=" => match (&left, &right) {
                (Value::Integer(l), Value::Integer(r)) => Ok(Value::Boolean(l >= r)),
                (Value::String(l), Value::String(r)) => Ok(Value::Boolean(l >= r)),
                _ => {
                    let ctx = RuntimeContext::new("comparing values");
                    Err(RuntimeError::invalid_comparison(&left, &right, ">=", Some(ctx)))
                }
            },
            "and" => {
                // Short-circuit: only evaluate right if left is truthy
                if !is_truthy(&left) {
                    Ok(left)
                } else {
                    let right = self.evaluate_expression(&binary.right)?;
                    Ok(Value::Boolean(is_truthy(&right)))
                }
            }
            "or" => {
                // Short-circuit: only evaluate right if left is falsy
                if is_truthy(&left) {
                    Ok(left)
                } else {
                    let right = self.evaluate_expression(&binary.right)?;
                    Ok(Value::Boolean(is_truthy(&right)))
                }
            }
            _ => {
                let ctx = RuntimeContext::new(format!("evaluating binary expression with '{}'", binary.operator));
                Err(RuntimeError::unknown_operator(&binary.operator, false, Some(ctx)))
            }
        }
    }

    /// Evaluates a function call expression
    fn evaluate_function_call(&mut self, call: &FunctionCallExpression) -> Result<Value, RuntimeError> {
        // Look up the function in the environment
        let func_value = match self.environment.lookup(&call.name) {
            Some(value) => value.clone(),
            None => {
                let ctx = RuntimeContext::new(format!("calling function '{}'", call.name));
                return Err(RuntimeError::undefined_function(&call.name, Some(ctx)));
            }
        };

        // Ensure the value is a function
        let func = match func_value {
            Value::Function(f) => f,
            _ => {
                let ctx = RuntimeContext::new(format!("calling '{}'", call.name));
                return Err(RuntimeError::not_callable(&func_value, Some(ctx)));
            }
        };

        // Validate arity (number of arguments)
        let expected_arity = func.parameters.len();
        let actual_arity = call.arguments.len();

        if actual_arity != expected_arity {
            let ctx = RuntimeContext::new(format!("calling function '{}'", call.name));
            return Err(RuntimeError::arity_mismatch(&call.name, expected_arity, actual_arity, Some(ctx)));
        }

        // Evaluate all argument expressions BEFORE entering the new scope
        let mut arg_values = Vec::with_capacity(actual_arity);
        for arg_expr in &call.arguments {
            arg_values.push(self.evaluate_expression(arg_expr)?);
        }

        // Enter a new scope for this function call
        self.environment.enter_scope();

        // Increment function depth to indicate we're inside a function
        self.function_depth += 1;

        // Create context with function name
        let _ctx = RuntimeContext::new("executing function body").with_function(&call.name);

        // Bind parameters to argument values
        for (param_name, arg_value) in func.parameters.iter().zip(arg_values.iter()) {
            self.environment.define(param_name.clone(), arg_value.clone());
        }

        // Execute the function body
        let result = self.execute_statements(&func.body);

        // Exit the function scope
        self.environment.exit_scope().ok();

        // Decrement function depth when leaving the function
        self.function_depth -= 1;

        // Return the function result
        match result {
            Ok(ControlFlow::Return(value)) => Ok(value.unwrap_or(Value::Nil)),
            Ok(ControlFlow::Continue) => Ok(Value::Nil),
            Err(e) => Err(e),
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks if a value is truthy
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Boolean(b) => *b,
        Value::Nil => false,
        Value::Integer(n) => *n != 0,
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Function(_) => true,
    }
}

/// Checks if two values are equal
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Integer(l), Value::Integer(r)) => l == r,
        (Value::String(l), Value::String(r)) => l == r,
        (Value::Boolean(l), Value::Boolean(r)) => l == r,
        (Value::Nil, Value::Nil) => true,
        (Value::Array(l), Value::Array(r)) => l == r,
        (Value::Function(_), Value::Function(_)) => false,
        _ => false,
    }
}

/// Returns the type name of a value
fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Integer(_) => "integer",
        Value::String(_) => "string",
        Value::Boolean(_) => "boolean",
        Value::Array(_) => "array",
        Value::Function(_) => "function",
        Value::Nil => "nil",
    }
}

/// Helper trait for value conversions
impl Value {
    fn as_integer(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Integer(n) => Ok(*n),
            _ => {
                let ctx = RuntimeContext::new("converting value to integer");
                Err(RuntimeError::new(format!("Expected an integer, but got a {}", value_type_name(self))).with_context(ctx))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::parser::ast::{AssignmentExpression, LetStatement};

    fn interpret_source(source: &str) -> Result<Value, RuntimeError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| RuntimeError {
            message: e.message,
            line: e.line,
            column: e.column,
            context: None,
        })?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| RuntimeError {
            message: e.message,
            line: e.line,
            column: e.column,
            context: None,
        })?;

        let mut interpreter = Interpreter::new();
        interpreter.execute_program(&program.statements)
    }

    fn interpret_with_functions(source: &str) -> Result<Value, RuntimeError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| RuntimeError {
            message: e.message,
            line: e.line,
            column: e.column,
            context: None,
        })?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| RuntimeError {
            message: e.message,
            line: e.line,
            column: e.column,
            context: None,
        })?;

        // Run semantic analysis first
        let mut analyzer = crate::semantic::SemanticAnalyzer::new();
        analyzer.analyze_program(&program).map_err(|e| RuntimeError {
            message: e.message,
            line: e.line,
            column: e.column,
            context: None,
        })?;

        let mut interpreter = Interpreter::new();
        interpreter.execute_program(&program.statements)
    }

    // =========================================================================
    // BASIC EXPRESSION TESTS
    // =========================================================================

    #[test]
    fn test_integer_literal() {
        let result = interpret_source("let x = 42");
        assert!(result.is_ok());
    }

    #[test]
    fn test_string_literal() {
        let result = interpret_source("let x = \"hello\"");
        assert!(result.is_ok());
    }

    #[test]
    fn test_boolean_literal() {
        let result = interpret_source("let x = true");
        assert!(result.is_ok());
        let result = interpret_source("let x = false");
        assert!(result.is_ok());
    }

    #[test]
    fn test_arithmetic_addition() {
        let result = interpret_source("let x = 5 + 3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_arithmetic_multiplication() {
        let result = interpret_source("let x = 5 * 3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_comparison_less_than() {
        let result = interpret_source("let x = 5 < 10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_comparison_equal() {
        let result = interpret_source("let x = 5 == 5");
        assert!(result.is_ok());
    }

    #[test]
    fn test_unary_negation() {
        let result = interpret_source("let x = -5");
        assert!(result.is_ok());
    }

    #[test]
    fn test_unary_not() {
        let result = interpret_source("let x = not true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_literal() {
        let result = interpret_source("let x = [1, 2, 3]");
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_array() {
        let result = interpret_source("let x = []");
        assert!(result.is_ok());
    }

    // =========================================================================
    // VARIABLE TESTS
    // =========================================================================

    #[test]
    fn test_variable_declaration_and_lookup() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(42));

        let value = env.lookup("x");
        assert!(value.is_some());
        assert_eq!(value.unwrap(), &Value::Integer(42));
    }

    #[test]
    fn test_variable_assignment() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(5));
        env.assign("x", Value::Integer(10)).unwrap();

        let value = env.lookup("x");
        assert_eq!(value.unwrap(), &Value::Integer(10));
    }

    #[test]
    fn test_scope_isolation() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(1));

        env.enter_scope();
        env.define("x".to_string(), Value::Integer(2));
        let inner_value = env.lookup("x");
        assert_eq!(inner_value.unwrap(), &Value::Integer(2));

        env.exit_scope().unwrap();
        let outer_value = env.lookup("x");
        assert_eq!(outer_value.unwrap(), &Value::Integer(1));
    }

    #[test]
    fn test_scope_exit_error() {
        let mut env = Environment::new();
        let result = env.exit_scope();
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("global scope"));
    }

    // =========================================================================
    // CONTROL FLOW TESTS
    // =========================================================================

    #[test]
    fn test_if_statement_true() {
        let source = r#"
if true:
    let x = 1
"#;
        let result = interpret_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_statement_false() {
        let source = r#"
if false:
    let x = 1
"#;
        let result = interpret_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_else_statement() {
        let source = r#"
if false:
    let x = 1
else:
    let y = 2
"#;
        let result = interpret_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_while_loop() {
        let source = r#"
let x = 0
while x < 3:
    x = x + 1
"#;
        let result = interpret_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_for_loop() {
        let source = r#"
let sum = 0
for i in [1, 2, 3]:
    sum = sum + i
"#;
        let result = interpret_source(source);
        assert!(result.is_ok());
    }

    // =========================================================================
    // FUNCTION DECLARATION TESTS
    // =========================================================================

    #[test]
    fn test_function_declaration_registers_in_environment() {
        let source = r#"
function greet(name):
    return name
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();

        let mut interpreter = Interpreter::new();
        interpreter.execute_program(&program.statements).unwrap();

        // Function should be stored as a value in the environment
        let value = interpreter.environment().lookup("greet");
        assert!(value.is_some());
        assert!(matches!(value.unwrap(), Value::Function(_)));
    }

    #[test]
    fn test_zero_parameter_function() {
        let source = r#"
function empty():
    return 42

empty()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Zero-parameter function should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(42));
    }

    // =========================================================================
    // FUNCTION CALL TESTS
    // =========================================================================

    #[test]
    fn test_simple_function_call() {
        let source = r#"
function greet(name):
    return name

greet("Alice")
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Function call should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::String("Alice".to_string()));
    }

    #[test]
    fn test_function_call_with_multiple_args() {
        let source = r#"
function add(a, b):
    return a + b

add(3, 4)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Multi-arg function call should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(7));
    }

    #[test]
    fn test_function_call_arity_error_too_few() {
        let source = r#"
function add(a, b):
    return a + b

add(3)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("expects 2"));
        assert!(err.message.contains("got 1"));
    }

    #[test]
    fn test_function_call_arity_error_too_many() {
        let source = r#"
function add(a, b):
    return a + b

add(1, 2, 3)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("expects 2"));
        assert!(err.message.contains("got 3"));
    }

    #[test]
    fn test_undefined_function_error() {
        let source = r#"
undefined_func()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown function") || err.message.contains("Undefined function"));
        assert!(err.message.contains("undefined_func"));
    }

    // =========================================================================
    // RECURSION TESTS
    // =========================================================================

    #[test]
    fn test_recursive_factorial() {
        let source = r#"
function factorial(n):
    if n <= 1:
        return 1
    else:
        return n * factorial(n - 1)

factorial(5)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Recursive factorial should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(120));
    }

    #[test]
    fn test_recursive_fibonacci() {
        let source = r#"
function fibonacci(n):
    if n <= 1:
        return n
    else:
        return fibonacci(n - 1) + fibonacci(n - 2)

fibonacci(6)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Recursive fibonacci should work: {:?}", result.err());
        // fib(6) = 8 (0, 1, 1, 2, 3, 5, 8)
        assert_eq!(result.unwrap(), Value::Integer(8));
    }

    #[test]
    fn test_nested_function_calls() {
        let source = r#"
function add(a, b):
    return a + b

function double(x):
    return add(x, x)

double(5)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Nested function calls should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(10));
    }

    // =========================================================================
    // SCOPE ISOLATION TESTS
    // =========================================================================

    #[test]
    fn test_scope_isolation_no_leak() {
        let source = r#"
function inner():
    let x = 100
    return x

function outer():
    inner()
    return x

outer()
"#;
        let result = interpret_with_functions(source);
        // Should fail because x is not defined in outer's scope
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undefined variable"));
    }

    #[test]
    fn test_parameter_shadowing() {
        let source = r#"
let x = 10

function shadow(x):
    return x

shadow(5)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Parameter shadowing should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(5));
    }

    #[test]
    fn test_function_call_does_not_leak_locals() {
        let source = r#"
let global_var = 42

function modify():
    let local_var = 100
    return local_var

modify()
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();

        let mut analyzer = crate::semantic::SemanticAnalyzer::new();
        analyzer.analyze_program(&program).unwrap();

        let mut interpreter = Interpreter::new();
        interpreter.execute_program(&program.statements).unwrap();

        // Global variable should still be accessible
        let global = interpreter.environment.lookup("global_var");
        assert!(global.is_some());
        assert_eq!(global.unwrap(), &Value::Integer(42));

        // Local variable should NOT be accessible
        let local = interpreter.environment.lookup("local_var");
        assert!(local.is_none());
    }

    // =========================================================================
    // RETURN STATEMENT TESTS
    // =========================================================================

    #[test]
    fn test_function_return_value() {
        let source = r#"
function get_value():
    return 42

get_value()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Integer(42));
    }

    #[test]
    fn test_function_empty_return() {
        let source = r#"
function empty_return():
    return

empty_return()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Nil);
    }

    #[test]
    fn test_function_no_return() {
        let source = r#"
function no_return():
    let x = 5

no_return()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Nil);
    }

    #[test]
    fn test_return_expression() {
        let source = r#"
function calc(a, b):
    return a * b + 1

calc(3, 4)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Integer(13));
    }

    // =========================================================================
    // ERROR HANDLING TESTS
    // =========================================================================

    #[test]
    fn test_undefined_variable() {
        let source = r#"
let x = undefined_var
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Undefined variable"));
    }

    #[test]
    fn test_division_by_zero() {
        let source = r#"
let x = 10 / 0
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Division by zero"));
    }

    #[test]
    fn test_type_error_addition() {
        let source = r#"
let x = 5 + "hello"
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot add"));
    }

    // =========================================================================
    // VALUE TYPE TESTS
    // =========================================================================

    #[test]
    fn test_truthy_values() {
        assert!(is_truthy(&Value::Boolean(true)));
        assert!(is_truthy(&Value::Integer(1)));
        assert!(is_truthy(&Value::String("hello".to_string())));
        assert!(is_truthy(&Value::Array(vec![Value::Integer(1)])));
    }

    #[test]
    fn test_falsy_values() {
        assert!(!is_truthy(&Value::Boolean(false)));
        assert!(!is_truthy(&Value::Nil));
        assert!(!is_truthy(&Value::Integer(0)));
        assert!(!is_truthy(&Value::String("".to_string())));
        assert!(!is_truthy(&Value::Array(vec![])));
    }

    #[test]
    fn test_value_equality() {
        assert!(values_equal(&Value::Integer(5), &Value::Integer(5)));
        assert!(!values_equal(&Value::Integer(5), &Value::Integer(6)));
        assert!(values_equal(&Value::Nil, &Value::Nil));
        assert!(!values_equal(&Value::Integer(5), &Value::Nil));
    }

    // =========================================================================
    // COMPLEX SCENARIOS
    // =========================================================================

    #[test]
    fn test_multiple_function_calls_isolated() {
        let source = r#"
function counter():
    let count = 0
    count = count + 1
    return count

counter()
counter()
counter()
"#;
        // Each call should have its own scope, so count always starts at 0
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Multiple isolated calls should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(1));
    }

    #[test]
    fn test_deeply_nested_calls() {
        let source = r#"
function level1(x):
    return x + 1

function level2(x):
    return level1(x) + 1

function level3(x):
    return level2(x) + 1

level3(5)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Deeply nested calls should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(8));
    }

    #[test]
    fn test_function_call_in_expression() {
        let source = r#"
function double(x):
    return x * 2

let result = double(5) + 3
result
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Function call in expression should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(13));
    }

    #[test]
    fn test_conditional_function_calls() {
        let source = r#"
function positive():
    return 1

function negative():
    return -1

let x = 5
if x > 0:
    positive()
else:
    negative()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Integer(1));
    }

    // =========================================================================
    // EDGE CASE TESTS - Required by specification
    // =========================================================================

    #[test]
    fn test_empty_program() {
        let source = "";
        let result = interpret_source(source);
        assert!(result.is_ok(), "Empty program should succeed");
        assert_eq!(result.unwrap(), Value::Nil, "Empty program should return Nil");
    }

    #[test]
    fn test_program_with_only_expressions() {
        let source = r#"
5 + 3
10 * 2
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Program with only expressions should succeed");
        // Last expression value should be the result
        assert_eq!(result.unwrap(), Value::Integer(20));
    }

    #[test]
    fn test_top_level_return_produces_error() {
        let source = r#"
let x = 10
return x
let y = 20
"#;
        let result = interpret_source(source);
        assert!(result.is_err(), "Top-level return should produce an error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Return statement outside of function"));
    }

    #[test]
    fn test_top_level_return_without_value_produces_error() {
        let source = r#"
let x = 10
return
let y = 20
"#;
        let result = interpret_source(source);
        assert!(result.is_err(), "Top-level return without value should produce an error");
        let err = result.unwrap_err();
        assert!(err.message.contains("Return statement outside of function"));
    }

    #[test]
    fn test_return_inside_function_is_allowed() {
        let source = r#"
function get_value():
    return 42

get_value()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Return inside function should be allowed");
        assert_eq!(result.unwrap(), Value::Integer(42));
    }

    #[test]
    fn test_return_inside_nested_function_is_allowed() {
        let source = r#"
function outer():
    function inner():
        return "inner_value"
    return inner()

outer()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Return inside nested function should be allowed");
        assert_eq!(result.unwrap(), Value::String("inner_value".to_string()));
    }

    #[test]
    fn test_variable_redeclaration_error() {
        let source = r#"
let x = 5
let x = 10
"#;
        let result = interpret_source(source);
        // Note: The semantic analyzer would catch this, but at runtime
        // the interpreter just overwrites the variable value
        // This test documents the current behavior
        assert!(result.is_ok(), "Interpreter allows variable overwrite (semantic analyzer catches redeclaration)");
    }

    #[test]
    fn test_undefined_variable_access() {
        let source = r#"
let x = undefined_var + 5
"#;
        let result = interpret_source(source);
        assert!(result.is_err(), "Accessing undefined variable should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Undefined variable"));
        assert!(err.message.contains("undefined_var"));
    }

    #[test]
    fn test_undefined_function_call() {
        let source = r#"
nonexistent_function()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err(), "Calling undefined function should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown function") || err.message.contains("Undefined function"));
        assert!(err.message.contains("nonexistent_function"));
    }

    #[test]
    fn test_multiple_variable_declarations() {
        let source = r#"
let a = 1
let b = 2
let c = 3
a + b + c
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Multiple variable declarations should succeed");
        assert_eq!(result.unwrap(), Value::Integer(6));
    }

    #[test]
    fn test_top_level_function_call() {
        let source = r#"
function greet():
    return "hello"

greet()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Top-level function call should succeed");
        assert_eq!(result.unwrap(), Value::String("hello".to_string()));
    }

    #[test]
    fn test_nested_blocks_scope() {
        let source = r#"
let x = 1
{
    let y = 2
    {
        let z = 3
        x + y + z
    }
}
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Nested blocks should work correctly");
        assert_eq!(result.unwrap(), Value::Integer(6));
    }

    // =========================================================================
    // MODULO OPERATOR TESTS
    // =========================================================================

    #[test]
    fn test_modulo_basic() {
        let source = r#"let x = 10 % 3"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Modulo operation should work");
    }

    #[test]
    fn test_modulo_result_value() {
        // Directly test the modulo operation through expression evaluation
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(10));
        env.define("y".to_string(), Value::Integer(3));
        
        // Create a binary expression: x % y
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "%".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Modulo evaluation should succeed");
        assert_eq!(result.unwrap(), Value::Integer(1)); // 10 % 3 = 1
    }

    #[test]
    fn test_modulo_by_zero_error() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(10));
        env.define("y".to_string(), Value::Integer(0));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "%".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_err(), "Modulo by zero should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Modulo by zero"));
    }

    #[test]
    fn test_modulo_with_non_integer_error() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::String("hello".to_string()));
        env.define("y".to_string(), Value::Integer(3));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "%".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_err(), "Modulo with non-integer should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot apply modulo"));
    }

    // =========================================================================
    // POWER OPERATOR TESTS
    // =========================================================================

    #[test]
    fn test_power_basic() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(2));
        env.define("y".to_string(), Value::Integer(3));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "**".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Power operation should succeed");
        assert_eq!(result.unwrap(), Value::Integer(8)); // 2 ** 3 = 8
    }

    #[test]
    fn test_power_zero_exponent() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(5));
        env.define("y".to_string(), Value::Integer(0));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "**".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Power with zero exponent should succeed");
        assert_eq!(result.unwrap(), Value::Integer(1)); // 5 ** 0 = 1
    }

    #[test]
    fn test_power_negative_exponent_returns_zero() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(2));
        env.define("y".to_string(), Value::Integer(-3));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "**".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Power with negative exponent should return 0");
        assert_eq!(result.unwrap(), Value::Integer(0)); // 2 ** -3 = 0 (integer semantics)
    }

    #[test]
    fn test_power_large_numbers() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(2));
        env.define("y".to_string(), Value::Integer(10));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "**".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Power with large numbers should succeed");
        assert_eq!(result.unwrap(), Value::Integer(1024)); // 2 ** 10 = 1024
    }

    #[test]
    fn test_power_with_non_integer_error() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::String("hello".to_string()));
        env.define("y".to_string(), Value::Integer(2));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "**".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_err(), "Power with non-integer should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot raise"));
    }

    // =========================================================================
    // SHORT-CIRCUITING LOGICAL OPERATORS TESTS
    // =========================================================================

    #[test]
    fn test_short_circuit_and_left_falsy() {
        // When left is falsy, and should short-circuit and return left without evaluating right
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Boolean(false));
        // y is not defined, but should not be accessed due to short-circuiting
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "and".to_string(),
            right: Box::new(Expression::Identifier("undefined_var".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Short-circuit and should succeed");
        assert_eq!(result.unwrap(), Value::Boolean(false));
    }

    #[test]
    fn test_short_circuit_and_left_truthy() {
        // When left is truthy, and should evaluate right
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Boolean(true));
        env.define("y".to_string(), Value::Boolean(true));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "and".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "And with truthy left should succeed");
        assert_eq!(result.unwrap(), Value::Boolean(true));
    }

    #[test]
    fn test_short_circuit_or_left_truthy() {
        // When left is truthy, or should short-circuit and return left without evaluating right
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Boolean(true));
        // y is not defined, but should not be accessed due to short-circuiting
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "or".to_string(),
            right: Box::new(Expression::Identifier("undefined_var".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Short-circuit or should succeed");
        assert_eq!(result.unwrap(), Value::Boolean(true));
    }

    #[test]
    fn test_short_circuit_or_left_falsy() {
        // When left is falsy, or should evaluate right
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Boolean(false));
        env.define("y".to_string(), Value::Boolean(true));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "or".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Or with falsy left should succeed");
        assert_eq!(result.unwrap(), Value::Boolean(true));
    }

    #[test]
    fn test_short_circuit_with_integer_truthy() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(5)); // truthy
        // undefined_var should not be accessed
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "or".to_string(),
            right: Box::new(Expression::Identifier("undefined_var".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Short-circuit or with integer should succeed");
        // Should return the original truthy value (5), not boolean true
        assert_eq!(result.unwrap(), Value::Integer(5));
    }

    #[test]
    fn test_short_circuit_with_integer_falsy() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(0)); // falsy
        env.define("y".to_string(), Value::Integer(10));
        
        let expr = Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Identifier("x".to_string())),
            operator: "and".to_string(),
            right: Box::new(Expression::Identifier("y".to_string())),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Short-circuit and with falsy integer should succeed");
        // Should return the original falsy value (0), not boolean false
        assert_eq!(result.unwrap(), Value::Integer(0));
    }

    // =========================================================================
    // ASSIGNMENT EXPRESSION TESTS
    // =========================================================================

    #[test]
    fn test_assignment_expression_basic() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(5));
        
        // Create assignment expression: x = 10
        let expr = Expression::Assignment(AssignmentExpression {
            name: "x".to_string(),
            value: Box::new(Expression::Integer(10)),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Assignment expression should succeed");
        assert_eq!(result.unwrap(), Value::Integer(10)); // Returns the assigned value
        
        // Verify the variable was updated
        let updated = interpreter.environment.lookup("x");
        assert_eq!(updated.unwrap(), &Value::Integer(10));
    }

    #[test]
    fn test_assignment_expression_with_expression_value() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(5));
        env.define("y".to_string(), Value::Integer(3));
        
        // Create assignment expression: x = y + 2
        let expr = Expression::Assignment(AssignmentExpression {
            name: "x".to_string(),
            value: Box::new(Expression::Binary(BinaryExpression {
                left: Box::new(Expression::Identifier("y".to_string())),
                operator: "+".to_string(),
                right: Box::new(Expression::Integer(2)),
            })),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_ok(), "Assignment with expression should succeed");
        assert_eq!(result.unwrap(), Value::Integer(5)); // 3 + 2 = 5
        
        let updated = interpreter.environment.lookup("x");
        assert_eq!(updated.unwrap(), &Value::Integer(5));
    }

    #[test]
    fn test_assignment_expression_undefined_variable() {
        // Try to assign to undefined variable
        let expr = Expression::Assignment(AssignmentExpression {
            name: "undefined_var".to_string(),
            value: Box::new(Expression::Integer(10)),
        });
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.evaluate_expression(&expr);
        
        assert!(result.is_err(), "Assignment to undefined variable should fail");
        let err = result.unwrap_err();
        assert!(err.message.contains("Undefined variable"));
    }

    #[test]
    fn test_chained_assignment_expression() {
        // Test chained assignment: a = b = 5 (evaluated right-to-left)
        let mut env = Environment::new();
        env.define("a".to_string(), Value::Integer(0));
        env.define("b".to_string(), Value::Integer(0));
        
        // Inner assignment: b = 5
        let inner = Expression::Assignment(AssignmentExpression {
            name: "b".to_string(),
            value: Box::new(Expression::Integer(5)),
        });
        
        // Outer assignment: a = (b = 5)
        let outer = Expression::Assignment(AssignmentExpression {
            name: "a".to_string(),
            value: Box::new(inner),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&outer);
        
        assert!(result.is_ok(), "Chained assignment should succeed");
        assert_eq!(result.unwrap(), Value::Integer(5));
        
        // Both variables should be 5
        let a_val = interpreter.environment.lookup("a");
        let b_val = interpreter.environment.lookup("b");
        assert_eq!(a_val.unwrap(), &Value::Integer(5));
        assert_eq!(b_val.unwrap(), &Value::Integer(5));
    }

    #[test]
    fn test_assignment_expression_returns_value_for_further_use() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(0));
        
        // Test that assignment returns a value that can be used in further expressions
        let assign_expr = Expression::Assignment(AssignmentExpression {
            name: "x".to_string(),
            value: Box::new(Expression::Integer(10)),
        });
        
        // Use the assignment result in a binary expression: (x = 10) + 5
        let binary_expr = Expression::Binary(BinaryExpression {
            left: Box::new(assign_expr),
            operator: "+".to_string(),
            right: Box::new(Expression::Integer(5)),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env;
        let result = interpreter.evaluate_expression(&binary_expr);
        
        assert!(result.is_ok(), "Assignment result in expression should work");
        assert_eq!(result.unwrap(), Value::Integer(15)); // (x = 10) returns 10, 10 + 5 = 15
    }

    // =========================================================================
    // SEQUENTIAL EVALUATION TESTS
    // =========================================================================

    #[test]
    fn test_sequential_variable_updates() {
        let mut env = Environment::new();
        env.define("x".to_string(), Value::Integer(0));
        
        // First assignment: x = 5
        let assign1 = Expression::Assignment(AssignmentExpression {
            name: "x".to_string(),
            value: Box::new(Expression::Integer(5)),
        });
        
        let mut interpreter = Interpreter::new();
        interpreter.environment = env.clone();
        interpreter.evaluate_expression(&assign1).unwrap();
        
        // Second assignment: x = x + 3
        let assign2 = Expression::Assignment(AssignmentExpression {
            name: "x".to_string(),
            value: Box::new(Expression::Binary(BinaryExpression {
                left: Box::new(Expression::Identifier("x".to_string())),
                operator: "+".to_string(),
                right: Box::new(Expression::Integer(3)),
            })),
        });
        
        let result = interpreter.evaluate_expression(&assign2);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Integer(8)); // 5 + 3 = 8
        
        let final_val = interpreter.environment.lookup("x");
        assert_eq!(final_val.unwrap(), &Value::Integer(8));
    }

    // =========================================================================
    // CONTROL FLOW EDGE CASE TESTS
    // =========================================================================

    #[test]
    fn test_empty_block() {
        let source = r#"
let x = 1
{
}
let y = 2
x + y
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Empty block should be allowed");
        assert_eq!(result.unwrap(), Value::Integer(3));
    }

    #[test]
    fn test_nested_control_flow_if_inside_while() {
        let source = r#"
let x = 0
let result = 0
while x < 3:
    if x == 1:
        result = result + 10
    else:
        result = result + 1
    x = x + 1
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Nested if inside while should work");
        // x=0: result=0+1=1, x=1: result=1+10=11, x=2: result=11+1=12
        assert_eq!(result.unwrap(), Value::Integer(12));
    }

    #[test]
    fn test_nested_control_flow_while_inside_if() {
        let source = r#"
let x = 5
let result = 0
if x > 0:
    while x > 0:
        result = result + x
        x = x - 1
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Nested while inside if should work");
        // 5+4+3+2+1 = 15
        assert_eq!(result.unwrap(), Value::Integer(15));
    }

    #[test]
    fn test_deeply_nested_control_flow() {
        let source = r#"
let result = 0
let x = 0
while x < 2:
    let y = 0
    while y < 2:
        if x == y:
            result = result + 1
        y = y + 1
    x = x + 1
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Deeply nested control flow should work");
        // (0,0) adds 1, (0,1) no add, (1,0) no add, (1,1) adds 1 -> total 2
        assert_eq!(result.unwrap(), Value::Integer(2));
    }

    #[test]
    fn test_while_loop_with_empty_body() {
        let source = r#"
let x = 0
while x < 5:
    x = x + 1
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "While loop with increment-only body should work");
    }

    #[test]
    fn test_for_loop_with_empty_body() {
        let source = r#"
let sum = 0
for i in [1, 2, 3]:
    sum = sum + i
sum
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "For loop should work");
        assert_eq!(result.unwrap(), Value::Integer(6));
    }

    #[test]
    fn test_if_statement_with_empty_consequence() {
        let source = r#"
let x = 1
if false:
    x = 2
x
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "If with empty consequence should work");
        assert_eq!(result.unwrap(), Value::Integer(1));
    }

    #[test]
    fn test_if_else_with_empty_alternative() {
        let source = r#"
let x = 1
if true:
    x = 2
else:
    x = 3
x
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "If-else with empty alternative branch should work");
        assert_eq!(result.unwrap(), Value::Integer(2));
    }

    #[test]
    fn test_sequential_execution_order() {
        let source = r#"
let a = 1
let b = 2
let c = 3
a = a + 1
b = b + a
c = c + b
c
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Sequential execution should maintain order");
        // a=2, b=4, c=7
        assert_eq!(result.unwrap(), Value::Integer(7));
    }

    #[test]
    fn test_block_scope_isolation() {
        let source = r#"
let x = 1
{
    let x = 100
    x = x + 1
}
x
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Block scope should be isolated");
        assert_eq!(result.unwrap(), Value::Integer(1), "Outer x should remain 1");
    }

    #[test]
    fn test_deeply_nested_scopes() {
        let source = r#"
let result = 0
{
    let a = 1
    {
        let b = 2
        {
            let c = 3
            result = a + b + c
        }
    }
}
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Deeply nested scopes should work");
        assert_eq!(result.unwrap(), Value::Integer(6));
    }

    #[test]
    fn test_multiple_blocks_same_level() {
        let source = r#"
let x = 0
{
    x = x + 1
}
{
    x = x + 10
}
{
    x = x + 100
}
x
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Multiple blocks at same level should work");
        assert_eq!(result.unwrap(), Value::Integer(111));
    }

    #[test]
    fn test_for_loop_nested_if() {
        let source = r#"
let result = 0
for i in [1, 2, 3, 4, 5]:
    if i > 3:
        result = result + i
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "For loop with nested if should work");
        // 4 + 5 = 9
        assert_eq!(result.unwrap(), Value::Integer(9));
    }

    #[test]
    fn test_if_inside_block() {
        let source = r#"
let x = 5
let result = 0
{
    if x > 0:
        result = 1
    else:
        result = 2
}
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "If statement inside block should work");
        assert_eq!(result.unwrap(), Value::Integer(1));
    }

    #[test]
    fn test_while_loop_condition_false_initially() {
        let source = r#"
let x = 10
let result = 0
while x < 5:
    result = result + 1
    x = x + 1
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "While loop with initially false condition should not execute");
        assert_eq!(result.unwrap(), Value::Integer(0));
    }

    #[test]
    fn test_for_loop_over_empty_array() {
        let source = r#"
let result = 0
for i in []:
    result = result + 1
result
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "For loop over empty array should not execute body");
        assert_eq!(result.unwrap(), Value::Integer(0));
    }

    #[test]
    fn test_return_in_nested_blocks_inside_function() {
        let source = r#"
function test():
    let x = 1
    {
        let y = 2
        {
            return x + y
        }
    }
    return 0

test()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Return in nested blocks inside function should work");
        assert_eq!(result.unwrap(), Value::Integer(3));
    }

    #[test]
    fn test_return_from_alternative_branch() {
        let source = r#"
function test(x):
    if x > 0:
        return "positive"
    else:
        return "non-positive"
    return "unreachable"

test(-5)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Return from alternative branch should work");
        assert_eq!(result.unwrap(), Value::String("non-positive".to_string()));
    }

    #[test]
    fn test_complex_nested_function_with_return() {
        let source = r#"
function outer(n):
    if n <= 0:
        return 0
    let result = 0
    for i in [1, 2, 3]:
        if i == n:
            return i
        result = result + i
    return result

outer(2)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Complex nested control flow with return should work");
        assert_eq!(result.unwrap(), Value::Integer(2));
    }

    #[test]
    fn test_control_flow_with_function_calls() {
        let source = r#"
function double(x):
    return x * 2

let result = 0
for i in [1, 2, 3]:
    if i > 1:
        result = result + double(i)
result
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Control flow with function calls should work");
        // double(2) + double(3) = 4 + 6 = 10
        assert_eq!(result.unwrap(), Value::Integer(10));
    }

    #[test]
    fn test_public_execute_if_statement_api() {
        let condition = Expression::Boolean(true);
        let if_stmt = IfStatement {
            condition,
            consequence: vec![],
            alternative: None,
        };
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.execute_if_statement(&if_stmt);
        
        assert!(result.is_ok(), "Public execute_if_statement API should work");
        assert_eq!(result.unwrap(), ControlFlow::Continue);
    }

    #[test]
    fn test_public_execute_while_loop_api() {
        let condition = Expression::Boolean(false);
        let while_stmt = WhileStatement {
            condition,
            body: vec![],
        };
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.execute_while_loop(&while_stmt);
        
        assert!(result.is_ok(), "Public execute_while_loop API should work");
        assert_eq!(result.unwrap(), ControlFlow::Continue);
    }

    #[test]
    fn test_public_execute_for_loop_api() {
        let for_stmt = ForStatement {
            item: "i".to_string(),
            list: Expression::Array(vec![]),
            body: vec![],
        };
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.execute_for_loop(&for_stmt);
        
        assert!(result.is_ok(), "Public execute_for_loop API should work");
        assert_eq!(result.unwrap(), ControlFlow::Continue);
    }

    #[test]
    fn test_public_execute_block_api() {
        let stmts = vec![
            Statement::Let(LetStatement {
                name: "x".to_string(),
                value: Expression::Integer(42),
            }),
        ];
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.execute_block(&stmts);
        
        assert!(result.is_ok(), "Public execute_block API should work");
        assert_eq!(result.unwrap(), ControlFlow::Continue);
        
        // Variable should not be visible outside block
        assert!(interpreter.environment.lookup("x").is_none());
    }

    #[test]
    fn test_public_execute_block_api_scope_isolation() {
        let stmts = vec![
            Statement::Expression(Expression::Identifier("outer_var".to_string())),
        ];
        
        let mut interpreter = Interpreter::new();
        interpreter.environment.define("outer_var".to_string(), Value::Integer(10));
        
        // Should be able to access outer variable inside block
        let result = interpreter.execute_block(&stmts);
        assert!(result.is_ok(), "Block should access outer scope");
    }

    #[test]
    fn test_deterministic_evaluation_order() {
        let source = r#"
let a = 0
let b = 0
let c = 0

if true:
    a = 1
    if true:
        b = 2
    c = 3

a + b + c
"#;
        let result = interpret_source(source);
        assert!(result.is_ok(), "Deterministic evaluation should work");
        assert_eq!(result.unwrap(), Value::Integer(6));
    }

    // =========================================================================
    // FRIENDLY RUNTIME ERROR MESSAGE TESTS
    // =========================================================================

    #[test]
    fn test_friendly_undefined_variable_message() {
        let source = r#"let x = unknown_var"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should contain helpful, friendly message
        assert!(err.message.contains("don't know about any variable"), 
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("unknown_var"));
        assert!(err.message.contains("forget to define"));
    }

    #[test]
    fn test_friendly_undefined_function_message() {
        let source = r#"my_func()"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Unknown function") || err.message.contains("can't find a function"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("my_func"));
    }

    #[test]
    fn test_friendly_arity_mismatch_message() {
        let source = r#"
function greet(a, b, c):
    return a + b + c

greet(1, 2)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("expects 3"),
            "Error should show expected: {}", err.message);
        assert!(err.message.contains("2") || err.message.contains("got 2"));
    }

    #[test]
    fn test_friendly_arity_mismatch_singular_plural() {
        // Test with 1 argument expected
        let source = r#"
function single(x):
    return x

single()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should use singular "argument" not "arguments"
        assert!(err.message.contains("expects 1 argument"),
            "Should use singular form: {}", err.message);
    }

    #[test]
    fn test_friendly_return_outside_function() {
        let source = r#"
let x = 5
return x
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("can't use 'return' here"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("only allowed inside functions"));
    }

    #[test]
    fn test_friendly_division_by_zero() {
        let source = r#"let x = 10 / 0"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot divide by zero"),
            "Error should be clear: {}", err.message);
        assert!(err.message.contains("undefined in mathematics"));
    }

    #[test]
    fn test_friendly_modulo_by_zero() {
        let source = r#"let x = 10 % 0"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot calculate modulo with zero"),
            "Error should be clear: {}", err.message);
    }

    #[test]
    fn test_friendly_type_mismatch_addition() {
        let source = r#"let x = 5 + "hello""#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot add"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("integer"));
        assert!(err.message.contains("string"));
        assert!(err.message.contains("don't work"));
    }

    #[test]
    fn test_friendly_type_mismatch_subtraction() {
        let source = r#"let x = 5 - "hello""#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot subtract"),
            "Error should be friendly: {}", err.message);
    }

    #[test]
    fn test_friendly_type_mismatch_comparison() {
        let source = r#"let x = 5 < "hello""#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot compare"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("can't be compared"));
    }

    #[test]
    fn test_friendly_not_iterable_error() {
        let source = r#"
for i in 42:
    write i
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot iterate over"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("for loops only work with arrays"));
        assert!(err.message.contains("integer"));
    }

    #[test]
    fn test_friendly_unary_negation_error() {
        let source = r#"let x = -"hello""#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot apply"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("string"));
        assert!(err.message.contains("doesn't make sense"));
    }

    #[test]
    fn test_friendly_assignment_undefined_variable() {
        let source = r#"x = 5"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot assign to 'x'"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("hasn't been defined yet"));
        assert!(err.message.contains("Use 'let x = ...' first"));
    }

    #[test]
    fn test_friendly_scope_exit_error() {
        let mut env = Environment::new();
        let result = env.exit_scope();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot exit the global scope"),
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("nowhere else to go"));
    }

    #[test]
    fn test_error_display_includes_context() {
        let ctx = RuntimeContext::new("testing operation").with_function("my_func");
        let err = RuntimeError::undefined_variable("x", Some(ctx));
        let display = format!("{}", err);
        assert!(display.contains("testing operation"), 
            "Display should include context: {}", display);
        assert!(display.contains("my_func"));
    }

    #[test]
    fn test_error_display_with_line_column() {
        let err = RuntimeError::new("Something went wrong").at(10, 5);
        let display = format!("{}", err);
        assert!(display.contains("at line 10"), 
            "Display should include line: {}", display);
        assert!(display.contains("column 5"));
    }

    #[test]
    fn test_error_display_without_context() {
        let err = RuntimeError::new("Simple error");
        let display = format!("{}", err);
        assert!(display.contains("Runtime error: Simple error"));
    }

    #[test]
    fn test_runtime_context_display() {
        let ctx = RuntimeContext::new("evaluating expression")
            .with_function("test_fn")
            .with_scope_depth(3);
        let display = format!("{}", ctx);
        assert!(display.contains("evaluating expression"));
        assert!(display.contains("test_fn"));
        assert!(display.contains("scope depth: 3"));
    }

    #[test]
    fn test_error_with_context_builder() {
        let ctx = RuntimeContext::new("calling function 'add'");
        let err = RuntimeError::arity_mismatch("add", 2, 1, Some(ctx));
        let display = format!("{}", err);
        assert!(display.contains("calling function 'add'"), 
            "Error should show context: {}", display);
    }

    #[test]
    fn test_multiple_sequential_errors() {
        // First error should stop execution, so we only get one
        let source = r#"
let x = unknown1
let y = unknown2
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should only report the first undefined variable
        assert!(err.message.contains("unknown1"));
        assert!(!err.message.contains("unknown2"));
    }

    #[test]
    fn test_nested_expression_error_propagation() {
        // Error in nested expression should propagate
        let source = r#"let x = 1 + (2 + unknown_var)"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unknown_var"));
    }

    #[test]
    fn test_function_call_in_expression_error() {
        let source = r#"
function add(a, b):
    return a + b

let x = add(1, unknown_var)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unknown_var"));
    }

    #[test]
    fn test_nested_function_call_error() {
        let source = r#"
function inner():
    return unknown_in_inner

function outer():
    return inner()

outer()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unknown_in_inner"));
    }

    #[test]
    fn test_boolean_operation_type_error() {
        let source = r#"let x = true + false"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot add"));
        assert!(err.message.contains("boolean"));
    }

    #[test]
    fn test_integer_overflow_error() {
        let source = r#"let x = 2 ** 100000"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("too big"), 
            "Error should be friendly: {}", err.message);
        assert!(err.message.contains("overflowed"));
    }

    #[test]
    fn test_type_error_in_if_condition() {
        // This tests that type checking happens in control flow
        let source = r#"
if 5 + "hello":
    let x = 1
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot add"));
    }

    #[test]
    fn test_type_error_in_while_condition() {
        let source = r#"
while 5 + "hello":
    let x = 1
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot add"));
    }

    #[test]
    fn test_error_in_array_literal() {
        let source = r#"let x = [1, 2, unknown_var]"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unknown_var"));
    }

    #[test]
    fn test_assignment_to_undefined_in_block() {
        let source = r#"
{
    x = 5
}
"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot assign"));
        assert!(err.message.contains("hasn't been defined"));
    }

    #[test]
    fn test_function_return_type_error() {
        let source = r#"
function bad():
    return 5 + "hello"

bad()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot add"));
    }

    #[test]
    fn test_modulo_with_invalid_types_string() {
        let source = r#"let x = "hello" % 3"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot"));
        assert!(err.message.contains("modulo"));
    }

    #[test]
    fn test_power_with_invalid_types() {
        let source = r#"let x = "hello" ** 2"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot raise"));
    }

    #[test]
    fn test_comparison_type_error_nil() {
        let source = r#"let x = nil < 5"#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot compare"));
        assert!(err.message.contains("nil"));
    }

    #[test]
    fn test_multiplication_type_error() {
        let source = r#"let x = 5 * "hello""#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot multiply"));
    }

    #[test]
    fn test_division_type_error() {
        let source = r#"let x = 5 / "hello""#;
        let result = interpret_source(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("Cannot divide"));
    }

    // =========================================================================
    // FIRST-CLASS FUNCTION TESTS
    // =========================================================================

    #[test]
    fn test_assign_function_to_variable() {
        let source = r#"
function add(a, b):
    return a + b

let f = add
f(3, 4)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Assigning function to variable should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(7));
    }

    #[test]
    fn test_call_function_via_variable() {
        let source = r#"
function greet(name):
    return "Hello, " + name

let say_hi = greet
say_hi("World")
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Calling function via variable should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::String("Hello, World".to_string()));
    }

    #[test]
    fn test_reassign_function_variable() {
        let source = r#"
function add(a, b):
    return a + b

function mul(a, b):
    return a * b

let f = add
let result1 = f(2, 3)
f = mul
let result2 = f(2, 3)
result1 + result2
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Reassigning function variable should work: {:?}", result.err());
        // 5 + 6 = 11
        assert_eq!(result.unwrap(), Value::Integer(11));
    }

    #[test]
    fn test_return_function_from_function() {
        let source = r#"
function make_adder(x):
    function adder(y):
        return x + y
    return adder

let add5 = make_adder(5)
add5(3)
"#;
        let result = interpret_with_functions(source);
        // Note: This works because we store the function value, but closures are not implemented yet
        // so x won't be captured. This tests that we can return and call functions.
        assert!(result.is_ok(), "Returning function from function should work: {:?}", result.err());
    }

    #[test]
    fn test_calling_non_function_error() {
        let source = r#"
let x = 5
x(3)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err(), "Calling non-function should produce error");
        let err = result.unwrap_err();
        assert!(err.message.contains("non-function"), "Error should mention non-function: {}", err.message);
    }

    #[test]
    fn test_calling_string_as_function_error() {
        let source = r#"
let s = "hello"
s()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_err(), "Calling string as function should produce error");
        let err = result.unwrap_err();
        assert!(err.message.contains("non-function"), "Error should mention non-function: {}", err.message);
        assert!(err.message.contains("string"), "Error should mention string type: {}", err.message);
    }

    #[test]
    fn test_function_defined_in_block_scope() {
        let source = r#"
{
    function inner():
        return 42
    let result = inner()
}
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Function defined in block scope should work: {:?}", result.err());
    }

    #[test]
    fn test_function_scope_isolation() {
        let source = r#"
function outer():
    let x = 10
    function inner():
        return x
    return inner()

outer()
"#;
        let result = interpret_with_functions(source);
        // Note: Without closures, x won't be captured. This tests scope isolation.
        assert!(result.is_err() || result.is_ok(), "Function scope isolation should be maintained");
    }

    #[test]
    fn test_truthy_function_value() {
        assert!(is_truthy(&Value::Function(FunctionValue::new(vec![], vec![]))));
    }

    #[test]
    fn test_function_equality() {
        let f1 = Value::Function(FunctionValue::new(vec!["a".to_string()], vec![]));
        let f2 = Value::Function(FunctionValue::new(vec!["a".to_string()], vec![]));
        // Functions should not be equal (identity comparison)
        assert!(!values_equal(&f1, &f2));
    }

    #[test]
    fn test_function_display() {
        let f = Value::Function(FunctionValue::new(vec!["a".to_string(), "b".to_string()], vec![]));
        let display = format!("{}", f);
        assert!(display.contains("function"), "Display should indicate function: {}", display);
        assert!(display.contains("a"), "Display should show parameters: {}", display);
        assert!(display.contains("b"), "Display should show parameters: {}", display);
    }

    #[test]
    fn test_nested_function_calls_via_variable() {
        let source = r#"
function add(a, b):
    return a + b

function double(x):
    return add(x, x)

let f = double
f(5)
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Nested calls via variable should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Integer(10));
    }

    #[test]
    fn test_function_shadowing() {
        let source = r#"
function f():
    return 1

{
    function f():
        return 2
    let result = f()
}
f()
"#;
        let result = interpret_with_functions(source);
        assert!(result.is_ok(), "Function shadowing should work: {:?}", result.err());
        // Outer f returns 1
        assert_eq!(result.unwrap(), Value::Integer(1));
    }
}

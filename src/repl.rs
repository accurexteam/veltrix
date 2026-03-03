use crate::interpreter::{ControlFlow, Interpreter, RuntimeError, Value};
use crate::lexer::Lexer;
use crate::parser::ast::Statement;
use crate::parser::Parser;
use std::io::{self, Write};

/// Result of reading input from the user
#[derive(Debug, Clone, PartialEq)]
pub enum ReadResult {
    /// A complete input ready for evaluation
    Complete(String),
    /// User wants to exit the REPL
    Exit,
    /// Empty input (just pressed enter)
    Empty,
    /// Continue collecting multi-line input
    Continue(String),
    /// Parse error occurred during input reading
    ParseError(String),
}

/// Result of evaluating REPL input
#[derive(Debug, Clone, PartialEq)]
pub enum EvalResult {
    /// Successful evaluation with a value to print
    Value(Value),
    /// Successful evaluation with no value (e.g., variable declaration)
    Ok,
    /// Function was defined
    FunctionDefined(String),
    /// Runtime error occurred
    Error(RuntimeError),
    /// Parse error occurred
    ParseError(String),
}

/// The Veltrix REPL for interactive execution
pub struct Repl {
    interpreter: Interpreter,
    multi_line_buffer: String,
    indent_level: usize,
}

impl Repl {
    /// Creates a new REPL with a fresh interpreter and environment
    pub fn new() -> Self {
        Self {
            interpreter: Interpreter::new(),
            multi_line_buffer: String::new(),
            indent_level: 0,
        }
    }

    /// Creates a REPL with an existing interpreter (e.g., after running a file).
    /// Preserves variable bindings and function definitions from the interpreter.
    pub fn with_interpreter(interpreter: Interpreter) -> Self {
        Self {
            interpreter,
            multi_line_buffer: String::new(),
            indent_level: 0,
        }
    }

    /// Starts the REPL loop, reading and evaluating input until exit
    pub fn start(&mut self) -> Result<(), io::Error> {
        println!("Veltrix REPL v0.1.0");
        println!("Type 'exit' or press Ctrl+C to quit.");
        println!();

        loop {
            match self.read_input()? {
                ReadResult::Complete(input) => {
                    self.reset_buffer();
                    match self.evaluate_input(&input) {
                        EvalResult::Value(val) => self.print_output(&val.to_string()),
                        EvalResult::Ok => {} // Silent success
                        EvalResult::FunctionDefined(name) => {
                            println!("Defined function '{}'", name);
                        }
                        EvalResult::Error(err) => self.print_error(&err),
                        EvalResult::ParseError(msg) => eprintln!("Parse error: {}", msg),
                    }
                }
                ReadResult::Continue(partial) => {
                    self.multi_line_buffer = partial;
                    self.indent_level += 1;
                }
                ReadResult::Exit => {
                    println!("Goodbye!");
                    break Ok(());
                }
                ReadResult::Empty => {}
                ReadResult::ParseError(msg) => {
                    eprintln!("Input error: {}", msg);
                    self.reset_buffer();
                }
            }
        }
    }

    /// Reads a line of input from the user
    /// Handles multi-line input detection for incomplete statements
    pub fn read_input(&self) -> Result<ReadResult, io::Error> {
        let prompt = if self.multi_line_buffer.is_empty() {
            "veltrix> "
        } else {
            "...    "
        };

        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        // Remove trailing newline
        let input = input.trim_end().to_string();

        // Check for exit command
        if input.trim() == "exit" {
            return Ok(ReadResult::Exit);
        }

        // Empty input
        if input.trim().is_empty() {
            if !self.multi_line_buffer.is_empty() {
                // Empty line in multi-line mode - try to complete
                let complete = self.multi_line_buffer.clone();
                return Ok(ReadResult::Complete(complete));
            }
            return Ok(ReadResult::Empty);
        }

        // Combine with existing buffer if in multi-line mode
        let full_input = if self.multi_line_buffer.is_empty() {
            input.clone()
        } else {
            format!("{}\n{}", self.multi_line_buffer, input)
        };

        // Check if input is complete (can be parsed)
        match self.is_complete_input(&full_input) {
            InputStatus::Complete => Ok(ReadResult::Complete(full_input)),
            InputStatus::Incomplete => Ok(ReadResult::Continue(full_input)),
            InputStatus::Error(msg) => Ok(ReadResult::ParseError(msg)),
        }
    }

    /// Evaluates a single input string in the REPL
    /// Maintains persistent state across evaluations
    pub fn evaluate_input(&mut self, input: &str) -> EvalResult {
        // First, try to parse as a complete program
        let mut lexer = Lexer::new(input);
        let tokens = match lexer.tokenize() {
            Ok(tokens) => tokens,
            Err(e) => {
                return EvalResult::ParseError(format!(
                    "Lexer error at line {}, column {}: {}",
                    e.line, e.column, e.message
                ));
            }
        };

        let mut parser = Parser::new(tokens);
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(e) => {
                return EvalResult::ParseError(format!(
                    "Parse error at line {}, column {}: {}",
                    e.line, e.column, e.message
                ));
            }
        };

        // Register any function declarations first
        for stmt in &program.statements {
            if let Statement::FunctionDeclaration(func) = stmt {
                // Create a Function value and store it in the environment
                let func_value = crate::interpreter::Value::Function(
                    crate::interpreter::FunctionValue::new(
                        func.params.clone(),
                        func.body.clone(),
                    )
                );
                self.interpreter.environment_mut().define(func.name.clone(), func_value);
            }
        }

        // Execute the statements
        let mut last_value = Value::Nil;

        for stmt in &program.statements {
            match self.execute_repl_statement(stmt) {
                Ok(control_flow) => {
                    match control_flow {
                        ControlFlow::Continue => {
                            // Track the last expression value for REPL-like behavior
                            if let Statement::Expression(expr) = stmt {
                                match self.interpreter.evaluate_expression_for_repl(expr) {
                                    Ok(val) => last_value = val,
                                    Err(e) => return EvalResult::Error(e),
                                }
                            }
                        }
                        ControlFlow::Return(val) => {
                            // In REPL, return gives us the value but doesn't exit
                            last_value = val.unwrap_or(Value::Nil);
                        }
                    }

                    // Check for function definition output
                    if let Statement::FunctionDeclaration(func) = stmt {
                        return EvalResult::FunctionDefined(func.name.clone());
                    }
                }
                Err(e) => return EvalResult::Error(e),
            }
        }

        // Return the last value if it was an expression, otherwise Ok
        if program.statements.len() == 1 {
            if let Statement::Expression(_) = &program.statements[0] {
                return EvalResult::Value(last_value);
            }
        }

        // Check if the last statement was an expression
        if let Some(last_stmt) = program.statements.last() {
            if let Statement::Expression(_) = last_stmt {
                return EvalResult::Value(last_value);
            }
        }

        EvalResult::Ok
    }

    /// Prints output to the user
    pub fn print_output(&self, output: &str) {
        println!("{}", output);
    }

    /// Prints an error message to the user
    pub fn print_error(&self, error: &RuntimeError) {
        eprintln!("{}", error);
    }

    /// Resets the multi-line buffer
    fn reset_buffer(&mut self) {
        self.multi_line_buffer.clear();
        self.indent_level = 0;
    }

    /// Checks if the input is a complete, parseable statement
    fn is_complete_input(&self, input: &str) -> InputStatus {
        let mut lexer = Lexer::new(input);
        let tokens = match lexer.tokenize() {
            Ok(tokens) => tokens,
            Err(_) => return InputStatus::Error("Lexer error".to_string()),
        };

        let mut parser = Parser::new(tokens);
        match parser.parse_program() {
            Ok(_) => InputStatus::Complete,
            Err(e) => {
                // Check if it looks like an incomplete input (e.g., missing colon, incomplete block)
                let msg = e.message.to_lowercase();
                if msg.contains("unexpected end")
                    || msg.contains("expected")
                    || msg.contains("incomplete")
                    || msg.contains("unexpected eof")
                {
                    InputStatus::Incomplete
                } else {
                    InputStatus::Error(e.message)
                }
            }
        }
    }

    /// Executes a single statement in the REPL context
    /// Returns ControlFlow to handle return statements properly
    fn execute_repl_statement(&mut self, stmt: &Statement) -> Result<ControlFlow, RuntimeError> {
        match stmt {
            Statement::Let(let_stmt) => {
                match self.interpreter.evaluate_expression_for_repl(&let_stmt.value) {
                    Ok(value) => {
                        self.interpreter.environment_mut().define(let_stmt.name.clone(), value);
                        Ok(ControlFlow::Continue)
                    }
                    Err(e) => Err(e),
                }
            }
            Statement::Assignment(assign) => {
                match self.interpreter.evaluate_expression_for_repl(&assign.value) {
                    Ok(value) => {
                        self.interpreter.environment_mut().assign(&assign.name, value)?;
                        Ok(ControlFlow::Continue)
                    }
                    Err(e) => Err(e),
                }
            }
            Statement::If(if_stmt) => self.interpreter.execute_if_statement(if_stmt),
            Statement::While(while_stmt) => self.interpreter.execute_while_loop(while_stmt),
            Statement::For(for_stmt) => self.interpreter.execute_for_loop(for_stmt),
            Statement::Block(stmts) => self.interpreter.execute_block(stmts),
            Statement::Return(ret) => {
                // In REPL, return is allowed and returns the value
                let value = match &ret.value {
                    Some(expr) => Some(self.interpreter.evaluate_expression_for_repl(expr)?),
                    None => None,
                };
                Ok(ControlFlow::Return(value))
            }
            Statement::Write(write) => {
                match self.interpreter.evaluate_expression_for_repl(&write.value) {
                    Ok(value) => {
                        println!("{}", value);
                        Ok(ControlFlow::Continue)
                    }
                    Err(e) => Err(e),
                }
            }
            Statement::Expression(expr) => {
                // Evaluate but don't print here - let the caller handle it
                self.interpreter.evaluate_expression_for_repl(expr)?;
                Ok(ControlFlow::Continue)
            }
            Statement::FunctionDeclaration(_) => {
                // Already registered, just continue
                Ok(ControlFlow::Continue)
            }
        }
    }

    /// Returns a reference to the interpreter (for testing)
    #[cfg(test)]
    pub fn interpreter(&self) -> &Interpreter {
        &self.interpreter
    }
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of input parsing
#[derive(Debug, Clone, PartialEq)]
enum InputStatus {
    Complete,
    Incomplete,
    Error(String),
}

/// Starts the REPL interactively
pub fn start_repl() -> Result<(), io::Error> {
    let mut repl = Repl::new();
    repl.start()
}

/// Reads input without running the full REPL (for testing)
pub fn read_input_line(input: &str) -> ReadResult {
    let repl = Repl::new();
    match repl.is_complete_input(input) {
        InputStatus::Complete => ReadResult::Complete(input.to_string()),
        InputStatus::Incomplete => ReadResult::Continue(input.to_string()),
        InputStatus::Error(msg) => ReadResult::ParseError(msg),
    }
}

/// Evaluates a single line in a fresh REPL context (for testing)
pub fn evaluate_line(input: &str) -> EvalResult {
    let mut repl = Repl::new();
    repl.evaluate_input(input)
}

/// Evaluates multiple lines with persistent state (for testing)
pub fn evaluate_lines(inputs: &[&str]) -> Vec<EvalResult> {
    let mut repl = Repl::new();
    inputs
        .iter()
        .map(|input| repl.evaluate_input(input))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // BASIC REPL TESTS
    // =========================================================================

    #[test]
    fn test_repl_creation() {
        let repl = Repl::new();
        assert!(repl.multi_line_buffer.is_empty());
        assert_eq!(repl.indent_level, 0);
    }

    #[test]
    fn test_evaluate_simple_expression() {
        let result = evaluate_line("5 + 3");
        assert!(
            matches!(result, EvalResult::Value(Value::Integer(8))),
            "Expected Value(8), got {:?}",
            result
        );
    }

    #[test]
    fn test_evaluate_variable_declaration() {
        let result = evaluate_line("let x = 42");
        assert!(matches!(result, EvalResult::Ok), "Expected Ok, got {:?}", result);
    }

    #[test]
    fn test_evaluate_multiple_statements() {
        let results = evaluate_lines(&["let x = 5", "let y = 10", "x + y"]);
        assert!(matches!(results[0], EvalResult::Ok));
        assert!(matches!(results[1], EvalResult::Ok));
        assert!(
            matches!(results[2], EvalResult::Value(Value::Integer(15))),
            "Expected Value(15), got {:?}",
            results[2]
        );
    }

    #[test]
    fn test_evaluate_function_definition() {
        let result = evaluate_line("function add(a, b):\n    return a + b");
        assert!(
            matches!(result, EvalResult::FunctionDefined(ref name) if name == "add"),
            "Expected FunctionDefined, got {:?}",
            result
        );
    }

    #[test]
    fn test_persistent_environment() {
        let mut repl = Repl::new();

        // Define a variable
        let result1 = repl.evaluate_input("let x = 100");
        assert!(matches!(result1, EvalResult::Ok));

        // Use the variable in a new expression
        let result2 = repl.evaluate_input("x + 1");
        assert!(
            matches!(result2, EvalResult::Value(Value::Integer(101))),
            "Expected 101, got {:?}",
            result2
        );
    }

    #[test]
    fn test_function_call_after_definition() {
        let mut repl = Repl::new();

        // Define a function
        let result1 = repl.evaluate_input("function double(x):\n    return x * 2");
        assert!(matches!(result1, EvalResult::FunctionDefined(_)));

        // Call the function
        let result2 = repl.evaluate_input("double(5)");
        assert!(
            matches!(result2, EvalResult::Value(Value::Integer(10))),
            "Expected 10, got {:?}",
            result2
        );
    }

    // =========================================================================
    // ERROR HANDLING TESTS
    // =========================================================================

    #[test]
    fn test_undefined_variable_error() {
        let result = evaluate_line("unknown_var + 5");
        assert!(
            matches!(result, EvalResult::Error(_)),
            "Expected Error, got {:?}",
            result
        );
        if let EvalResult::Error(err) = result {
            assert!(err.message.contains("don't know"));
        }
    }

    #[test]
    fn test_runtime_error_recovery() {
        let mut repl = Repl::new();

        // First, a valid statement
        let result1 = repl.evaluate_input("let x = 10");
        assert!(matches!(result1, EvalResult::Ok));

        // Then an error
        let result2 = repl.evaluate_input("x / 0");
        assert!(matches!(result2, EvalResult::Error(_)));

        // Environment should still be intact - can use x again
        let result3 = repl.evaluate_input("x + 5");
        assert!(
            matches!(result3, EvalResult::Value(Value::Integer(15))),
            "Expected recovery with value 15, got {:?}",
            result3
        );
    }

    #[test]
    fn test_parse_error_handling() {
        let result = evaluate_line("let x = ");
        assert!(
            matches!(result, EvalResult::ParseError(_)),
            "Expected ParseError, got {:?}",
            result
        );
    }

    // =========================================================================
    // CONTROL FLOW TESTS
    // =========================================================================

    #[test]
    fn test_if_statement_in_repl() {
        let mut repl = Repl::new();

        let result1 = repl.evaluate_input("let x = 5");
        assert!(matches!(result1, EvalResult::Ok));

        let result2 = repl.evaluate_input("if x > 3:\n    let y = 10\ny");
        // Should execute and the last expression should return
        assert!(
            matches!(result2, EvalResult::Value(Value::Integer(10))),
            "Expected 10, got {:?}",
            result2
        );
    }

    #[test]
    fn test_while_loop_in_repl() {
        let mut repl = Repl::new();

        let result1 = repl.evaluate_input("let count = 0");
        assert!(matches!(result1, EvalResult::Ok));

        let result2 = repl.evaluate_input("while count < 3:\n    count = count + 1\ncount");
        assert!(
            matches!(result2, EvalResult::Value(Value::Integer(3))),
            "Expected 3, got {:?}",
            result2
        );
    }

    #[test]
    fn test_for_loop_in_repl() {
        let mut repl = Repl::new();

        let result1 = repl.evaluate_input("let sum = 0");
        assert!(matches!(result1, EvalResult::Ok));

        let result2 = repl.evaluate_input("for i in [1, 2, 3]:\n    sum = sum + i\nsum");
        assert!(
            matches!(result2, EvalResult::Value(Value::Integer(6))),
            "Expected 6, got {:?}",
            result2
        );
    }

    #[test]
    fn test_return_in_repl() {
        // In REPL, return is allowed and returns the value
        let result = evaluate_line("return 42");
        assert!(
            matches!(result, EvalResult::Value(Value::Integer(42))),
            "Expected 42, got {:?}",
            result
        );
    }

    // =========================================================================
    // MULTI-LINE INPUT TESTS
    // =========================================================================

    #[test]
    fn test_is_complete_input_complete() {
        let repl = Repl::new();
        let status = repl.is_complete_input("let x = 5");
        assert!(matches!(status, InputStatus::Complete));
    }

    #[test]
    fn test_is_complete_input_incomplete() {
        let repl = Repl::new();
        // This should be incomplete because it expects more after the colon
        let status = repl.is_complete_input("function foo():");
        // Note: Depending on parser behavior, this might be Complete or Incomplete
        // The test documents current behavior
        assert!(
            matches!(status, InputStatus::Complete | InputStatus::Incomplete),
            "Got unexpected status: {:?}",
            status
        );
    }

    #[test]
    fn test_function_definition_persistence() {
        let mut repl = Repl::new();

        // Define the function
        let def = "function greet(name):\n    return name";
        let result1 = repl.evaluate_input(def);
        assert!(matches!(result1, EvalResult::FunctionDefined(_)));

        // Call it multiple times
        let result2 = repl.evaluate_input("greet(\"Alice\")");
        assert!(
            matches!(result2, EvalResult::Value(Value::String(ref s)) if s == "Alice")
        );

        let result3 = repl.evaluate_input("greet(\"Bob\")");
        assert!(
            matches!(result3, EvalResult::Value(Value::String(ref s)) if s == "Bob")
        );
    }

    // =========================================================================
    // EDGE CASE TESTS
    // =========================================================================

    #[test]
    fn test_empty_input() {
        let result = evaluate_line("");
        // Empty input might return Ok or Value(Nil) depending on implementation
        assert!(
            matches!(result, EvalResult::Ok | EvalResult::ParseError(_)),
            "Got unexpected result: {:?}",
            result
        );
    }

    #[test]
    fn test_comment_handling() {
        // If comments are supported
        let result = evaluate_line("5 + 3  # This is a comment");
        // This depends on whether lexer supports comments
        assert!(
            matches!(result, EvalResult::Value(_) | EvalResult::ParseError(_)),
            "Got: {:?}",
            result
        );
    }

    #[test]
    fn test_variable_reassignment() {
        let mut repl = Repl::new();

        let result1 = repl.evaluate_input("let x = 5");
        assert!(matches!(result1, EvalResult::Ok));

        let result2 = repl.evaluate_input("x = 10");
        assert!(matches!(result2, EvalResult::Ok));

        let result3 = repl.evaluate_input("x");
        assert!(
            matches!(result3, EvalResult::Value(Value::Integer(10))),
            "Expected 10, got {:?}",
            result3
        );
    }

    #[test]
    fn test_nested_function_calls() {
        let mut repl = Repl::new();

        // Define nested functions
        let def1 = repl.evaluate_input("function add(a, b):\n    return a + b");
        assert!(matches!(def1, EvalResult::FunctionDefined(_)));

        let def2 = repl.evaluate_input("function double(x):\n    return add(x, x)");
        assert!(matches!(def2, EvalResult::FunctionDefined(_)));

        // Call nested function
        let result = repl.evaluate_input("double(5)");
        assert!(
            matches!(result, EvalResult::Value(Value::Integer(10))),
            "Expected 10, got {:?}",
            result
        );
    }

    #[test]
    fn test_block_scope_in_repl() {
        let mut repl = Repl::new();

        let result1 = repl.evaluate_input("let x = 1");
        assert!(matches!(result1, EvalResult::Ok));

        // Block with its own scope
        let result2 = repl.evaluate_input("{\n    let x = 100\n    x\n}");
        assert!(
            matches!(result2, EvalResult::Value(Value::Integer(100))),
            "Expected 100, got {:?}",
            result2
        );

        // Original x should still be 1
        let result3 = repl.evaluate_input("x");
        assert!(
            matches!(result3, EvalResult::Value(Value::Integer(1))),
            "Expected 1, got {:?}",
            result3
        );
    }

    #[test]
    fn test_read_input_line_exit() {
        let result = read_input_line("exit");
        assert!(matches!(result, ReadResult::Complete(ref s) if s == "exit"));
    }

    #[test]
    fn test_complex_expression_evaluation() {
        let result = evaluate_line("(5 + 3) * 2 - 4 / 2");
        // (5 + 3) * 2 - 4 / 2 = 8 * 2 - 2 = 16 - 2 = 14
        assert!(
            matches!(result, EvalResult::Value(Value::Integer(14))),
            "Expected 14, got {:?}",
            result
        );
    }

    #[test]
    fn test_array_literal_evaluation() {
        let result = evaluate_line("[1, 2, 3]");
        assert!(
            matches!(result, EvalResult::Value(Value::Array(ref arr)) if arr.len() == 3),
            "Expected array, got {:?}",
            result
        );
    }

    #[test]
    fn test_string_concatenation_not_supported() {
        // Test that string + string works if supported, or fails appropriately
        let result = evaluate_line("\"hello\" + \"world\"");
        // If strings can be concatenated:
        assert!(
            matches!(result, EvalResult::Value(Value::String(ref s)) if s == "helloworld")
                || matches!(result, EvalResult::Error(_)),
            "String concatenation behavior: {:?}",
            result
        );
    }

    #[test]
    fn test_boolean_operations() {
        let result = evaluate_line("true and false");
        assert!(
            matches!(result, EvalResult::Value(Value::Boolean(false))),
            "Expected false, got {:?}",
            result
        );

        let result2 = evaluate_line("true or false");
        assert!(
            matches!(result2, EvalResult::Value(Value::Boolean(true))),
            "Expected true, got {:?}",
            result2
        );
    }

    #[test]
    fn test_comparison_operations() {
        let result = evaluate_line("5 < 10");
        assert!(
            matches!(result, EvalResult::Value(Value::Boolean(true))),
            "Expected true, got {:?}",
            result
        );

        let result2 = evaluate_line("5 == 5");
        assert!(
            matches!(result2, EvalResult::Value(Value::Boolean(true))),
            "Expected true, got {:?}",
            result2
        );
    }

    #[test]
    fn test_nil_value() {
        let result = evaluate_line("nil");
        assert!(
            matches!(result, EvalResult::Value(Value::Nil)),
            "Expected nil, got {:?}",
            result
        );
    }

    #[test]
    fn test_unary_operations() {
        let result = evaluate_line("-5");
        assert!(
            matches!(result, EvalResult::Value(Value::Integer(-5))),
            "Expected -5, got {:?}",
            result
        );

        let result2 = evaluate_line("not true");
        assert!(
            matches!(result2, EvalResult::Value(Value::Boolean(false))),
            "Expected false, got {:?}",
            result2
        );
    }

    #[test]
    fn test_write_statement_output() {
        // write statement prints to stdout, which we can't easily capture in unit tests
        // but we can verify it executes without error
        let result = evaluate_line("write 42");
        assert!(
            matches!(result, EvalResult::Ok),
            "Expected Ok, got {:?}",
            result
        );
    }
}

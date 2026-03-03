fn main() {
    use veltrix::interpreter::{Interpreter, Value};
    use veltrix::lexer::Lexer;
    use veltrix::parser::Parser;
    
    let source = r#"
{
    x = 5
}
"#;
    
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();
    
    let mut interpreter = Interpreter::new();
    let result = interpreter.execute_program(&program.statements);
    
    match result {
        Ok(val) => println!("Success: {:?}", val),
        Err(e) => println!("Error: {}", e.message),
    }
}

# Veltrix v0.1 Lexer Specification

██╗   ██╗███████╗██╗  ████████╗██████╗ ██╗██╗  ██╗
██║   ██║██╔════╝██║  ╚══██╔══╝██╔══██╗██║╚██╗██╔╝
██║   ██║█████╗  ██║     ██║   ██████╔╝██║ ╚███╔╝ 
╚██╗ ██╔╝██╔══╝  ██║     ██║   ██╔══██╗██║ ██╔██╗ 
 ╚████╔╝ ███████╗███████╗██║   ██║  ██║██║██╔╝ ██╗
  ╚═══╝  ╚══════╝╚══════╝╚═╝   ╚═╝  ╚═╝╚═╝╚═╝  ╚═╝
## 1. Project Overview

Veltrix is a programming language designed with an indentation-based block structure to enforce uniform code formatting and readability. The v0.1 specification defines the foundational lexing phase, strictly separating token generation from parsing.

Indentation-based blocks are employed to eliminate syntactic clutter associated with brace-delimited scopes, enforcing a structural correlation between visual layout and logical nesting. 

Structured errors are utilized to provide precise, programmatic diagnostics rather than string-based error messages. This guarantees that tooling and the future parser can deterministically handle and report lexical failures with exact line and column tracking.

Interpolation parsing is intentionally deferred to the parser phase. The lexer solely identifies raw strings to maintain lexical simplicity and ensure a strict linear scan without requiring context-aware recursive descent during tokenization.

## 2. Lexer Architecture Overview

The lexer processes source code via a straightforward character traversal model. It maintains a stateful cursor that performs a single-pass, linear iteration over the input byte slice, tracking absolute position, line numbers (1-indexed), and column numbers (1-indexed).

Token emission follows a discrete model where identified sequences are consumed and immediately translated into corresponding `Token` structures. The lexer generates a complete sequence of tokens in memory before yielding them to the caller.

Error handling is implemented via `Result<Vec<Token>, LexError>`. Any lexical invalidity instantly halts the process, returning a structured `LexError` detailing the failure mode and its exact location. There are no partial returns or error recovery mechanisms at this stage.

An internal INDENT/DEDENT stack explicitly tracks changes in line indentation. The stack is modified when a non-blank line begins with a different indentation depth than the current top of the stack.

Upon encountering EOF, the lexer performs a deterministic stack unwind, emitting standard `Dedent` tokens until the base indentation level (depth 0) is reached. Finally, an `EOF` token is appended.

## 3. Complete Token Reference

Tokens are grouped by their designated semantic categories.

### Keywords
Reserved identifiers that dictate control flow, declarations, and core language primitives.
- `let`: Variable declaration.
- `if`: Conditional branching.
- `else`: Alternative conditional branch.
- `for`: Definite loop iteration.
- `in`: Membership testing or loop iteration target.
- `while`: Indefinite loop iteration.
- `func`: Function declaration.
- `return`: Function termination and value emission.
- `write`: Standard output generation.
- `true`: Boolean literal true.
- `false`: Boolean literal false.
- `and`: Logical conjunction.
- `or`: Logical disjunction.
- `not`: Logical negation.

### Literals
Primitive data representations.
- `Identifier`: Alphanumeric sequences starting with an alphabetical character or underscore, used for variable and function naming.
- `Number`: 64-bit signed integer (`i64`).
- `String`: Raw text enclosed in double quotes.

### Operators
Symbols executing specific mathematical or logical operations.
- `=`: Assignment.
- `==`: Equality comparison.
- `!=`: Inequality comparison.
- `<`: Less-than comparison.
- `>`: Greater-than comparison.
- `<=`: Less-than or equal-to comparison.
- `>=`: Greater-than or equal-to comparison.
- `+`: Addition.
- `-`: Subtraction.
- `*`: Multiplication.
- `/`: Division.
- `%`: Modulo.

### Delimiters
Structural boundaries for expressions, parameters, and collections.
- `(`: Open parenthesis.
- `)`: Close parenthesis.
- `[`: Open bracket.
- `]`: Close bracket.
- `,`: Comma separator.

### Structural Tokens
Tokens dictating the logical structure of the program based on file layout and indentation.
- `Newline`: Represents a structural end-of-line where significant.
- `Indent`: Represents an increase in the current indentation depth block.
- `Dedent`: Represents a decrease in the current indentation depth block.
- `EOF`: Marks the definitive end of the input source.

## 4. Indentation Rules (Formal Specification)

The indentation engine adheres to a strict space-based standard. 
- **Spaces Only**: All indentation must be accomplished using space characters (0x20).
- **Tabs Forbidden**: Tab characters (0x09) are strictly forbidden within leading whitespace and yield immediate errors.
- **Indentation Stack Behavior**: The lexer utilizes a stack data structure to track active indentation depths. The base level begins at 0. A new sequence of spaces strictly greater than the top of the stack results in pushing the new depth and emitting an `Indent` token.
- **Dedent Rules**: When leading spaces are fewer than the current stack top, the lexer pops values from the stack, emitting a `Dedent` token for each pop, until the stack top equals the new indentation level.
- **Blank Line Handling**: Lines composed entirely of whitespace are ignored and do not affect the indentation stack.
- **Comment-Only Line Handling**: Lines containing only whitespace followed by a comment are ignored and do not influence indentation state.
- **EOF Dedent Behavior**: Reaching the end of the file triggers artificial unwinding of the indentation stack. Discovered depths are sequentially popped, and `Dedent` tokens are emitted until the stack returns to size 1 (depth 0).

## 5. Error Conditions

The lexer captures lexical violations deterministically via `LexError`.

- **Unexpected Character**: Encountering a character that does not constitute a valid token or operator.
  ```
  let $var = 1
  ```
- **Invalid Operator**: Detecting an incomplete or unrecognizable sequence following an operator prefix.
  ```
  let a = !b
  ```
- **Unterminated String**: A string literal that lacks a closing double quote before EOF.
  ```
  let s = "hello
  ```
- **String with Newline**: A string literal containing a raw newline character instead of closing.
  ```
  let s = "hello
  world"
  ```
- **Integer Overflow**: A numeric sequence that exceeds the bounds of a signed 64-bit integer (`i64`).
  ```
  let num = 9223372036854775808
  ```
- **Inconsistent Indentation**: A dedent operation that does not align with any previously established indentation depth on the stack.
  ```
  if true
      let a = 1
    let b = 2
  ```
- **File Starting with Indentation**: Providing leading whitespace on the very first substantial line of the file.
  ```
    let a = 1
  ```
- **Tab Usage**: The presence of a tab character (`\t`) anywhere within leading indentation or source files where forbidden.
  ```
  \tlet a = 1
  ```
- **Dedent to Invalid Level**: Backing out an indentation to a width that is not present in the stack hierarchy.

## 6. Known Limitations (Intentional)

The following behaviors are explicitly scoped out of the v0.1 specification:
- Does not support floats.
- Does not support escape sequences.
- Does not parse interpolation yet.
- Does not support multi-line strings.
- Does not support Unicode normalization.
- Does not include parser implementation.

## 7. Testing Strategy

The validation strategy relies on deterministic, exhaustive coverage.
- **Manual Test via `main.rs`**: Initial validation is conducted via string injection in the compilation entry point.
- **Planned Future Unit Tests**: Architecture is designed for standard Rust `#[test]` modules focusing on per-character inputs and state assertions.
- **Edge Case Matrix Philosophy**: Testing methodologies follow a matrix strategy targeting the boundaries of integer limits, stack unrolling edge cases, and unexpected token adjacency.

## 8. Future Roadmap (Lexer Phase 2)

Future enhancements constrained strictly to lexical processing:
- Float support.
- Escape sequences.
- Better error recovery.
- Performance optimizations.
- Parser integration.

## 9. Security Requirements

- **Deterministic Design**: The lexer is a finite state machine producing strictly predictable outputs based entirely on its input sequence.
- **No Unsafe Rust**: The entire implementation is written exclusively in safe Rust. The `unsafe` keyword is strictly prohibited.
- **Malicious Input Resilience**: The engine defends against OOM attacks and CPU exhaustion by maintaining a linear sequence scan without exponential backtracking complexity.
- **No Recursion Used**: Call stacks remain flat; no iterative state parsing relies on functional recursion, averting stack overflow vulnerabilities on deeply nested or malformed inputs.

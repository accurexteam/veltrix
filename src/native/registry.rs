//! Immutable native function registry.
//!
//! Built via `NativeFunctionRegistryBuilder`. No global mutable state.
//! Case-sensitive lookups.

use crate::native::function::{NativeFunction, NativeFunctionError};
use std::collections::HashMap;

/// Immutable registry of native functions.
/// Constructed via `NativeFunctionRegistryBuilder`; cannot be mutated after build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunctionRegistry {
    functions: HashMap<String, NativeFunction>,
}

impl NativeFunctionRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Returns true if the given name is a registered native function.
    /// Case-sensitive.
    pub fn is_native(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Looks up a native function by name.
    /// Returns `Some(&NativeFunction)` if found, `None` otherwise.
    pub fn lookup(&self, name: &str) -> Option<&NativeFunction> {
        self.functions.get(name)
    }

    /// Validates a call: checks that the function exists and argument count matches.
    ///
    /// # Errors
    /// - `FunctionNotFound` if the name is not registered
    /// - `WrongArgumentCount` if arg_count does not match the function's arity
    pub fn validate_call(
        &self,
        name: &str,
        arg_count: usize,
        line: usize,
        column: usize,
    ) -> Result<(), NativeFunctionValidationError> {
        let func = self.lookup(name).ok_or_else(|| {
            NativeFunctionValidationError::FunctionNotFound {
                name: name.to_string(),
                line,
                column,
            }
        })?;

        if arg_count != func.arity() {
            return Err(NativeFunctionValidationError::WrongArgumentCount {
                name: name.to_string(),
                expected: func.arity(),
                actual: arg_count,
                line,
                column,
            });
        }

        Ok(())
    }

    /// Returns the number of registered native functions.
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Returns true if the registry contains no functions.
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Returns an iterator over registered function names.
    pub fn function_names(&self) -> impl Iterator<Item = &String> {
        self.functions.keys()
    }
}

impl Default for NativeFunctionRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

/// Builder for constructing an immutable `NativeFunctionRegistry`.
/// Rejects duplicate registrations.
#[derive(Debug, Default)]
pub struct NativeFunctionRegistryBuilder {
    functions: HashMap<String, NativeFunction>,
}

impl NativeFunctionRegistryBuilder {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    /// Registers a native function. Fails if the name is already registered.
    ///
    /// # Errors
    /// - `DuplicateRegistration` if the name already exists
    /// - `ArityTooLarge` if arity exceeds `MAX_NATIVE_ARITY`
    pub fn register(
        &mut self,
        name: &str,
        arity: usize,
    ) -> Result<&mut Self, NativeFunctionError> {
        if self.functions.contains_key(name) {
            return Err(NativeFunctionError::DuplicateRegistration {
                name: name.to_string(),
            });
        }

        let func = NativeFunction::new(name.to_string(), arity)?;
        self.functions.insert(name.to_string(), func);
        Ok(self)
    }

    /// Builds the immutable registry.
    pub fn build(self) -> NativeFunctionRegistry {
        NativeFunctionRegistry {
            functions: self.functions,
        }
    }
}

/// Validation error for native function calls, with source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFunctionValidationError {
    FunctionNotFound {
        name: String,
        line: usize,
        column: usize,
    },
    WrongArgumentCount {
        name: String,
        expected: usize,
        actual: usize,
        line: usize,
        column: usize,
    },
}

impl std::fmt::Display for NativeFunctionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeFunctionValidationError::FunctionNotFound { name, .. } => {
                write!(f, "Unknown function '{}'", name)
            }
            NativeFunctionValidationError::WrongArgumentCount {
                name,
                expected,
                actual,
                ..
            } => {
                write!(
                    f,
                    "Function '{}' expects {} argument(s), got {}",
                    name, expected, actual
                )
            }
        }
    }
}

impl std::error::Error for NativeFunctionValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::MAX_NATIVE_ARITY;

    #[test]
    fn test_registry_empty() {
        let reg = NativeFunctionRegistry::empty();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(!reg.is_native("foo"));
        assert!(reg.lookup("foo").is_none());
    }

    #[test]
    fn test_registry_default_empty() {
        let reg: NativeFunctionRegistry = Default::default();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_builder_register_single() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("foo", 2).expect("register should succeed");
        let reg = builder.build();
        assert_eq!(reg.len(), 1);
        assert!(reg.is_native("foo"));
        let func = reg.lookup("foo").expect("lookup should succeed");
        assert_eq!(func.name(), "foo");
        assert_eq!(func.arity(), 2);
    }

    #[test]
    fn test_builder_register_duplicate_rejected() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("bar", 0).expect("first register should succeed");
        let result = builder.register("bar", 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        matches!(err, NativeFunctionError::DuplicateRegistration { .. });
        let s = format!("{}", err);
        assert!(s.contains("bar"));
        assert!(s.contains("already registered"));
    }

    #[test]
    fn test_builder_register_arity_too_large() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        let result = builder.register("huge", MAX_NATIVE_ARITY + 1);
        assert!(result.is_err());
        matches!(result.unwrap_err(), NativeFunctionError::ArityTooLarge { .. });
    }

    #[test]
    fn test_builder_register_multiple() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("a", 0).unwrap();
        builder.register("b", 1).unwrap();
        builder.register("c", 2).unwrap();
        let reg = builder.build();
        assert_eq!(reg.len(), 3);
        assert!(reg.is_native("a"));
        assert!(reg.is_native("b"));
        assert!(reg.is_native("c"));
    }

    #[test]
    fn test_lookup_unknown() {
        let reg = NativeFunctionRegistry::empty();
        assert!(reg.lookup("unknown").is_none());
    }

    #[test]
    fn test_validate_call_success() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("add", 2).unwrap();
        let reg = builder.build();
        let result = reg.validate_call("add", 2, 1, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_call_wrong_arg_count() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("add", 2).unwrap();
        let reg = builder.build();
        let result = reg.validate_call("add", 3, 1, 5);
        assert!(result.is_err());
        let err = result.unwrap_err();
        matches!(err, NativeFunctionValidationError::WrongArgumentCount { .. });
        let s = format!("{}", err);
        assert!(s.contains("add"));
        assert!(s.contains("2"));
        assert!(s.contains("3"));
    }

    #[test]
    fn test_validate_call_function_not_found() {
        let reg = NativeFunctionRegistry::empty();
        let result = reg.validate_call("nonexistent", 0, 1, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        matches!(err, NativeFunctionValidationError::FunctionNotFound { .. });
    }

    #[test]
    fn test_case_sensitive() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("Print", 1).unwrap();
        let reg = builder.build();
        assert!(reg.is_native("Print"));
        assert!(!reg.is_native("print"));
        assert!(!reg.is_native("PRINT"));
        assert!(reg.lookup("Print").is_some());
        assert!(reg.lookup("print").is_none());
    }

    #[test]
    fn test_registry_immutable_after_build() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("x", 0).unwrap();
        let reg = builder.build();
        assert_eq!(reg.len(), 1);
        assert!(reg.function_names().next().is_some());
    }

    #[test]
    fn test_registry_clone() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("clone", 1).unwrap();
        let reg = builder.build();
        let cloned = reg.clone();
        assert_eq!(reg.len(), cloned.len());
        assert!(cloned.is_native("clone"));
    }

    #[test]
    fn test_deterministic_lookup() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("det", 0).unwrap();
        let reg = builder.build();
        let r1 = reg.lookup("det");
        let r2 = reg.lookup("det");
        assert_eq!(r1.is_some(), r2.is_some());
    }

    #[test]
    fn test_function_names_iterator() {
        let mut builder = NativeFunctionRegistryBuilder::new();
        builder.register("alpha", 0).unwrap();
        builder.register("beta", 0).unwrap();
        let reg = builder.build();
        let names: Vec<&String> = reg.function_names().collect();
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| *n == "alpha"));
        assert!(names.iter().any(|n| *n == "beta"));
    }
}

//! Native function abstraction for built-in functions.
//!
//! Defines the NativeFunction descriptor used by the registry.
//! Case-sensitive: function names match exactly.

/// Maximum allowed arity for a native function.
/// Prevents absurd registrations and potential misuse.
pub const MAX_NATIVE_ARITY: usize = 256;

/// Descriptor for a native (built-in) function.
/// Contains only metadata for semantic validation; no execution logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunction {
    /// Function name. Case-sensitive.
    pub name: String,
    /// Expected number of arguments.
    pub arity: usize,
}

impl NativeFunction {
    /// Creates a new native function descriptor.
    ///
    /// # Errors
    /// Returns `NativeFunctionError::ArityTooLarge` if arity exceeds `MAX_NATIVE_ARITY`.
    pub fn new(name: String, arity: usize) -> Result<Self, NativeFunctionError> {
        if arity > MAX_NATIVE_ARITY {
            return Err(NativeFunctionError::ArityTooLarge {
                name: name.clone(),
                arity,
                max: MAX_NATIVE_ARITY,
            });
        }
        Ok(Self { name, arity })
    }

    /// Returns the expected argument count.
    pub fn arity(&self) -> usize {
        self.arity
    }

    /// Returns the function name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Errors specific to native function registration and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFunctionError {
    /// Attempted to register a native function that is already registered.
    DuplicateRegistration { name: String },
    /// Lookup or validation for a function that is not in the registry.
    FunctionNotFound { name: String },
    /// Call has wrong number of arguments.
    WrongArgumentCount {
        name: String,
        expected: usize,
        actual: usize,
    },
    /// Arity exceeds maximum allowed.
    ArityTooLarge {
        name: String,
        arity: usize,
        max: usize,
    },
}

impl std::fmt::Display for NativeFunctionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NativeFunctionError::DuplicateRegistration { name } => {
                write!(f, "Native function '{}' is already registered", name)
            }
            NativeFunctionError::FunctionNotFound { name } => {
                write!(f, "Unknown native function '{}'", name)
            }
            NativeFunctionError::WrongArgumentCount {
                name,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Function '{}' expects {} argument(s), got {}",
                    name, expected, actual
                )
            }
            NativeFunctionError::ArityTooLarge { name, arity, max } => {
                write!(
                    f,
                    "Native function '{}' has arity {} which exceeds maximum {}",
                    name, arity, max
                )
            }
        }
    }
}

impl std::error::Error for NativeFunctionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_function_new_valid() {
        let func = NativeFunction::new("foo".to_string(), 2);
        assert!(func.is_ok());
        let func = func.unwrap();
        assert_eq!(func.name(), "foo");
        assert_eq!(func.arity(), 2);
    }

    #[test]
    fn test_native_function_new_zero_arity() {
        let func = NativeFunction::new("noargs".to_string(), 0);
        assert!(func.is_ok());
        assert_eq!(func.unwrap().arity(), 0);
    }

    #[test]
    fn test_native_function_new_max_arity() {
        let func = NativeFunction::new("many".to_string(), MAX_NATIVE_ARITY);
        assert!(func.is_ok());
        assert_eq!(func.unwrap().arity(), MAX_NATIVE_ARITY);
    }

    #[test]
    fn test_native_function_new_arity_too_large() {
        let result = NativeFunction::new("huge".to_string(), MAX_NATIVE_ARITY + 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        matches!(err, NativeFunctionError::ArityTooLarge { .. });
        let s = format!("{}", err);
        assert!(s.contains("huge"));
        assert!(s.contains("exceeds maximum"));
    }

    #[test]
    fn test_native_function_clone() {
        let func = NativeFunction::new("dup".to_string(), 1).unwrap();
        let cloned = func.clone();
        assert_eq!(func.name, cloned.name);
        assert_eq!(func.arity, cloned.arity);
    }

    #[test]
    fn test_native_function_error_display() {
        let err = NativeFunctionError::DuplicateRegistration {
            name: "bar".to_string(),
        };
        let s = format!("{}", err);
        assert!(s.contains("bar"));
        assert!(s.contains("already registered"));

        let err = NativeFunctionError::FunctionNotFound {
            name: "baz".to_string(),
        };
        let s = format!("{}", err);
        assert!(s.contains("baz"));
        assert!(s.contains("Unknown"));

        let err = NativeFunctionError::WrongArgumentCount {
            name: "qux".to_string(),
            expected: 2,
            actual: 3,
        };
        let s = format!("{}", err);
        assert!(s.contains("qux"));
        assert!(s.contains("2"));
        assert!(s.contains("3"));
    }

    #[test]
    fn test_deterministic_creation() {
        let f1 = NativeFunction::new("a".to_string(), 1);
        let f2 = NativeFunction::new("a".to_string(), 1);
        assert_eq!(f1.is_ok(), f2.is_ok());
        if let (Ok(a), Ok(b)) = (f1, f2) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.arity, b.arity);
        }
    }
}

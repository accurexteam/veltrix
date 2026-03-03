//! Native function registry infrastructure.
//!
//! Provides read-only metadata for built-in functions, used during semantic analysis.
//! No runtime execution; no global mutable state.

pub mod function;
pub mod registry;

pub use function::{NativeFunction, NativeFunctionError, MAX_NATIVE_ARITY};
pub use registry::{
    NativeFunctionRegistry, NativeFunctionRegistryBuilder, NativeFunctionValidationError,
};

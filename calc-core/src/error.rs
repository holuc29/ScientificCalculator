//! Error type shared by every stage of the engine.

use std::fmt;

/// Everything that can go wrong while parsing or evaluating an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcError {
    /// The input could not be tokenized or parsed.
    Syntax(String),
    /// A name that is neither a constant nor a defined variable.
    UnknownIdentifier(String),
    /// A function name that is not registered.
    UnknownFunction(String),
    /// A function was called with the wrong number of arguments.
    Arity {
        name: String,
        expected: String,
        got: usize,
    },
    /// Division by zero (`x / 0`, `x mod 0`, `0^-1`, ...).
    DivisionByZero,
    /// An argument outside the function's domain (`sqrt(-1)`, `ln(0)`, ...).
    Domain(String),
    /// A mathematically undefined result (`0/0`, `0^0`, `tan(90)` in degrees, ...).
    Undefined(String),
    /// The result is too large to represent.
    Overflow,
    /// The result is too small to represent (it would be a non-zero number
    /// closer to zero than the smallest supported magnitude).
    Underflow,
    /// A variable could not be assigned (reserved name, ...).
    Assignment(String),
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcError::Syntax(m) => write!(f, "Syntax error: {m}"),
            CalcError::UnknownIdentifier(n) => write!(f, "Unknown name '{n}'"),
            CalcError::UnknownFunction(n) => write!(f, "Unknown function '{n}'"),
            CalcError::Arity {
                name,
                expected,
                got,
            } => write!(f, "{name}() expects {expected} argument(s), got {got}"),
            CalcError::DivisionByZero => write!(f, "Cannot divide by zero"),
            CalcError::Domain(m) => write!(f, "Invalid input: {m}"),
            CalcError::Undefined(m) => write!(f, "Result is undefined: {m}"),
            CalcError::Overflow => write!(f, "Overflow: the result is too large"),
            CalcError::Underflow => write!(f, "Underflow: the result is too small to represent"),
            CalcError::Assignment(m) => write!(f, "Cannot assign: {m}"),
        }
    }
}

impl std::error::Error for CalcError {}

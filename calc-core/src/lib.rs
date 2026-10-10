//! # calc-core
//!
//! An arbitrary-precision scientific calculator engine with no UI dependencies.
//!
//! ```
//! use calc_core::Calculator;
//!
//! let mut calc = Calculator::new();
//! assert_eq!(calc.eval_str("0.1 + 0.2").unwrap(), "0.3");
//! assert_eq!(calc.eval_str("sin(30)").unwrap(), "0.5"); // degrees by default
//! assert_eq!(calc.eval_str("2^10").unwrap(), "1024");
//! ```
//!
//! Pipeline: [`lexer`] -> [`parser`] (AST) -> [`eval`], with all math in
//! [`ops`] on top of the [`num`] types, and callable functions in [`functions`].

pub mod calculator;
pub mod error;
pub mod eval;
pub mod fasteval;
pub mod functions;
pub mod lexer;
pub mod num;
pub mod ops;
pub mod parser;
pub mod programmer;
pub mod symbolic;
pub mod userfn;

pub use calculator::{Calculator, HistoryEntry};
pub use error::CalcError;
pub use programmer::{Base, Outcome, ProgValue, WordSize};
pub use fasteval::PlotFn;
pub use symbolic::Derivative;
pub use functions::{Arity, FunctionDef, Registry};
pub use num::{AngleMode, Ctx, Num, DEFAULT_DISPLAY_DIGITS, MAX_DISPLAY_DIGITS, PRECISION};

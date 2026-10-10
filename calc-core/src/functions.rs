//! The function registry.
//!
//! Every function callable from an expression lives here. Adding one is a
//! single `register` call, which is how future features (statistics,
//! regression, matrices, ...) are meant to be plugged in:
//!
//! ```
//! use calc_core::{Arity, Calculator};
//!
//! let mut calc = Calculator::new();
//! calc.registry_mut().register("double", Arity::Exact(1), "twice the argument", |ctx, a| {
//!     ctx.mul(&a[0], &calc_core::Num::from_i64(2))
//! });
//! assert_eq!(calc.eval_str("double(21)").unwrap(), "42");
//! ```

use std::collections::HashMap;
use std::fmt;

use crate::error::CalcError;
use crate::num::{Ctx, Num};

/// How many arguments a function accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    Exact(usize),
    AtLeast(usize),
    Range(usize, usize),
}

impl Arity {
    pub fn accepts(self, n: usize) -> bool {
        match self {
            Arity::Exact(k) => n == k,
            Arity::AtLeast(k) => n >= k,
            Arity::Range(lo, hi) => (lo..=hi).contains(&n),
        }
    }
}

impl fmt::Display for Arity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arity::Exact(k) => write!(f, "{k}"),
            Arity::AtLeast(k) => write!(f, "{k} or more"),
            Arity::Range(lo, hi) => write!(f, "{lo} to {hi}"),
        }
    }
}

/// The signature every registered function has.
pub type FuncImpl = Box<dyn Fn(&mut Ctx, &[Num]) -> Result<Num, CalcError>>;

pub struct FunctionDef {
    pub name: String,
    pub arity: Arity,
    pub help: String,
    pub(crate) f: FuncImpl,
}

#[derive(Default)]
pub struct Registry {
    map: HashMap<String, FunctionDef>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry containing every built-in function.
    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        register_builtins(&mut r);
        r
    }

    pub fn register<F>(&mut self, name: &str, arity: Arity, help: &str, f: F)
    where
        F: Fn(&mut Ctx, &[Num]) -> Result<Num, CalcError> + 'static,
    {
        let name = name.to_lowercase();
        self.map.insert(
            name.clone(),
            FunctionDef {
                name,
                arity,
                help: help.to_string(),
                f: Box::new(f),
            },
        );
    }

    /// Registers a function of exactly one argument.
    fn reg1(&mut self, name: &str, help: &str, f: fn(&mut Ctx, &Num) -> Result<Num, CalcError>) {
        self.register(name, Arity::Exact(1), help, move |c, a| f(c, &a[0]));
    }

    /// Registers a function of exactly two arguments.
    fn reg2(
        &mut self,
        name: &str,
        help: &str,
        f: fn(&mut Ctx, &Num, &Num) -> Result<Num, CalcError>,
    ) {
        self.register(name, Arity::Exact(2), help, move |c, a| f(c, &a[0], &a[1]));
    }

    pub fn get(&self, name: &str) -> Option<&FunctionDef> {
        self.map.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// All functions, sorted by name.
    pub fn list(&self) -> Vec<&FunctionDef> {
        let mut v: Vec<_> = self.map.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }
}

fn register_builtins(r: &mut Registry) {
    // Powers, roots, exponentials, logarithms
    r.reg1("sqrt", "square root", |c, x| c.sqrt(x));
    r.reg1("cbrt", "cube root", |c, x| c.cbrt(x));
    r.reg2("root", "root(x, n): the n-th root of x", |c, x, n| c.root(x, n));
    r.reg1("exp", "e raised to x", |c, x| c.exp(x));
    r.reg1("ln", "natural logarithm", |c, x| c.ln(x));
    r.reg1("log", "base-10 logarithm", |c, x| c.log10(x));
    r.reg1("log10", "base-10 logarithm", |c, x| c.log10(x));
    r.reg1("log2", "base-2 logarithm", |c, x| c.log2(x));
    r.reg2("logb", "logb(x, b): logarithm of x to base b", |c, x, b| c.logb(x, b));

    // Rounding and sign
    r.reg1("abs", "absolute value", |c, x| Ok(c.abs(x)));
    r.reg1("floor", "largest integer <= x", |c, x| Ok(c.floor(x)));
    r.reg1("ceil", "smallest integer >= x", |c, x| Ok(c.ceil(x)));
    r.reg1("trunc", "integer part (towards zero)", |c, x| Ok(c.trunc(x)));
    r.reg1("frac", "fractional part", |c, x| Ok(c.frac(x)));
    r.reg1("round", "round half away from zero", |c, x| c.round(x));
    r.reg1("sign", "-1, 0 or 1", |c, x| Ok(c.sign(x)));

    // Trigonometry (angle mode applies)
    r.reg1("sin", "sine (angle mode applies)", |c, x| c.sin(x));
    r.reg1("cos", "cosine (angle mode applies)", |c, x| c.cos(x));
    r.reg1("tan", "tangent (angle mode applies)", |c, x| c.tan(x));
    r.reg1("sec", "secant (angle mode applies)", |c, x| c.sec(x));
    r.reg1("csc", "cosecant (angle mode applies)", |c, x| c.csc(x));
    r.reg1("cot", "cotangent (angle mode applies)", |c, x| c.cot(x));
    r.reg1("asin", "inverse sine, result in the angle mode", |c, x| c.asin(x));
    r.reg1("acos", "inverse cosine, result in the angle mode", |c, x| c.acos(x));
    r.reg1("atan", "inverse tangent, result in the angle mode", |c, x| c.atan(x));
    r.reg1("asec", "inverse secant", |c, x| c.asec(x));
    r.reg1("acsc", "inverse cosecant", |c, x| c.acsc(x));
    r.reg1("acot", "inverse cotangent, range (0, pi)", |c, x| c.acot(x));

    // Hyperbolic
    r.reg1("sinh", "hyperbolic sine", |c, x| c.sinh(x));
    r.reg1("cosh", "hyperbolic cosine", |c, x| c.cosh(x));
    r.reg1("tanh", "hyperbolic tangent", |c, x| c.tanh(x));
    r.reg1("sech", "hyperbolic secant", |c, x| c.sech(x));
    r.reg1("csch", "hyperbolic cosecant", |c, x| c.csch(x));
    r.reg1("coth", "hyperbolic cotangent", |c, x| c.coth(x));
    r.reg1("asinh", "inverse hyperbolic sine", |c, x| c.asinh(x));
    r.reg1("acosh", "inverse hyperbolic cosine", |c, x| c.acosh(x));
    r.reg1("atanh", "inverse hyperbolic tangent", |c, x| c.atanh(x));
    r.reg1("asech", "inverse hyperbolic secant", |c, x| c.asech(x));
    r.reg1("acsch", "inverse hyperbolic cosecant", |c, x| c.acsch(x));
    r.reg1("acoth", "inverse hyperbolic cotangent", |c, x| c.acoth(x));

    // Combinatorics and conversions
    r.reg1("fact", "factorial (same as x!)", |c, x| c.factorial(x));
    r.reg2("ncr", "ncr(n, k): combinations", |c, n, k| c.ncr(n, k));
    r.reg2("npr", "npr(n, k): permutations", |c, n, k| c.npr(n, k));
    r.reg1("dms", "decimal degrees to d.mmss (10.5 -> 10.3)", |c, x| c.to_dms(x));
    r.reg1("deg", "d.mmss to decimal degrees (10.3 -> 10.5)", |c, x| c.from_dms(x));

    // Random numbers
    r.register("rand", Arity::Exact(0), "random number in [0, 1)", |c, _| {
        Ok(c.next_random())
    });

    // Variadic helpers: a first taste of "analysis" functions.
    r.register("sum", Arity::AtLeast(1), "sum of the arguments", |c, a| {
        let mut acc = Num::zero();
        for x in a {
            acc = c.add(&acc, x)?;
        }
        Ok(acc)
    });
    r.register("mean", Arity::AtLeast(1), "arithmetic mean of the arguments", |c, a| {
        let mut acc = Num::zero();
        for x in a {
            acc = c.add(&acc, x)?;
        }
        c.div(&acc, &Num::from_u64(a.len() as u64))
    });
    r.register("min", Arity::AtLeast(1), "smallest argument", |_, a| {
        let mut best = a[0].clone();
        for x in &a[1..] {
            if *x < best {
                best = x.clone();
            }
        }
        Ok(best)
    });
    r.register("max", Arity::AtLeast(1), "largest argument", |_, a| {
        let mut best = a[0].clone();
        for x in &a[1..] {
            if *x > best {
                best = x.clone();
            }
        }
        Ok(best)
    });
}

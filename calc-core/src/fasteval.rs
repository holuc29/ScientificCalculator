//! A fast `f64` evaluator for plotting.
//!
//! The main engine works with 256-bit numbers, which is far too slow to
//! evaluate a function a few thousand times per frame. [`compile`] turns an
//! expression into nested closures over `f64`. The results are only meant for
//! drawing: `NaN` and infinities mean "undefined here" and are never errors.

use std::f64::consts::{E, PI};

use crate::error::CalcError;
use crate::functions::Registry;
use crate::num::AngleMode;
use crate::parser::{Expr, Op};

type Node = Box<dyn Fn(f64) -> f64 + Send + Sync>;

/// A compiled function of one variable.
pub struct PlotFn {
    f: Node,
}

impl PlotFn {
    /// Value at `x`; `NaN` or infinite where the function is undefined.
    pub fn eval(&self, x: f64) -> f64 {
        (self.f)(x)
    }
}

/// Compiles `expr` as a function of `var`. `lookup` supplies the value of any
/// other name (user variables, `ans`, `mem`).
pub fn compile(
    expr: &Expr,
    var: &str,
    registry: &Registry,
    angle: AngleMode,
    lookup: &mut dyn FnMut(&str) -> Result<f64, CalcError>,
) -> Result<PlotFn, CalcError> {
    let c = Compiler {
        var,
        registry,
        angle,
    };
    Ok(PlotFn {
        f: c.node(expr, lookup)?,
    })
}

struct Compiler<'a> {
    var: &'a str,
    registry: &'a Registry,
    angle: AngleMode,
}

fn konst(v: f64) -> Node {
    Box::new(move |_| v)
}

/// Radians per angle unit.
fn unit(angle: AngleMode) -> f64 {
    match angle {
        AngleMode::Radians => 1.0,
        AngleMode::Degrees => PI / 180.0,
        AngleMode::Gradians => PI / 200.0,
    }
}

/// Half a turn in the angle unit.
fn half_turn(angle: AngleMode) -> f64 {
    match angle {
        AngleMode::Radians => PI,
        AngleMode::Degrees => 180.0,
        AngleMode::Gradians => 200.0,
    }
}

// Trigonometry with exact zeros at the multiples of a quarter turn in degree
// and gradian mode, mirroring the main engine (cos(90) = 0, tan(90) undefined).
fn sin_a(x: f64, a: AngleMode) -> f64 {
    if a != AngleMode::Radians && x % half_turn(a) == 0.0 {
        return 0.0;
    }
    (x * unit(a)).sin()
}

fn cos_a(x: f64, a: AngleMode) -> f64 {
    if a != AngleMode::Radians && (x % half_turn(a)).abs() == half_turn(a) / 2.0 {
        return 0.0;
    }
    (x * unit(a)).cos()
}

fn is_int(x: f64) -> bool {
    x.is_finite() && x == x.trunc()
}

fn factorial(x: f64) -> f64 {
    if !is_int(x) || !(0.0..=170.0).contains(&x) {
        return f64::NAN;
    }
    (2..=x as u64).fold(1.0, |acc, k| acc * k as f64)
}

fn ncr(n: f64, k: f64) -> f64 {
    if !is_int(n) || !is_int(k) || n < 0.0 || k < 0.0 || k > n || n > 1e9 {
        return f64::NAN;
    }
    let k = k.min(n - k) as u64;
    let mut r = 1.0f64;
    for i in 1..=k {
        r = r * (n - k as f64 + i as f64) / i as f64;
    }
    r.round()
}

fn npr(n: f64, k: f64) -> f64 {
    if !is_int(n) || !is_int(k) || n < 0.0 || k < 0.0 || k > n || k > 1e6 {
        return f64::NAN;
    }
    (0..k as u64).fold(1.0, |acc, i| acc * (n - i as f64))
}

fn root(x: f64, n: f64) -> f64 {
    let n = n.round();
    if n == 0.0 || !n.is_finite() {
        return f64::NAN;
    }
    if x < 0.0 {
        // Real root of a negative number: odd integer degree only.
        if n.abs() % 2.0 == 1.0 {
            return -(-x).powf(1.0 / n);
        }
        return f64::NAN;
    }
    x.powf(1.0 / n)
}

impl Compiler<'_> {
    fn node(
        &self,
        e: &Expr,
        lookup: &mut dyn FnMut(&str) -> Result<f64, CalcError>,
    ) -> Result<Node, CalcError> {
        Ok(match e {
            Expr::Num(text) => {
                let v: f64 = text.parse().map_err(|_| {
                    CalcError::Syntax(format!("bad number '{text}'"))
                })?;
                konst(v)
            }
            Expr::Var(name) => {
                if name == self.var {
                    Box::new(|x| x)
                } else {
                    match name.as_str() {
                        "pi" => konst(PI),
                        "e" => konst(E),
                        _ => konst(lookup(name)?),
                    }
                }
            }
            Expr::Neg(a) => {
                let a = self.node(a, lookup)?;
                Box::new(move |x| -a(x))
            }
            Expr::Fact(a) => {
                let a = self.node(a, lookup)?;
                Box::new(move |x| factorial(a(x)))
            }
            Expr::Bin(op, l, r) => {
                let l = self.node(l, lookup)?;
                let r = self.node(r, lookup)?;
                match op {
                    Op::Add => Box::new(move |x| l(x) + r(x)),
                    Op::Sub => Box::new(move |x| l(x) - r(x)),
                    Op::Mul => Box::new(move |x| l(x) * r(x)),
                    Op::Div => Box::new(move |x| l(x) / r(x)),
                    Op::Mod => Box::new(move |x| l(x) % r(x)),
                    Op::Pow => Box::new(move |x| {
                        let (a, b) = (l(x), r(x));
                        if a == 0.0 && b == 0.0 {
                            f64::NAN // 0^0 is undefined, as in the main engine
                        } else {
                            a.powf(b)
                        }
                    }),
                }
            }
            Expr::Assign(..) => {
                return Err(CalcError::Syntax(
                    "an assignment cannot be plotted".into(),
                ))
            }
            Expr::Call(name, args) => self.call(name, args, lookup)?,
        })
    }

    fn call(
        &self,
        name: &str,
        args: &[Expr],
        lookup: &mut dyn FnMut(&str) -> Result<f64, CalcError>,
    ) -> Result<Node, CalcError> {
        let def = self
            .registry
            .get(name)
            .ok_or_else(|| CalcError::UnknownFunction(name.to_string()))?;
        if !def.arity.accepts(args.len()) {
            return Err(CalcError::Arity {
                name: name.to_string(),
                expected: def.arity.to_string(),
                got: args.len(),
            });
        }
        let mut a: Vec<Node> = Vec::with_capacity(args.len());
        for x in args {
            a.push(self.node(x, lookup)?);
        }
        let ang = self.angle;

        macro_rules! f1 {
            ($f:expr) => {{
                let a0 = a.remove(0);
                let f: fn(f64) -> f64 = $f;
                Box::new(move |x| f(a0(x))) as Node
            }};
        }
        macro_rules! f1a {
            // one-argument function that needs the angle mode
            ($f:expr) => {{
                let a0 = a.remove(0);
                let f: fn(f64, AngleMode) -> f64 = $f;
                Box::new(move |x| f(a0(x), ang)) as Node
            }};
        }
        macro_rules! f2 {
            ($f:expr) => {{
                let a1 = a.remove(1);
                let a0 = a.remove(0);
                let f: fn(f64, f64) -> f64 = $f;
                Box::new(move |x| f(a0(x), a1(x))) as Node
            }};
        }

        Ok(match name {
            "sqrt" => f1!(f64::sqrt),
            "cbrt" => f1!(f64::cbrt),
            "root" => f2!(root),
            "exp" => f1!(f64::exp),
            "ln" => f1!(|v| if v > 0.0 { v.ln() } else { f64::NAN }),
            "log" | "log10" => f1!(|v| if v > 0.0 { v.log10() } else { f64::NAN }),
            "log2" => f1!(|v| if v > 0.0 { v.log2() } else { f64::NAN }),
            "logb" => f2!(|v, b| if v > 0.0 && b > 0.0 && b != 1.0 {
                v.ln() / b.ln()
            } else {
                f64::NAN
            }),

            "abs" => f1!(f64::abs),
            "floor" => f1!(f64::floor),
            "ceil" => f1!(f64::ceil),
            "trunc" => f1!(f64::trunc),
            "frac" => f1!(|v| v - v.trunc()),
            "round" => f1!(f64::round),
            "sign" => f1!(|v| if v == 0.0 { 0.0 } else { v.signum() }),

            "sin" => f1a!(sin_a),
            "cos" => f1a!(cos_a),
            "tan" => f1a!(|v, a| {
                let c = cos_a(v, a);
                if c == 0.0 {
                    f64::NAN
                } else {
                    sin_a(v, a) / c
                }
            }),
            "sec" => f1a!(|v, a| {
                let c = cos_a(v, a);
                if c == 0.0 {
                    f64::NAN
                } else {
                    1.0 / c
                }
            }),
            "csc" => f1a!(|v, a| {
                let s = sin_a(v, a);
                if s == 0.0 {
                    f64::NAN
                } else {
                    1.0 / s
                }
            }),
            "cot" => f1a!(|v, a| {
                let s = sin_a(v, a);
                if s == 0.0 {
                    f64::NAN
                } else {
                    cos_a(v, a) / s
                }
            }),
            "asin" => f1a!(|v, a| v.asin() / unit(a)),
            "acos" => f1a!(|v, a| v.acos() / unit(a)),
            "atan" => f1a!(|v, a| v.atan() / unit(a)),
            "asec" => f1a!(|v, a| (1.0 / v).acos() / unit(a)),
            "acsc" => f1a!(|v, a| (1.0 / v).asin() / unit(a)),
            "acot" => f1a!(|v, a| {
                let r = if v == 0.0 {
                    PI / 2.0
                } else if v > 0.0 {
                    (1.0 / v).atan()
                } else {
                    (1.0 / v).atan() + PI
                };
                r / unit(a)
            }),

            "sinh" => f1!(f64::sinh),
            "cosh" => f1!(f64::cosh),
            "tanh" => f1!(f64::tanh),
            "sech" => f1!(|v| 1.0 / v.cosh()),
            "csch" => f1!(|v| if v == 0.0 { f64::NAN } else { 1.0 / v.sinh() }),
            "coth" => f1!(|v| if v == 0.0 { f64::NAN } else { v.cosh() / v.sinh() }),
            "asinh" => f1!(f64::asinh),
            "acosh" => f1!(f64::acosh),
            "atanh" => f1!(f64::atanh),
            "asech" => f1!(|v| (1.0 / v).acosh()),
            "acsch" => f1!(|v| if v == 0.0 { f64::NAN } else { (1.0 / v).asinh() }),
            "acoth" => f1!(|v| (1.0 / v).atanh()),

            "fact" => f1!(factorial),
            "ncr" => f2!(ncr),
            "npr" => f2!(npr),

            "sum" => Box::new(move |x| a.iter().map(|f| f(x)).sum()),
            "mean" => {
                let n = a.len() as f64;
                Box::new(move |x| a.iter().map(|f| f(x)).sum::<f64>() / n)
            }
            "min" => Box::new(move |x| {
                a.iter().map(|f| f(x)).fold(f64::INFINITY, |m, v| {
                    if v.is_nan() || m.is_nan() {
                        f64::NAN
                    } else {
                        m.min(v)
                    }
                })
            }),
            "max" => Box::new(move |x| {
                a.iter().map(|f| f(x)).fold(f64::NEG_INFINITY, |m, v| {
                    if v.is_nan() || m.is_nan() {
                        f64::NAN
                    } else {
                        m.max(v)
                    }
                })
            }),
            other => {
                return Err(CalcError::Domain(format!(
                    "'{other}' cannot be used in a plot"
                )))
            }
        })
    }
}

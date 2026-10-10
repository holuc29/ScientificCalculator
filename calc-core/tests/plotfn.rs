//! Cross-checks the fast f64 evaluator against the 256-bit engine.

use calc_core::{AngleMode, Calculator, Num};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

const EXPRS: &[&str] = &[
    "x^2 + 3x - 1", "sqrt(x)", "cbrt(x)", "root(x, 3)", "root(x, 4)", "exp(x/3)", "ln(x)", "log(x)",
    "log2(x)", "logb(x, 3)", "abs(x) - 2", "floor(x)", "ceil(x)", "trunc(x)", "frac(x)", "round(x)",
    "sign(x)", "sin(x)", "cos(x)", "tan(x)", "sec(x)", "csc(x)", "cot(x)", "asin(x/10)",
    "acos(x/10)", "atan(x)", "asec(x)", "acsc(x)", "acot(x)", "sinh(x/3)", "cosh(x/3)", "tanh(x)",
    "sech(x)", "csch(x)", "coth(x)", "asinh(x)", "acosh(x)", "atanh(x/10)", "asech(x/10)",
    "acsch(x)", "acoth(x)", "x!", "ncr(x, 3)", "npr(x, 2)", "x mod 3", "x / (x - 1)",
    "x^x", "2^x", "x^0.5", "(-x)^3", "sum(x, 2x, 1)", "mean(x, 3)", "min(x, 2)", "max(x, -2)",
    "pi x + e", "sin(x)/x", "-x^2",
];

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) + 1e-12
}

#[test]
fn matches_engine() {
    let mut rng = Lcg(12345);
    for mode in [AngleMode::Degrees, AngleMode::Radians, AngleMode::Gradians] {
        let mut c = Calculator::new();
        c.set_angle_mode(mode);
        for src in EXPRS {
            let f = c.plot_fn(src, "x").unwrap_or_else(|e| panic!("{src}: {e}"));
            let mut checked = 0;
            for i in 0..120 {
                // mix of integers, halves and random reals in [-12, 12]
                let x = match i % 3 {
                    0 => (rng.next() * 24.0 - 12.0).round(),
                    1 => ((rng.next() * 48.0 - 24.0).round()) / 2.0,
                    _ => rng.next() * 24.0 - 12.0,
                };
                let fast = f.eval(x);
                let slow = c.eval_with(src, "x", &Num::from_f64(x));
                match slow {
                    Ok(n) => {
                        let s = c.to_f64(&n).unwrap();
                        assert!(
                            close(fast, s),
                            "{src} at x={x} ({mode:?}): fast {fast} vs engine {s}"
                        );
                        checked += 1;
                    }
                    Err(_) => assert!(
                        !fast.is_finite(),
                        "{src} at x={x} ({mode:?}): engine errors but fast gave {fast}"
                    ),
                }
            }
            assert!(checked > 0, "{src}: no valid sample points");
        }
    }
}

#[test]
fn variables_functions_and_definitions() {
    let mut c = Calculator::new();
    c.eval_str("a = 3").unwrap();
    c.define_function("f(x) = a*x^2").unwrap();
    let f = c.plot_fn("f(x) + 1", "x").unwrap();
    assert_eq!(f.eval(2.0), 13.0);
    // a definition text is compiled without being stored; first parameter is the variable
    let g = c.plot_fn("g(t) = t + a", "x").unwrap();
    assert_eq!(g.eval(1.0), 4.0);
    assert!(c.functions().iter().all(|f| f.0 != "g"));
    // unknown names are reported
    assert!(c.plot_fn("x + zzz", "x").is_err());
    assert!(c.plot_fn("rand()", "x").is_err());
    assert!(c.plot_fn("x = 3", "x").is_err());
}

#[test]
fn exact_zeros_and_poles() {
    let mut c = Calculator::new(); // degrees
    let tan = c.plot_fn("tan(x)", "x").unwrap();
    assert!(tan.eval(90.0).is_nan());
    assert!(tan.eval(270.0).is_nan());
    assert_eq!(c.plot_fn("sin(x)", "x").unwrap().eval(180.0), 0.0);
    assert_eq!(c.plot_fn("cos(x)", "x").unwrap().eval(90.0), 0.0);
}

//! Symbolic differentiation: every derivative is checked against an
//! independent central finite difference computed with the same 256-bit
//! arithmetic (the differentiation code is not involved in the check).

use calc_core::{AngleMode, CalcError, Calculator, Num};

fn calc(mode: AngleMode) -> Calculator {
    let mut c = Calculator::new();
    c.set_angle_mode(mode);
    c.set_display_digits(60);
    c
}

fn num(c: &mut Calculator, text: &str) -> Num {
    c.eval(text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// Value of `expr` (in `x`) at `x0`, as a 60-digit string.
fn at(c: &mut Calculator, expr: &str, x0: &str) -> String {
    let x = num(c, x0);
    let v = c
        .eval_with(expr, "x", &x)
        .unwrap_or_else(|e| panic!("evaluating {expr} at {x0}: {e}"));
    c.format(&v)
}

/// Central difference of `expr` at `x0` with step 1e-25.
fn finite_difference(c: &mut Calculator, expr: &str, x0: &str) -> String {
    let p = format!("({x0})+1e-25");
    let m = format!("({x0})-1e-25");
    let fp = at(c, expr, &p);
    let fm = at(c, expr, &m);
    let (pp, mm) = (num(c, &p), num(c, &m));
    let (ps, ms) = (c.format(&pp), c.format(&mm));
    let v = num(c, &format!("(({fp}) - ({fm})) / (({ps}) - ({ms}))"));
    c.format(&v)
}

fn assert_close(c: &mut Calculator, a: &str, b: &str, what: &str) {
    let rel = num(c, &format!("abs(({a}) - ({b})) / max(1, abs({b}))"));
    let r = c.to_f64(&rel).unwrap();
    assert!(r < 1e-25, "{what}: symbolic {a} vs numeric {b} (relative gap {r:e})");
}

fn check_all(mode: AngleMode, cases: &[(&str, &str)]) {
    let mut c = calc(mode);
    for (expr, x0) in cases {
        let d = c
            .differentiate(expr, "x", 1)
            .unwrap_or_else(|e| panic!("d/dx {expr}: {e}"));
        let symbolic = at(&mut c, &d.expr, x0);
        let numeric = finite_difference(&mut c, expr, x0);
        assert_close(&mut c, &symbolic, &numeric, &format!("d/dx {expr} = {} at {x0}", d.expr));
    }
}

#[test]
fn rules_match_finite_differences_in_radians() {
    check_all(
        AngleMode::Radians,
        &[
            ("x", "1.3"),
            ("7", "1.3"),
            ("x^2", "1.3"),
            ("x^3 - 4*x^2 + x - 9", "2.7"),
            ("3*x^5/7", "0.9"),
            ("1/x", "1.7"),
            ("1/x^3", "-1.4"),
            ("(x+1)/(x^2+3)", "0.8"),
            ("x*(x+2)*(x-3)", "1.1"),
            ("(2*x+1)^5", "0.3"),
            ("x^2*sin(x)", "1.2"),
            ("sin(x)/x", "0.7"),
            ("x^x", "1.9"),
            ("x^(2*x)", "1.1"),
            ("2^x", "1.5"),
            ("2^(x^2)", "0.6"),
            ("x^0.5", "3.2"),
            ("x^pi", "1.8"),
            ("e^x", "0.4"),
            ("exp(2*x)", "0.4"),
            ("exp(-x^2)", "0.9"),
            ("ln(x)", "2.3"),
            ("ln(x^2+1)", "1.6"),
            ("log(x)", "2.3"),
            ("log10(3*x)", "1.7"),
            ("log2(x^2)", "1.7"),
            ("logb(x, 5)", "2.1"),
            ("logb(7, x)", "2.1"),
            ("sqrt(x)", "2.6"),
            ("sqrt(1-x^2)", "0.4"),
            ("sqrt(x^2+1)", "-1.3"),
            ("cbrt(x)", "-8.5"),
            ("cbrt(x^2+1)", "1.5"),
            ("root(x, 4)", "2.5"),
            ("root(x, 3)", "-2.5"),
            ("root(x, 2*x)", "1.5"),
            ("sin(x)", "0.6"),
            ("cos(x)", "0.6"),
            ("tan(x)", "0.6"),
            ("sec(x)", "0.6"),
            ("csc(x)", "0.6"),
            ("cot(x)", "0.6"),
            ("sin(cos(x))", "0.9"),
            ("sin(x)^2 + cos(x)^2", "0.9"),
            ("tan(x^2)", "0.5"),
            ("asin(x)", "0.3"),
            ("acos(2*x)", "0.2"),
            ("atan(x)", "1.4"),
            ("atan(1/x)", "1.4"),
            ("asec(x)", "2.5"),
            ("asec(x)", "-2.5"),
            ("acsc(x)", "3.5"),
            ("acsc(x)", "-3.5"),
            ("acot(x)", "0.7"),
            ("acot(x)", "-0.7"),
            ("sinh(x)", "0.8"),
            ("cosh(x)", "0.8"),
            ("tanh(x)", "0.8"),
            ("sech(x)", "0.8"),
            ("csch(x)", "0.8"),
            ("coth(x)", "0.8"),
            ("sinh(x)*cosh(x)", "0.8"),
            ("asinh(x)", "-1.7"),
            ("acosh(x)", "2.5"),
            ("atanh(x)", "0.4"),
            ("asech(x)", "0.5"),
            ("acsch(x)", "0.7"),
            ("acsch(x)", "-0.7"),
            ("acoth(x)", "2.5"),
            ("abs(x)", "-2.2"),
            ("abs(x^2-4)", "1.1"),
            ("sign(x)*x", "1.1"),
            ("floor(x)", "1.5"),
            ("frac(x)", "1.5"),
            ("sum(x, x^2, 3)", "1.5"),
            ("mean(x, x^3)", "1.5"),
            ("x*(x mod 7)", "2.5"),
            ("sin(x)*cos(x)*exp(x)/(x^2+1)", "0.45"),
            ("ln(sin(x)^2 + 2)", "0.95"),
            ("x^3^2", "1.1"),
        ],
    );
}

#[test]
fn trigonometric_derivatives_follow_the_angle_mode() {
    let cases = [
        ("sin(x)", "30"),
        ("cos(2*x)", "20"),
        ("tan(x)", "10"),
        ("sec(x)", "10"),
        ("cot(3*x)", "7"),
        ("asin(x)", "0.4"),
        ("acos(x)", "0.4"),
        ("atan(x)", "2.5"),
        ("acot(x)", "2.5"),
        ("asec(x)", "2.5"),
        ("acsc(x)", "-2.5"),
        ("sinh(x)*sin(x)", "12"),
        ("sin(x)^2", "40"),
    ];
    check_all(AngleMode::Degrees, &cases);
    check_all(AngleMode::Gradians, &cases);
}

#[test]
fn second_and_higher_derivatives() {
    let mut c = calc(AngleMode::Radians);
    for (expr, x0) in [
        ("x^2*sin(x)", "1.2"),
        ("exp(2*x)/x", "0.8"),
        ("ln(x)*x^3", "1.7"),
        ("atan(x^2)", "0.6"),
    ] {
        let d1 = c.differentiate(expr, "x", 1).unwrap().expr;
        let d2 = c.differentiate(expr, "x", 2).unwrap().expr;
        let d3 = c.differentiate(expr, "x", 3).unwrap().expr;
        // Each order is the derivative of the one before, whatever its form.
        let (s2, n2) = (at(&mut c, &d2, x0), finite_difference(&mut c, &d1, x0));
        assert_close(&mut c, &s2, &n2, &format!("second derivative of {expr}"));
        let (s3, n3) = (at(&mut c, &d3, x0), finite_difference(&mut c, &d2, x0));
        assert_close(&mut c, &s3, &n3, &format!("third derivative of {expr}"));
    }
}

fn d(mode: AngleMode, expr: &str) -> String {
    calc(mode).differentiate(expr, "x", 1).unwrap().expr
}

fn dr(expr: &str) -> String {
    d(AngleMode::Radians, expr)
}

#[test]
fn results_are_simplified() {
    assert_eq!(dr("x^2"), "2*x");
    assert_eq!(dr("x"), "1");
    assert_eq!(dr("5"), "0");
    assert_eq!(dr("pi*x"), "pi");
    assert_eq!(dr("y*x"), "y"); // other names are constants
    assert_eq!(dr("x*x"), "2*x");
    assert_eq!(dr("3*x^2 + 2*x + 1"), "6*x + 2");
    assert_eq!(dr("x/3"), "1/3");
    assert_eq!(dr("0.5*x^2"), "x");
    assert_eq!(dr("x^2*sin(x)"), "2*x*sin(x) + x^2*cos(x)");
    assert_eq!(dr("sin(x)/x"), "(cos(x)*x - sin(x))/x^2");
    assert_eq!(dr("(x+1)^3"), "3*(x + 1)^2");
    assert_eq!(dr("ln(x)"), "1/x");
    assert_eq!(dr("exp(2*x)"), "2*exp(2*x)");
    assert_eq!(dr("e^x"), "e^x");
    assert_eq!(dr("2^x"), "2^x*ln(2)");
    assert_eq!(dr("x^x"), "x^x*(ln(x) + 1)");
    assert_eq!(dr("sqrt(x)"), "1/(2*sqrt(x))");
    assert_eq!(dr("asin(2*x)"), "2/sqrt(1 - 4*x^2)");
    assert_eq!(dr("sum(x, x^2)"), "1 + 2*x");
    // terms that cancel disappear
    assert_eq!(dr("sin(x)^2 + cos(x)^2"), "0");
}

#[test]
fn degrees_and_gradians_carry_the_conversion_factor() {
    assert_eq!(d(AngleMode::Degrees, "sin(x)"), "cos(x)*pi/180");
    assert_eq!(d(AngleMode::Gradians, "sin(x)"), "cos(x)*pi/200");
    assert_eq!(d(AngleMode::Degrees, "atan(x)"), "180/((1 + x^2)*pi)");
    let with_note = calc(AngleMode::Degrees).differentiate("sin(x)", "x", 1).unwrap();
    assert_eq!(with_note.notes.len(), 1);
    assert!(calc(AngleMode::Radians)
        .differentiate("sin(x)", "x", 1)
        .unwrap()
        .notes
        .is_empty());
}

#[test]
fn other_variables_and_user_variables() {
    let mut c = calc(AngleMode::Radians);
    assert_eq!(c.differentiate("t^2*x", "t", 1).unwrap().expr, "2*t*x");
    assert_eq!(c.differentiate("t^2*x", "x", 1).unwrap().expr, "t^2");
    c.eval("k = 3").unwrap();
    assert_eq!(c.differentiate("k*x^2", "x", 1).unwrap().expr, "2*k*x");
    // eval_with leaves the session untouched
    let x = c.eval("2").unwrap();
    let before = c.history().len();
    let v = c.eval_with("k*x^2", "x", &x).unwrap();
    assert_eq!(c.format(&v), "12");
    assert_eq!(c.history().len(), before);
    assert!(c.variables().iter().all(|(n, _)| *n != "x"));
}

#[test]
fn differentiation_errors() {
    let c = calc(AngleMode::Radians);
    let e = |s: &str, v: &str, o: usize| c.differentiate(s, v, o).unwrap_err();
    assert!(matches!(e("x!", "x", 1), CalcError::Domain(_)));
    assert!(matches!(e("min(x, 1)", "x", 1), CalcError::Domain(_)));
    assert!(matches!(e("ncr(x, 2)", "x", 1), CalcError::Domain(_)));
    assert!(matches!(e("x mod x", "x", 1), CalcError::Domain(_)));
    assert!(matches!(e("y = x", "x", 1), CalcError::Syntax(_)));
    assert!(matches!(e("x+", "x", 1), CalcError::Syntax(_)));
    assert!(matches!(e("x", "pi", 1), CalcError::Syntax(_)));
    assert!(matches!(e("x", "sin", 1), CalcError::Syntax(_)));
    assert!(matches!(e("x", "2x", 1), CalcError::Syntax(_)));
    assert!(matches!(e("x", "x", 0), CalcError::Domain(_)));
    assert!(matches!(e("x", "x", 11), CalcError::Domain(_)));
    // constants differentiate fine even if they contain unsupported functions
    assert_eq!(c.differentiate("ncr(5, 2)*x", "x", 1).unwrap().expr, "ncr(5, 2)");
}

#[test]
fn printed_results_read_back_identically() {
    // Parenthesisation: evaluating the printed text must give the same value
    // as the numeric derivative for awkward shapes (negative bases, nested
    // powers, subtraction chains, quotients of products).
    check_all(
        AngleMode::Radians,
        &[
            ("(-x)^3", "1.3"),
            ("-x^2 - x", "1.3"),
            ("x - (x^2 - x^3)", "1.3"),
            ("1/(x*(x+1)*(x+2))", "1.3"),
            ("x/(x/(x+1))", "1.3"),
            ("(x^2)^(1/3) * x", "1.3"),
            ("2^(3^x)", "0.4"),
            ("(2^x)^3", "0.4"),
            ("x^(-2)", "1.3"),
            ("x^(-x)", "1.3"),
        ],
    );
}

#[test]
fn huge_input_is_rejected_not_hung() {
    let c = calc(AngleMode::Radians);
    // x^x^x^... grows quickly with every order
    let r = c.differentiate("x^x^x^x", "x", 10);
    assert!(r.is_ok() || matches!(r, Err(CalcError::Domain(_))));
}

#[test]
fn latex_output_looks_right() {
    let c = calc(AngleMode::Radians);
    let l = |s: &str| c.differentiate(s, "x", 1).unwrap().latex;
    assert_eq!(l("sin(x)/x"), r"\frac{\cos(x)\,x - \sin(x)}{x^{2}}");
    assert_eq!(l("sqrt(1-x^2)"), r"-\frac{x}{\sqrt{1 - x^{2}}}");
    assert_eq!(l("x^x"), r"x^{x}\,(\ln(x) + 1)");
    assert_eq!(l("sin(x)^3"), r"3\,\sin^{2}(x)\,\cos(x)");
}

#[test]
fn square_roots_cancel_their_squares() {
    assert_eq!(dr("sqrt(x)*sqrt(x)"), "1");
    assert_eq!(dr("sqrt(x)^3"), "3*sqrt(x)/2");
    assert_eq!(dr("1/sqrt(x^2+1)"), "-x/((x^2 + 1)*sqrt(x^2 + 1))");
    check_all(
        AngleMode::Radians,
        &[
            ("sin(x)/sqrt(1+x^2)", "0.7"),
            ("sqrt(x)^3", "1.9"),
            ("x/sqrt(x^2+4)^3", "1.3"),
            ("sqrt(x)*sqrt(x+1)*sqrt(x)", "1.3"),
        ],
    );
}

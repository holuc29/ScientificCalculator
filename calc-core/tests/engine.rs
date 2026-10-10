//! Integration tests for the calculator engine.
//!
//! Expected values for non-trivial results were computed independently with
//! Python's mpmath (100 digits) and rounded to 32 significant digits.

use calc_core::{AngleMode, Arity, CalcError, Calculator, Num};

fn calc() -> Calculator {
    Calculator::new()
}

/// Evaluates and returns the formatted result, or the error's message.
fn ev(c: &mut Calculator, input: &str) -> String {
    match c.eval_str(input) {
        Ok(s) => s,
        Err(e) => format!("ERR: {e}"),
    }
}

fn check(cases: &[(&str, &str)]) {
    let mut c = calc();
    for (input, expected) in cases {
        assert_eq!(&ev(&mut c, input), expected, "input: {input}");
    }
}

fn check_in(mode: AngleMode, cases: &[(&str, &str)]) {
    let mut c = calc();
    c.set_angle_mode(mode);
    for (input, expected) in cases {
        assert_eq!(&ev(&mut c, input), expected, "input: {input} ({mode})");
    }
}

fn err(input: &str) -> CalcError {
    calc().eval(input).expect_err(input)
}

// ------------------------------------------------------------------ basics

#[test]
fn arithmetic_and_precedence() {
    check(&[
        ("1+2", "3"),
        ("2+3*4", "14"),
        ("(2+3)*4", "20"),
        ("10-4-3", "3"),
        ("100/5/2", "10"),
        ("2*3+4*5", "26"),
        ("7/2", "3.5"),
        ("-5+3", "-2"),
        ("--5", "5"),
        ("+5", "5"),
        ("2*-3", "-6"),
        ("((((1))))", "1"),
    ]);
}

#[test]
fn power_precedence_and_associativity() {
    check(&[
        ("2^3", "8"),
        ("2^3^2", "512"),   // right associative
        ("-2^2", "-4"),     // unary minus binds looser than ^
        ("(-2)^2", "4"),
        ("2^-2", "0.25"),
        ("2**10", "1024"),  // alias
        ("2^10", "1024"),
        ("10^-3", "0.001"),
        ("2^3!", "64"),     // postfix binds tighter than ^
        ("3²", "9"),
        ("2³", "8"),
        ("(-8)^3", "-512"),
    ]);
}

#[test]
fn modulo_has_the_sign_of_the_dividend() {
    check(&[
        ("7 mod 3", "1"),
        ("-7 mod 3", "-1"),
        ("7 mod -3", "1"),
        ("7.5 mod 2", "1.5"),
        ("7 % 3", "1"),
        ("2+7 mod 4*2", "8"), // mod binds like * : 2 + ((7 mod 4) * 2)
    ]);
}

#[test]
fn implicit_multiplication() {
    check(&[
        ("2pi", "6.283185307179586476925286766559"),
        ("3(4+5)", "27"),
        ("(1+2)(3+4)", "21"),
        ("2 sqrt(16)", "8"),
        ("2e", "5.4365636569180904707205749427053"),
    ]);
    let mut c = calc();
    c.eval("x = 3").unwrap();
    assert_eq!(ev(&mut c, "2x"), "6");
    assert_eq!(ev(&mut c, "x(2+1)"), "9");
    assert_eq!(ev(&mut c, "x x"), "9");
}

#[test]
fn input_is_forgiving() {
    check(&[
        ("  1 +   2  ", "3"),
        ("SQRT(16)", "4"),
        ("Sin(0)", "0"),
        ("π", "3.1415926535897932384626433832795"),
        ("√(81)", "9"),
        ("6×7", "42"),
        ("84÷2", "42"),
        ("5−3", "2"),
        (".5+.5", "1"),
        ("5.+1", "6"),
    ]);
}

// --------------------------------------------------- precision and display

#[test]
fn no_binary_floating_point_artifacts() {
    check(&[
        ("0.1+0.2", "0.3"),
        ("0.1*3", "0.3"),
        ("1-0.9", "0.1"),
        ("4.35*100", "435"),
        ("1/3*3", "1"),
        ("0.3-0.1", "0.2"),
        ("1.1*1.1", "1.21"),
    ]);
}

#[test]
fn shows_32_significant_digits() {
    check(&[
        ("10/3", "3.3333333333333333333333333333333"),
        ("2/3", "0.66666666666666666666666666666667"),
        ("-2/3", "-0.66666666666666666666666666666667"),
        ("1/7", "0.14285714285714285714285714285714"),
        ("sqrt(2)", "1.4142135623730950488016887242097"),
        ("pi", "3.1415926535897932384626433832795"),
        ("e", "2.7182818284590452353602874713527"),
    ]);
}

#[test]
fn plain_versus_scientific_notation() {
    check(&[
        ("0.000001", "0.000001"),
        ("0.0000001", "1e-7"),
        ("3e-7", "3e-7"),
        ("1.5e-10", "1.5e-10"),
        ("1.5e3", "1500"),
        ("1.5E3", "1500"),
        ("2e+3", "2000"),
        ("10^31", "10000000000000000000000000000000"),
        ("10^32", "1e+32"),
        ("10^40", "1e+40"),
        ("2^200", "1.6069380442589902755419620923412e+60"),
        ("1e5000*1e5000", "1e+10000"),
        ("99999999999999999999999999999999.5", "1e+32"), // rounding carries
        ("1234567890123456789012345678901234567890", "1.2345678901234567890123456789012e+39"),
    ]);
}

#[test]
fn display_digits_can_be_changed() {
    let mut c = calc();
    c.set_display_digits(10);
    assert_eq!(ev(&mut c, "10/3"), "3.333333333");
    assert_eq!(ev(&mut c, "pi"), "3.141592654");
    c.set_display_digits(50);
    assert_eq!(
        ev(&mut c, "10/3"),
        "3.3333333333333333333333333333333333333333333333333"
    );
    c.set_display_digits(0);
    assert_eq!(c.display_digits(), 1);
    c.set_display_digits(1000);
    assert_eq!(c.display_digits(), calc_core::MAX_DISPLAY_DIGITS);
}

#[test]
fn results_can_be_fed_back_as_input() {
    let mut c = calc();
    for expr in ["10^40/3", "1/3e9", "pi*1e-20", "-sqrt(2)*1e50"] {
        let shown = c.eval_str(expr).unwrap();
        let shown_again = c.eval_str(&shown).unwrap();
        assert_eq!(shown, shown_again, "round trip of {expr}");
    }
}

// ------------------------------------------------------------ trigonometry

#[test]
fn trig_in_degrees_is_exact_at_the_special_angles() {
    check_in(
        AngleMode::Degrees,
        &[
            ("sin(0)", "0"),
            ("sin(30)", "0.5"),
            ("sin(90)", "1"),
            ("sin(180)", "0"),
            ("sin(270)", "-1"),
            ("sin(360)", "0"),
            ("cos(0)", "1"),
            ("cos(60)", "0.5"),
            ("cos(90)", "0"),
            ("cos(180)", "-1"),
            ("cos(270)", "0"),
            ("tan(0)", "0"),
            ("tan(45)", "1"),
            ("tan(135)", "-1"),
            ("tan(180)", "0"),
            ("sin(-30)", "-0.5"),
            ("sin(390)", "0.5"),
            ("sec(60)", "2"),
            ("csc(30)", "2"),
            ("cot(45)", "1"),
            ("sin(1e20*180)", "0"),
        ],
    );
}

#[test]
fn trig_values_match_references() {
    check_in(
        AngleMode::Degrees,
        &[
            ("sin(37)", "0.60181502315204827991797700044149"),
            ("cos(37)", "0.79863551004729284628400080406894"),
            ("tan(37)", "0.75355405010279415707395644862159"),
            ("sin(1000000)", "-0.98480775301220805936674302458952"),
        ],
    );
    check_in(
        AngleMode::Radians,
        &[
            ("sin(1)", "0.8414709848078965066525023216303"),
            ("cos(1)", "0.54030230586813971740093660744298"),
            ("tan(1)", "1.5574077246549022305069748074584"),
            ("sin(pi/6)", "0.5"),
            ("sin(pi)", "0"),
            ("cos(pi/2)", "0"),
            ("cos(pi)", "-1"),
            ("sin(1e-70)", "1e-70"), // tiny angles must not be flushed to zero
        ],
    );
}

#[test]
fn gradians() {
    check_in(
        AngleMode::Gradians,
        &[
            ("sin(100)", "1"),
            ("cos(200)", "-1"),
            ("sin(0)", "0"),
            ("tan(50)", "1"),
            ("asin(1)", "100"),
            ("atan(1)", "50"),
        ],
    );
}

#[test]
fn inverse_trig_returns_angles_in_the_current_mode() {
    check_in(
        AngleMode::Degrees,
        &[
            ("asin(0.5)", "30"),
            ("acos(0.5)", "60"),
            ("acos(-1)", "180"),
            ("atan(1)", "45"),
            ("asin(1)", "90"),
            ("asin(0.3)", "17.457603123722092290246045792445"),
            ("acot(2)", "26.565051177077989351572193720453"),
            ("asec(2)", "60"),
            ("acsc(2)", "30"),
        ],
    );
    check_in(
        AngleMode::Radians,
        &[
            ("acos(0.3)", "1.2661036727794991112593187304122"),
            ("atan(1)*4", "3.1415926535897932384626433832795"),
        ],
    );
}

#[test]
fn trig_function_roundtrip_in_every_mode() {
    for mode in [AngleMode::Degrees, AngleMode::Radians, AngleMode::Gradians] {
        let mut c = calc();
        c.set_angle_mode(mode);
        assert_eq!(ev(&mut c, "sin(asin(0.25))"), "0.25", "{mode}");
        assert_eq!(ev(&mut c, "cos(acos(0.25))"), "0.25", "{mode}");
        assert_eq!(ev(&mut c, "tan(atan(2))"), "2", "{mode}");
    }
}

#[test]
fn undefined_trig_values_are_errors() {
    let mut c = calc();
    for input in ["tan(90)", "tan(270)", "tan(-90)", "sec(90)", "cot(0)", "cot(180)", "csc(0)"] {
        assert!(
            matches!(c.eval(input), Err(CalcError::Undefined(_))),
            "{input} should be undefined"
        );
    }
    c.set_angle_mode(AngleMode::Radians);
    assert!(matches!(c.eval("tan(pi/2)"), Err(CalcError::Undefined(_))));
    assert!(c.eval("tan(1.5707)").is_ok()); // close to, but not at, pi/2
}

// -------------------------------------------------------------- hyperbolic

#[test]
fn hyperbolic_functions() {
    check(&[
        ("sinh(0)", "0"),
        ("cosh(0)", "1"),
        ("tanh(0)", "0"),
        ("sinh(1)", "1.1752011936438014568823818505956"),
        ("cosh(2)", "3.7621956910836314595622134777737"),
        ("tanh(0.5)", "0.46211715726000975850231848364367"),
        ("asinh(1)", "0.88137358701954302523260932497979"),
        ("acosh(2)", "1.316957896924816708625046347308"),
        ("atanh(0.5)", "0.54930614433405484569762261846126"),
        ("sech(1)", "0.64805427366388539957497735322615"),
        ("asinh(sinh(3))", "3"),
        ("acosh(1)", "0"),
        ("atanh(0)", "0"),
    ]);
}

// ---------------------------------------------------- powers, roots, logs

#[test]
fn powers_and_roots() {
    check(&[
        ("2^10", "1024"),
        ("2^0.5", "1.4142135623730950488016887242097"),
        ("3^1.5", "5.1961524227066318805823390245176"),
        ("4^0.5", "2"),
        ("27^(1/3)", "3"),
        ("root(10,5)", "1.5848931924611134852021013733915"),
        ("root(27,3)", "3"),
        ("root(-27,3)", "-3"),
        ("root(-32,5)", "-2"),
        ("root(16,4)", "2"),
        ("cbrt(2)", "1.2599210498948731647672106072782"),
        ("cbrt(-8)", "-2"),
        ("sqrt(0)", "0"),
        ("sqrt(144)", "12"),
        ("0^5", "0"),
        ("5^0", "1"),
        ("2^200", "1.6069380442589902755419620923412e+60"),
        ("floor(2^10)", "1024"), // integer powers are exact
    ]);
}

/// Regression: the underlying library's `pow` never returns when the exact
/// result is representable (e.g. 4^0.5). These must finish instantly and be exact.
#[test]
fn exact_fractional_powers_do_not_hang_and_stay_exact() {
    check(&[
        ("4^0.5", "2"),
        ("100^0.5", "10"),
        ("81^0.25", "3"),
        ("16^0.75", "8"),
        ("9^1.5", "27"),
        ("4^-0.5", "0.5"),
        ("(1/4)^0.5", "0.5"),
        ("root(16,4)", "2"),
        ("root(256,8)", "2"),
        ("floor(81^0.25)", "3"),
        ("8^(1/3)", "2"),
        ("floor(27^(1/3))", "3"),
        ("floor(1024^(1/10))", "2"),
        ("floor(root(1000,3))", "10"),
        ("2^-0.5", "0.70710678118654752440084436210485"),
        ("10^0.5", "3.1622776601683793319988935444327"),
        ("2^0.001", "1.0006933874625806325375686393039"),
        ("1.0001^10000.5", "2.7182818307240535902527477152808"),
        // Exponents beyond 2^-16, and a base of exactly 1, used to hang as well.
        ("(2^65536)^(1/65536)", "2"),
        ("(2^131072)^(1/131072)", "2"),
        ("(2^1048576)^(2^-20)", "2"),
        ("(2^1000000)^(2^-30)", "1.0006457520249102338825433523413"), // mpmath: 2^(1000000/2^30)
        ("1^0.1", "1"),
        ("1^(1/3)", "1"),
        ("1^-0.1", "1"),
    ]);
}

#[test]
fn exponentials_and_logarithms() {
    check(&[
        ("exp(0)", "1"),
        ("exp(1)", "2.7182818284590452353602874713527"),
        ("exp(1.5)", "4.4816890703380648226020554601193"),
        ("exp(100)", "2.68811714181613544841262555158e+43"),
        ("exp(-100)", "3.7200759760208359629596958038631e-44"),
        ("ln(1)", "0"),
        ("ln(e)", "1"),
        ("ln(2)", "0.69314718055994530941723212145818"),
        ("log(100)", "2"),
        ("log10(0.001)", "-3"),
        ("log10(2)", "0.30102999566398119521373889472449"),
        ("log2(1024)", "10"),
        ("log2(0.25)", "-2"),
        ("logb(5,3)", "1.4649735207179271671970404076786"),
        ("logb(81,3)", "4"),
        ("floor(log10(1000))", "3"),
        ("floor(log2(8))", "3"),
        ("floor(logb(125,5))", "3"),
        ("floor(log10(1e100))", "100"),
        ("ln(exp(5))", "5"),
    ]);
}

// ------------------------------------------- factorial and combinatorics

#[test]
fn factorials() {
    check(&[
        ("0!", "1"),
        ("1!", "1"),
        ("5!", "120"),
        ("10!", "3628800"),
        ("20!", "2432902008176640000"),
        ("25!", "15511210043330985984000000"),
        ("30!", "2.6525285981219105863630848e+32"),
        ("50!", "3.0414093201713378043612608166065e+64"),
        ("70!", "1.1978571669969891796072783721689e+100"),
        ("fact(6)", "720"),
        ("(2+3)!", "120"),
        ("3!!", "720"),
        ("4!/3!", "4"),
        ("-3!", "-6"),
        ("fact(6/2)", "6"),
    ]);
}

#[test]
fn combinations_and_permutations() {
    check(&[
        ("ncr(5,2)", "10"),
        ("ncr(52,5)", "2598960"),
        ("ncr(100,50)", "100891344545564193334812497256"),
        ("ncr(10,0)", "1"),
        ("ncr(10,10)", "1"),
        ("npr(5,2)", "20"),
        ("npr(10,4)", "5040"),
        ("npr(7,0)", "1"),
    ]);
}

// -------------------------------------------- rounding and miscellaneous

#[test]
fn rounding_functions() {
    check(&[
        ("abs(-3.5)", "3.5"),
        ("abs(3.5)", "3.5"),
        ("floor(-1.5)", "-2"),
        ("floor(1.5)", "1"),
        ("ceil(-1.5)", "-1"),
        ("ceil(1.2)", "2"),
        ("trunc(-1.7)", "-1"),
        ("trunc(1.7)", "1"),
        ("frac(1.25)", "0.25"),
        ("frac(-1.25)", "-0.25"),
        ("round(2.5)", "3"),
        ("round(-2.5)", "-3"),
        ("round(2.4)", "2"),
        ("round(-2.4)", "-2"),
        ("round(0.5)", "1"),
        ("sign(-9)", "-1"),
        ("sign(0)", "0"),
        ("sign(9)", "1"),
    ]);
}

#[test]
fn variadic_functions() {
    check(&[
        ("max(1,5,3)", "5"),
        ("min(4,-2,9)", "-2"),
        ("sum(1,2,3,4)", "10"),
        ("mean(1,2,3,4)", "2.5"),
        ("sum(7)", "7"),
        ("max(2)", "2"),
        ("mean(0.1,0.2,0.3)", "0.2"),
    ]);
}

#[test]
fn degrees_minutes_seconds() {
    check(&[
        ("dms(10.5)", "10.3"),
        ("deg(10.3)", "10.5"),
        ("dms(0.25)", "0.15"),
        ("deg(0.15)", "0.25"),
        ("deg(dms(37.123))", "37.123"),
        ("dms(-10.5)", "-10.3"),
    ]);
}

#[test]
fn random_numbers_are_in_range() {
    let mut c = calc();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        let v = c.eval("rand()").unwrap();
        let f = c.to_f64(&v).unwrap();
        assert!((0.0..1.0).contains(&f), "{f}");
        seen.insert(c.format(&v));
    }
    assert!(seen.len() > 190, "rand() should not repeat itself");
}

// ------------------------------------------------------------------ errors

#[test]
fn arithmetic_errors() {
    assert_eq!(err("1/0"), CalcError::DivisionByZero);
    assert_eq!(err("5 mod 0"), CalcError::DivisionByZero);
    assert_eq!(err("0^-1"), CalcError::DivisionByZero);
    assert!(matches!(err("0/0"), CalcError::Undefined(_)));
    assert!(matches!(err("0^0"), CalcError::Undefined(_)));
    assert!(matches!(err("sqrt(-1)"), CalcError::Domain(_)));
    assert!(matches!(err("ln(0)"), CalcError::Domain(_)));
    assert!(matches!(err("ln(-1)"), CalcError::Domain(_)));
    assert!(matches!(err("log(0)"), CalcError::Domain(_)));
    assert!(matches!(err("log2(-4)"), CalcError::Domain(_)));
    assert!(matches!(err("logb(8,1)"), CalcError::Domain(_)));
    assert!(matches!(err("(-8)^(1/3)"), CalcError::Domain(_)));
    assert!(matches!(err("root(-4,2)"), CalcError::Domain(_)));
    assert!(matches!(err("root(-16,4)"), CalcError::Domain(_)));
    assert!(matches!(err("root(5,0)"), CalcError::Domain(_)));
    assert!(matches!(err("asin(2)"), CalcError::Domain(_)));
    assert!(matches!(err("acos(-1.5)"), CalcError::Domain(_)));
    assert!(matches!(err("acosh(0.5)"), CalcError::Domain(_)));
    assert!(matches!(err("atanh(1)"), CalcError::Domain(_)));
    assert!(matches!(err("asec(0.5)"), CalcError::Domain(_)));
    assert!(matches!(err("(-1)!"), CalcError::Domain(_)));
    assert!(matches!(err("2.5!"), CalcError::Domain(_)));
    assert!(matches!(err("10001!"), CalcError::Domain(_)));
    assert!(matches!(err("ncr(3,5)"), CalcError::Domain(_)));
    assert!(matches!(err("ncr(-1,0)"), CalcError::Domain(_)));
    assert!(matches!(err("ncr(2.5,1)"), CalcError::Domain(_)));
}

#[test]
fn overflow_is_reported_not_silently_infinite() {
    assert_eq!(err("exp(1e20)"), CalcError::Overflow);
    assert_eq!(err("10^(10^10)"), CalcError::Overflow);
    assert_eq!(err("1e999999999999"), CalcError::Overflow);
}

#[test]
fn syntax_errors() {
    for input in [
        "", "   ", "2+", "*3", "(2", "2)", "2 3", "2..3", "1.2.3", "2 $ 3", "sin(", "sin(1,",
        "sin 30", "sqrt", "mod 3", "2 mod", "()", "(,)", "1,2", "= 5", "5 = 5", "x = ", "3 = x",
        "2 + * 3", "max()", "f(",
    ] {
        assert!(
            c_eval_is_err(input),
            "{input:?} should be rejected but evaluated"
        );
    }
    assert!(matches!(err("2+"), CalcError::Syntax(_)));
    assert!(matches!(err("(2"), CalcError::Syntax(_)));
    assert!(matches!(err("sin 30"), CalcError::Syntax(_)));
}

fn c_eval_is_err(input: &str) -> bool {
    calc().eval(input).is_err()
}

#[test]
fn error_messages_point_at_the_problem() {
    let msg = err("2 + * 3").to_string();
    assert!(msg.contains("'*'") && msg.contains("position 5"), "{msg}");
    let msg = err("2 $ 3").to_string();
    assert!(msg.contains("'$'") && msg.contains("position 3"), "{msg}");
    let msg = err("sin 30").to_string();
    assert!(msg.contains("sin(x)"), "{msg}");
    assert_eq!(err("1/0").to_string(), "Cannot divide by zero");
}

#[test]
fn unknown_names_and_arity() {
    assert_eq!(err("foo"), CalcError::UnknownIdentifier("foo".into()));
    assert_eq!(err("2+bar*3"), CalcError::UnknownIdentifier("bar".into()));
    assert!(matches!(err("sin()"), CalcError::Arity { got: 0, .. }));
    assert!(matches!(err("sin(1,2)"), CalcError::Arity { got: 2, .. }));
    assert!(matches!(err("root(4)"), CalcError::Arity { got: 1, .. }));
    assert!(matches!(err("rand(1)"), CalcError::Arity { got: 1, .. }));
    assert!(matches!(err("max()"), CalcError::Arity { got: 0, .. }));
}

#[test]
fn deeply_nested_input_does_not_overflow_the_stack() {
    let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
    assert!(matches!(err(&deep), CalcError::Syntax(_)));
    let ok = format!("{}1{}", "(".repeat(100), ")".repeat(100));
    assert_eq!(ev(&mut calc(), &ok), "1");
    let unary = format!("{}1", "-".repeat(5000));
    assert!(calc().eval(&unary).is_err());
}

// ------------------------------------------- variables, memory, history

#[test]
fn variables_and_ans() {
    let mut c = calc();
    assert_eq!(ev(&mut c, "x = 5"), "5");
    assert_eq!(ev(&mut c, "x * 2"), "10");
    assert_eq!(ev(&mut c, "y = x^2 + 1"), "26");
    assert_eq!(ev(&mut c, "ans + x"), "31");
    assert_eq!(ev(&mut c, "x = x + 1"), "6");
    assert_eq!(ev(&mut c, "radius_2 = 2"), "2");
    assert_eq!(ev(&mut c, "pi * radius_2^2"), "12.566370614359172953850573533118");
    let vars: Vec<_> = c.variables().iter().map(|(n, _)| n.to_string()).collect();
    assert_eq!(vars, ["radius_2", "x", "y"]);
    c.clear_variables();
    assert_eq!(ev(&mut c, "x"), "ERR: Unknown name 'x'");
}

#[test]
fn variable_failures_leave_state_untouched() {
    let mut c = calc();
    c.eval("x = 5").unwrap();
    assert!(c.eval("x = 1/0").is_err());
    assert_eq!(ev(&mut c, "x"), "5");
    assert_eq!(ev(&mut c, "7"), "7");
    assert!(c.eval("x = 1/0").is_err());
    assert_eq!(ev(&mut c, "ans"), "7", "a failed statement must not change ans");
}

#[test]
fn reserved_names_cannot_be_assigned() {
    let mut c = calc();
    for name in ["pi", "e", "ans", "mem", "sin", "sqrt", "mod"] {
        let r = c.eval(&format!("{name} = 3"));
        assert!(matches!(r, Err(CalcError::Assignment(_))), "{name}: {r:?}");
    }
    assert_eq!(ev(&mut c, "e"), "2.7182818284590452353602874713527");
}

#[test]
fn memory_functions() {
    let mut c = calc();
    assert!(c.memory().is_none());
    assert_eq!(ev(&mut c, "mem"), "0");
    let twelve = c.eval("12").unwrap();
    c.memory_store(twelve);
    assert_eq!(ev(&mut c, "mem"), "12");
    c.memory_add(&Num::from_i64(8)).unwrap();
    assert_eq!(ev(&mut c, "mem * 2"), "40");
    c.memory_subtract(&Num::from_i64(5)).unwrap();
    assert_eq!(ev(&mut c, "mem"), "15");
    c.memory_clear();
    assert!(c.memory().is_none());
    assert_eq!(ev(&mut c, "mem"), "0");
    // M+ on an empty memory starts from zero.
    c.memory_add(&Num::from_i64(3)).unwrap();
    assert_eq!(ev(&mut c, "mem"), "3");
}

#[test]
fn history_records_successful_inputs_only() {
    let mut c = calc();
    c.eval("1+1").unwrap();
    assert!(c.eval("1/0").is_err());
    c.eval("x = 2*3").unwrap();
    let h = c.history();
    assert_eq!(h.len(), 2);
    assert_eq!((h[0].input.as_str(), h[0].output.as_str()), ("1+1", "2"));
    assert_eq!((h[1].input.as_str(), h[1].output.as_str()), ("x = 2*3", "6"));
    c.clear_history();
    assert!(c.history().is_empty());
}

#[test]
fn angle_mode_parsing_and_state() {
    assert_eq!("deg".parse::<AngleMode>().unwrap(), AngleMode::Degrees);
    assert_eq!("RAD".parse::<AngleMode>().unwrap(), AngleMode::Radians);
    assert_eq!("Grad".parse::<AngleMode>().unwrap(), AngleMode::Gradians);
    assert!("turns".parse::<AngleMode>().is_err());
    let mut c = calc();
    assert_eq!(c.angle_mode(), AngleMode::Degrees, "default mode");
    c.set_angle_mode(AngleMode::Radians);
    assert_eq!(ev(&mut c, "sin(pi/2)"), "1");
}

// --------------------------------------------------------------- extension

#[test]
fn custom_functions_can_be_registered() {
    let mut c = calc();
    c.registry_mut()
        .register("double", Arity::Exact(1), "twice the argument", |ctx, a| {
            ctx.mul(&a[0], &Num::from_i64(2))
        });
    c.registry_mut()
        .register("hyp", Arity::Exact(2), "hypotenuse", |ctx, a| {
            let x = ctx.mul(&a[0], &a[0])?;
            let y = ctx.mul(&a[1], &a[1])?;
            let s = ctx.add(&x, &y)?;
            ctx.sqrt(&s)
        });
    assert_eq!(ev(&mut c, "double(21)"), "42");
    assert_eq!(ev(&mut c, "hyp(3,4)"), "5");
    assert_eq!(ev(&mut c, "double(hyp(6,8))"), "20");
    assert!(c.registry().contains("double"));
    assert!(c.registry().list().iter().any(|f| f.name == "hyp"));
    assert!(matches!(c.eval("double()"), Err(CalcError::Arity { .. })));
}

#[test]
fn registry_lists_every_function_sorted() {
    let c = calc();
    let names: Vec<_> = c.registry().list().iter().map(|f| f.name.clone()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    for expected in [
        "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh",
        "atanh", "sec", "csc", "cot", "asec", "acsc", "acot", "sech", "csch", "coth", "asech",
        "acsch", "acoth", "ln", "log", "log2", "logb", "exp", "sqrt", "cbrt", "root", "abs",
        "floor", "ceil", "fact", "dms", "deg", "rand",
    ] {
        assert!(names.iter().any(|n| n == expected), "missing {expected}");
    }
}

#[test]
fn conversions_to_primitive_types() {
    let mut c = calc();
    let v = c.eval("1/4").unwrap();
    assert_eq!(c.to_f64(&v), Some(0.25));
    let v = c.eval("-1e300").unwrap();
    assert_eq!(c.to_f64(&v), Some(-1e300));
    let v = c.eval("0").unwrap();
    assert_eq!(c.to_f64(&v), Some(0.0));
}

// ------------------------------------------------ decimal arithmetic is exact
//
// Inputs like 0.294 are binary approximations internally. Results that are
// integers (or half-integers, or zero) on paper must behave as such in floor,
// ceil, mod, round, factorial, ... even though the binary product can land one
// ulp away. These tests failed ~7% of the time before values within rounding
// noise of an integer were treated as that integer.

/// Small deterministic generator so the tests need no extra dependencies.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

/// `m / 10^d` written as an exact decimal string, e.g. (4428, 6) -> "0.004428".
fn decimal(m: u64, d: u32) -> String {
    let scale = 10u64.pow(d);
    format!("{}.{:0width$}", m / scale, m % scale, width = d as usize)
}

fn expect_all(cases: Vec<(String, String)>) {
    let mut c = calc();
    let mut bad = Vec::new();
    for (input, expected) in &cases {
        let got = ev(&mut c, input);
        if &got != expected {
            bad.push(format!("{input}: expected {expected}, got {got}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} cases wrong, e.g.:\n{}",
        bad.len(),
        cases.len(),
        bad.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn decimal_products_that_are_integers_are_exact() {
    let mut rng = Lcg(12345);
    let mut cases = Vec::new();
    for _ in 0..1500 {
        let d = rng.range(1, 9) as u32;
        let digits = rng.range(1, 9) as u32;
        let m = rng.range(1, 10u64.pow(digits));
        let t = rng.range(1, 1000);
        let a = decimal(m, d);
        let b = 10u64.pow(d) * t;
        let want = (m * t).to_string();
        cases.push((format!("floor({a}*{b})"), want.clone()));
        cases.push((format!("ceil({a}*{b})"), want.clone()));
        cases.push((format!("trunc({a}*{b})"), want.clone()));
        cases.push((format!("round({a}*{b})"), want.clone()));
        cases.push((format!("({a}*{b}) mod 1"), "0".into()));
        cases.push((format!("frac({a}*{b})"), "0".into()));
        cases.push((format!("{a}*{b} - {want}"), "0".into()));
    }
    expect_all(cases);
}

#[test]
fn decimal_quotients_and_sums_that_are_integers_are_exact() {
    let mut rng = Lcg(777);
    let mut cases = Vec::new();
    for _ in 0..1000 {
        let d = rng.range(1, 6) as u32;
        let digits = rng.range(1, 7) as u32;
        let m = rng.range(1, 10u64.pow(digits));
        let p = 10u64.pow(d);
        // m / 10^d * 10^d == m
        cases.push((format!("floor({m}/{p}*{p})"), m.to_string()));
        cases.push((format!("ceil({m}/{p}*{p})"), m.to_string()));
        // three decimals that add up to an integer S
        let s = rng.range(1, 500);
        let total = s * p;
        let m1 = rng.range(0, total);
        let m2 = rng.range(0, total - m1);
        let m3 = total - m1 - m2;
        let sum = format!("{}+{}+{}", decimal(m1, d), decimal(m2, d), decimal(m3, d));
        cases.push((format!("floor({sum})"), s.to_string()));
        cases.push((format!("ceil({sum})"), s.to_string()));
        cases.push((format!("({sum}) mod 1"), "0".into()));
        cases.push((format!("{sum} - {s}"), "0".into()));
    }
    expect_all(cases);
}

#[test]
fn halves_round_away_from_zero_like_on_paper() {
    let mut rng = Lcg(99);
    let mut cases = Vec::new();
    for _ in 0..800 {
        let d = rng.range(1, 8) as u32;
        let n = rng.range(0, 100_000);
        // (10n+5)/10^d * 10^(d-1) == n + 0.5
        let a = decimal(10 * n + 5, d);
        let b = 10u64.pow(d - 1);
        cases.push((format!("round({a}*{b})"), (n + 1).to_string()));
        cases.push((format!("round(-{a}*{b})"), format!("-{}", n + 1)));
        cases.push((format!("floor({a}*{b})"), n.to_string()));
        cases.push((format!("ceil({a}*{b})"), (n + 1).to_string()));
    }
    expect_all(cases);
    check(&[
        ("round(0.285*100)", "29"),
        ("round(1.005*100)", "101"),
        ("round(2.675*100)", "268"),
    ]);
}

#[test]
fn integer_dependent_functions_accept_decimal_arithmetic() {
    let mut rng = Lcg(2024);
    let mut cases = Vec::new();
    let facts: [u64; 13] = [1, 1, 2, 6, 24, 120, 720, 5040, 40320, 362880, 3628800, 39916800, 479001600];
    for _ in 0..400 {
        let d = rng.range(1, 6) as u32;
        let k = rng.range(0, 12);
        let p = 10u64.pow(d);
        // fact(k / 10^d * 10^d) with k written as a decimal fraction
        cases.push((format!("fact({k}/{p}*{p})"), facts[k as usize].to_string()));
        cases.push((format!("({k}/{p}*{p})!"), facts[k as usize].to_string()));
        // ncr / npr with decimal-built arguments
        let n = rng.range(k, 20);
        let ncr = (0..k).fold(1u128, |acc, i| acc * (n - i) as u128 / (i + 1) as u128);
        let npr = (0..k).fold(1u128, |acc, i| acc * (n - i) as u128);
        cases.push((format!("ncr({n}/{p}*{p}, {k}/{p}*{p})"), ncr.to_string()));
        cases.push((format!("npr({n}/{p}*{p}, {k}/{p}*{p})"), npr.to_string()));
        // perfect squares built from decimals
        let m = rng.range(1, 3000);
        let a = decimal(m, d);
        let sq = 10u64.pow(2 * d);
        cases.push((format!("floor(sqrt({a}*{a}*{sq}))"), m.to_string()));
        cases.push((format!("frac(sqrt({a}*{a}*{sq}))"), "0".into()));
        // integer powers
        let base = rng.range(2, 40);
        cases.push((
            format!("floor(({base}/{p}*{p})^3)"),
            (base * base * base).to_string(),
        ));
    }
    expect_all(cases);
}

#[test]
fn modulo_of_decimals() {
    check(&[
        ("0.3 mod 0.1", "0"),
        ("7.5 mod 2.5", "0"),
        ("1.1 mod 0.1", "0"),
        ("10 mod 0.1", "0"),
        ("0.7 mod 0.2", "0.1"),
        ("7.5 mod 2", "1.5"),
        ("-0.7 mod 0.2", "-0.1"),
        ("1e100 mod 7", "4"), // 10^100 mod 7; exact because 10^100 fits in 256 bits
        ("1e30 mod 7", "1"),  // 10^30 mod 7
    ]);
}

#[test]
fn rounding_noise_is_flushed_but_real_small_values_survive() {
    check(&[
        ("0.1+0.2-0.3", "0"),
        ("0.3-0.1*3", "0"),
        ("1/3+1/3+1/3-1", "0"),
        ("sqrt(2)^2-2", "0"),
        // genuinely tiny values are not noise:
        ("1e-70+1e-70", "2e-70"),
        // 1 + 1e-60 only keeps ~17 digits of the 1e-60 (256-bit mantissa), but it
        // must not be flushed to zero:
        ("round(((1+1e-60)-1)*1e60)", "1"),
        ("round(((1+1e-70)-1)*1e70)", "1"),
        ("sin(1e-70)", "1.7453292519943295769236907684886e-72"), // 1e-70 degrees, in radians
        ("frac(1e-70)", "1e-70"),
        ("floor(1e-70)", "0"),
        ("ceil(1e-70)", "1"),
        ("floor(-1e-60)", "-1"),
        ("floor(3-1e-60)", "2"),
        ("ceil(3+1e-60)", "4"),
        ("floor(1e-3*1e-3*1e6)", "1"),
    ]);
}

// -------------------------------------------------------------- huge angles

/// Exactly representable angles are reduced exactly, whatever their size;
/// angles that had to be rounded to fit are rejected rather than answered with
/// the sine of a different number. References: mpmath, 300 digits.
#[test]
fn huge_angles_are_exact_when_representable_and_rejected_when_not() {
    // 10^30 and 10^100 are both 280 mod 360.
    check_in(
        AngleMode::Degrees,
        &[
            ("cos(1e30)", "0.17364817766693034885171662676931"),
            ("tan(1e30)", "-5.671281819617709530994418439864"),
            ("sin(1e100)", "-0.98480775301220805936674302458952"),
            ("sin(1e30) - sin(280)", "0"),
        ],
    );
    check_in(
        AngleMode::Radians,
        &[
            ("sin(1e30)", "-0.090116901912138058030386428952987"),
            ("cos(1e30)", "-0.99593119440539570239424858799705"),
            ("sin(1e100)", "-0.37237612366127668826208669555316"),
        ],
    );
    for mode in [AngleMode::Degrees, AngleMode::Radians, AngleMode::Gradians] {
        let mut c = calc();
        c.set_angle_mode(mode);
        for input in [
            "sin(1e200)",
            "cos(1e1000000)",
            "tan(1e111)",
            "sec(1e200)",
            "sin(0.1*1e40)", // 0.1 is inexact, and the product is far beyond 2^128
        ] {
            assert!(
                matches!(c.eval(input), Err(CalcError::Domain(_))),
                "{input} in {mode} should be rejected, got {:?}",
                c.eval(input)
            );
        }
        // Inexact but small enough that plenty of digits survive:
        assert!(c.eval("sin(0.1*1e30)").is_ok(), "{mode}");
        assert!(c.eval("sin(1e110)").is_ok(), "1e110 is exactly representable ({mode})");
    }
}

#[test]
fn out_of_range_literals_and_underflow_are_reported() {
    for s in ["1e-650000000", "1e-700000000", "1e-99999999999999999999"] {
        assert_eq!(err(s), CalcError::Underflow, "{s}");
    }
    for s in ["1e650000000", "1e99999999999999999999", "0.1^-700000000"] {
        assert_eq!(err(s), CalcError::Overflow, "{s}");
    }
    for s in [
        "1e-640000000*1e-640000000",
        "(1e-640000000)^2",
        "exp(-1e10)",
        "0.1^700000000",
        "10^-700000000",
        "1/1e640000000/1e640000000",
    ] {
        assert_eq!(err(s), CalcError::Underflow, "{s}");
    }
    check(&[
        ("0e-99999999999999999999", "0"),
        ("0.0e5", "0"),
        ("0*1e-640000000", "0"),
        ("1e-640000000", "1e-640000000"),
        ("1e640000000", "1e+640000000"),
        ("0^5", "0"),
    ]);
}

#[test]
fn variables_can_be_set_and_removed_directly() {
    let mut c = calc();
    let v = c.set_variable("R", "2*2.5").unwrap();
    assert_eq!(c.format(&v), "5");
    assert_eq!(ev(&mut c, "r^2"), "25");
    // uses other variables, and replaces an existing value
    c.set_variable("k", "r+1").unwrap();
    assert_eq!(ev(&mut c, "k"), "6");
    c.set_variable("k", "7").unwrap();
    assert_eq!(ev(&mut c, "k"), "7");
    // no history entry and no change to ans
    let history = c.history().len();
    c.set_variable("z", "99").unwrap();
    assert_eq!(c.history().len(), history);
    assert_ne!(ev(&mut c, "ans"), "99");

    for bad in ["pi", "sin", "mod", "2x", "x y", "", "a+b"] {
        assert!(
            matches!(c.set_variable(bad, "1"), Err(CalcError::Assignment(_)) | Err(CalcError::Syntax(_))),
            "{bad}"
        );
    }
    // a failing value changes nothing
    assert!(c.set_variable("k", "1/0").is_err());
    assert_eq!(ev(&mut c, "k"), "7");
    assert!(c.set_variable("q", "unknown_name").is_err());

    assert!(c.remove_variable("K"));
    assert!(!c.remove_variable("k"));
    assert!(c.eval("k").is_err());
}

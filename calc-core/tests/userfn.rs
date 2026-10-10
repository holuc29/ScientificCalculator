use calc_core::{CalcError, Calculator};

fn calc() -> Calculator {
    Calculator::new()
}

#[test]
fn define_and_call() {
    let mut c = calc();
    assert_eq!(c.define_function("f(x) = x^2 + 1").unwrap().unwrap(), "f(x) = x^2 + 1");
    assert_eq!(c.eval_str("f(3)").unwrap(), "10");
    assert_eq!(c.eval_str("f(f(2))").unwrap(), "26");
    assert_eq!(c.eval_str("2f(1)").unwrap(), "4");
}

#[test]
fn not_a_definition_returns_none() {
    let mut c = calc();
    assert!(c.define_function("2+2").unwrap().is_none());
    assert!(c.define_function("x = 3").unwrap().is_none());
    assert!(c.define_function("sin(30)").unwrap().is_none());
}

#[test]
fn multiple_parameters_and_nesting() {
    let mut c = calc();
    c.define_function("g(a, b) = a*b - b").unwrap();
    c.define_function("h(x) = g(x, x + 1) + 1").unwrap();
    assert_eq!(c.eval_str("g(3, 4)").unwrap(), "8");
    assert_eq!(c.eval_str("h(2)").unwrap(), "4"); // 2*3-3+1
    // arguments must not capture parameters: g(b, a) with swapped names
    assert_eq!(c.eval_str("g(2, 10)").unwrap(), "10");
}

#[test]
fn substitution_is_simultaneous() {
    let mut c = calc();
    c.define_function("s(x, y) = x - y").unwrap();
    c.eval_str("y = 100").unwrap();
    // x -> y (global), y -> x: must give y - x = 100 - 5
    c.eval_str("x = 5").unwrap();
    assert_eq!(c.eval_str("s(y, x)").unwrap(), "95");
}

#[test]
fn globals_visible_parameters_shadow() {
    let mut c = calc();
    c.eval_str("k = 10").unwrap();
    c.define_function("f(x) = k*x").unwrap();
    assert_eq!(c.eval_str("f(2)").unwrap(), "20");
    c.eval_str("k = 3").unwrap();
    assert_eq!(c.eval_str("f(2)").unwrap(), "6");
    c.eval_str("x = 99").unwrap();
    assert_eq!(c.eval_str("f(2)").unwrap(), "6");
}

#[test]
fn redefinition_updates_dependents() {
    let mut c = calc();
    c.define_function("a(x) = x + 1").unwrap();
    c.define_function("b(x) = a(x) * 2").unwrap();
    assert_eq!(c.eval_str("b(1)").unwrap(), "4");
    c.define_function("a(x) = x + 10").unwrap();
    assert_eq!(c.eval_str("b(1)").unwrap(), "22");
}

#[test]
fn definition_errors() {
    let mut c = calc();
    for bad in ["sin(x) = x", "pi(x) = x", "f(x, x) = x", "f(sin) = 1", "f(x) =", "f(e) = e"] {
        assert!(c.define_function(bad).is_err(), "{bad} should fail");
    }
    assert!(matches!(
        c.define_function("f(x) = f(x) + 1"),
        Err(CalcError::Assignment(_))
    ));
    c.define_function("p(x) = x").unwrap();
    c.define_function("q(x) = p(x)").unwrap();
    assert!(matches!(
        c.define_function("p(x) = q(x)"),
        Err(CalcError::Assignment(_))
    ));
    // the failed redefinition changed nothing
    assert_eq!(c.eval_str("q(7)").unwrap(), "7");
    c.eval_str("v = 1").unwrap();
    assert!(c.define_function("v(x) = x").is_err());
}

#[test]
fn call_errors() {
    let mut c = calc();
    c.define_function("f(x) = x").unwrap();
    assert!(matches!(c.eval_str("f(1, 2)"), Err(CalcError::Arity { .. })));
    assert!(matches!(c.eval_str("f()"), Err(CalcError::Arity { .. })));
    assert!(c.eval_str("f").is_err());
    assert!(c.eval_str("f(x) = 3").is_err());
    assert!(c.eval_str("f = 3").is_err());
    assert!(c.set_variable("f", "3").is_err());
}

#[test]
fn derivative_through_functions() {
    let mut c = calc();
    c.define_function("f(x) = x^3").unwrap();
    c.define_function("g(x) = f(x) + 2x").unwrap();
    let d = c.differentiate("g(x)", "x", 1).unwrap();
    assert_eq!(d.expr, "3*x^2 + 2");
}

#[test]
fn no_blowup() {
    let mut c = calc();
    c.define_function("d(x) = x + x + x + x").unwrap();
    let mut e = String::from("x");
    for _ in 0..30 {
        e = format!("d({e})");
    }
    c.eval_str("x = 1").unwrap();
    assert!(c.eval_str(&e).is_err()); // too large, but no hang or crash
}

#[test]
fn list_and_remove() {
    let mut c = calc();
    c.define_function("b(x) = x").unwrap();
    c.define_function("a(x, y) = x + y").unwrap();
    let names: Vec<_> = c.functions().into_iter().map(|f| f.0).collect();
    assert_eq!(names, ["a", "b"]);
    assert!(c.remove_function("A"));
    assert!(!c.remove_function("a"));
    assert!(c.eval_str("a(1,2)").is_err());
}

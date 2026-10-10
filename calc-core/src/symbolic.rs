//! Symbolic differentiation of expression trees.
//!
//! [`differentiate`] applies the usual rules (sum, product, quotient, chain,
//! power, a table for every differentiable built-in) and then [`simplify`]
//! cleans the result up: integer and rational constants are folded exactly,
//! like terms and equal factors are combined, `x*1`, `x+0`, `x^1` disappear,
//! and products are written as `numerator/denominator`.
//!
//! Limits worth knowing:
//! * Trigonometric functions follow the active angle mode, so in degrees
//!   `d/dx sin(x)` is `pi*cos(x)/180`.
//! * Simplification is local and syntactic. The result is correct but not
//!   always the shortest form a person would write.
//! * Cancelling `x/x` to `1` ignores the point `x = 0`, as is usual.
//! * Other names (user variables, `pi`, `e`, `ans`, `mem`) are constants.

use crate::error::CalcError;
use crate::num::AngleMode;
use crate::parser::{Expr, Op};

/// Upper bound on the size of an expression before differentiation gives up.
const MAX_NODES: usize = 20_000;

/// The result of a symbolic differentiation.
#[derive(Debug, Clone, PartialEq)]
pub struct Derivative {
    /// The derivative, in a form the calculator can read back.
    pub expr: String,
    /// The same derivative as LaTeX math, for typeset display.
    pub latex: String,
    /// Caveats that apply to the result (angle mode, jump points, ...).
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------- building

fn b(e: Expr) -> Box<Expr> {
    Box::new(e)
}

fn int(n: i128) -> Expr {
    if n < 0 {
        Expr::Neg(b(Expr::Num((-n).to_string())))
    } else {
        Expr::Num(n.to_string())
    }
}

fn var(name: &str) -> Expr {
    Expr::Var(name.to_string())
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(name.to_string(), args)
}

fn bin(op: Op, l: Expr, r: Expr) -> Expr {
    Expr::Bin(op, b(l), b(r))
}

fn add(l: Expr, r: Expr) -> Expr {
    bin(Op::Add, l, r)
}
fn sub(l: Expr, r: Expr) -> Expr {
    bin(Op::Sub, l, r)
}
fn mul(l: Expr, r: Expr) -> Expr {
    bin(Op::Mul, l, r)
}
fn div(l: Expr, r: Expr) -> Expr {
    bin(Op::Div, l, r)
}
fn pow(l: Expr, r: Expr) -> Expr {
    bin(Op::Pow, l, r)
}
fn neg(e: Expr) -> Expr {
    Expr::Neg(b(e))
}

// --------------------------------------------------------------- rationals

/// An exact fraction `n/d` with `d > 0`, in lowest terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Q {
    n: i128,
    d: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    const ONE: Q = Q { n: 1, d: 1 };

    fn new(n: i128, d: i128) -> Option<Q> {
        if d == 0 {
            return None;
        }
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Some(Q {
            n: s * (n / g),
            d: s * (d / g),
        })
    }

    fn is_zero(self) -> bool {
        self.n == 0
    }

    fn neg(self) -> Q {
        Q {
            n: -self.n,
            d: self.d,
        }
    }

    fn mul(self, o: Q) -> Option<Q> {
        Q::new(self.n.checked_mul(o.n)?, self.d.checked_mul(o.d)?)
    }

    fn add(self, o: Q) -> Option<Q> {
        let n = self
            .n
            .checked_mul(o.d)?
            .checked_add(o.n.checked_mul(self.d)?)?;
        Q::new(n, self.d.checked_mul(o.d)?)
    }

    fn recip(self) -> Option<Q> {
        Q::new(self.d, self.n)
    }

    /// `self^k` for an integer `k` (`None` on overflow or `0^negative`).
    fn powi(self, k: i64) -> Option<Q> {
        if k.unsigned_abs() > 64 {
            return None;
        }
        let base = if k < 0 { self.recip()? } else { self };
        let mut out = Q::ONE;
        for _ in 0..k.unsigned_abs() {
            out = out.mul(base)?;
        }
        Some(out)
    }
}

/// Reads a plain decimal literal (`12`, `0.25`) as an exact fraction.
fn literal(s: &str) -> Option<Q> {
    let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
    let digits = format!("{int_part}{frac_part}");
    if digits.is_empty()
        || digits.len() > 30
        || !digits.bytes().all(|c| c.is_ascii_digit())
        || frac_part.len() > 30
    {
        return None;
    }
    let n: i128 = digits.parse().ok()?;
    let d = 10i128.checked_pow(frac_part.len() as u32)?;
    Q::new(n, d)
}

/// An integer-valued exponent such as `3` or `-2`.
fn as_int(e: &Expr) -> Option<i64> {
    match e {
        Expr::Num(s) => {
            let q = literal(s)?;
            if q.d == 1 {
                i64::try_from(q.n).ok()
            } else {
                None
            }
        }
        Expr::Neg(inner) => as_int(inner).and_then(i64::checked_neg),
        _ => None,
    }
}

// --------------------------------------------------------------- measuring

fn node_count(e: &Expr) -> usize {
    match e {
        Expr::Num(_) | Expr::Var(_) => 1,
        Expr::Neg(a) | Expr::Fact(a) | Expr::Assign(_, a) => 1 + node_count(a),
        Expr::Bin(_, l, r) => 1 + node_count(l) + node_count(r),
        Expr::Call(_, args) => 1 + args.iter().map(node_count).sum::<usize>(),
    }
}

fn depends_on(e: &Expr, v: &str) -> bool {
    match e {
        Expr::Num(_) => false,
        Expr::Var(n) => n == v,
        Expr::Neg(a) | Expr::Fact(a) | Expr::Assign(_, a) => depends_on(a, v),
        Expr::Bin(_, l, r) => depends_on(l, v) || depends_on(r, v),
        Expr::Call(_, args) => args.iter().any(|a| depends_on(a, v)),
    }
}

// ------------------------------------------------------------ simplification

/// A product `coef * base1^e1 * base2^e2 * ...`; negative exponents are
/// factors of the denominator.
struct Prod {
    coef: Q,
    factors: Vec<(Expr, i64)>,
}

impl Prod {
    fn new() -> Prod {
        Prod {
            coef: Q::ONE,
            factors: Vec::new(),
        }
    }

    fn push(&mut self, base: Expr, exp: i64) {
        if let Some(f) = self.factors.iter_mut().find(|(b, _)| *b == base) {
            f.1 = f.1.saturating_add(exp);
        } else {
            self.factors.push((base, exp));
        }
    }
}

/// Breaks `e^s` into the product `p` (flattening `*`, `/`, `-` and integer
/// powers, folding numeric literals).
fn decompose(e: &Expr, s: i64, p: &mut Prod) {
    match e {
        Expr::Bin(Op::Mul, l, r) => {
            decompose(l, s, p);
            decompose(r, s, p);
        }
        Expr::Bin(Op::Div, l, r) => {
            decompose(l, s, p);
            match s.checked_neg() {
                Some(ns) => decompose(r, ns, p),
                None => p.push((**r).clone(), 1),
            }
        }
        Expr::Neg(a) => {
            if s % 2 != 0 {
                p.coef = p.coef.neg();
            }
            decompose(a, s, p);
        }
        Expr::Num(text) => {
            if let Some(q) = literal(text) {
                if let Some(folded) = q.powi(s).and_then(|f| p.coef.mul(f)) {
                    p.coef = folded;
                    return;
                }
            }
            p.push(e.clone(), s);
        }
        Expr::Bin(Op::Pow, base, ex) => {
            let sex = simplify(ex);
            if let Some(k) = as_int(&sex) {
                if let Some(total) = k.checked_mul(s) {
                    if total.unsigned_abs() <= 10_000 {
                        decompose(base, total, p);
                        return;
                    }
                }
            }
            let sb = simplify(base);
            p.push(Expr::Bin(Op::Pow, b(sb), b(sex)), s);
        }
        Expr::Var(_) | Expr::Fact(_) | Expr::Call(..) | Expr::Assign(..) | Expr::Bin(..) => {
            let simple = simplify_atom(e);
            let is_compound = matches!(
                simple,
                Expr::Num(_) | Expr::Neg(_) | Expr::Bin(Op::Mul | Op::Div | Op::Pow, ..)
            );
            if is_compound && simple != *e {
                decompose(&simple, s, p);
            } else {
                p.push(simple, s);
            }
        }
    }
}

/// Simplifies a node that is not a product (sum, call, ...).
fn simplify_atom(e: &Expr) -> Expr {
    match e {
        Expr::Bin(Op::Add | Op::Sub, ..) => simplify_sum(e),
        Expr::Bin(Op::Mod, l, r) => Expr::Bin(Op::Mod, b(simplify(l)), b(simplify(r))),
        Expr::Fact(a) => Expr::Fact(b(simplify(a))),
        Expr::Call(name, args) => {
            let args: Vec<Expr> = args.iter().map(simplify).collect();
            fold_call(name, args)
        }
        other => other.clone(),
    }
}

/// A few function identities that keep derivatives short.
fn fold_call(name: &str, args: Vec<Expr>) -> Expr {
    match (name, args.as_slice()) {
        ("ln", [Expr::Var(v)]) if v == "e" => int(1),
        ("ln", [Expr::Num(n)]) if n == "1" => int(0),
        ("exp", [Expr::Num(n)]) if n == "0" => int(1),
        ("exp", [Expr::Num(n)]) if n == "1" => var("e"),
        ("sqrt", [Expr::Num(n)]) if n == "1" || n == "0" => Expr::Num(n.clone()),
        _ => Expr::Call(name.to_string(), args),
    }
}

fn base_with_exp(base: &Expr, exp: i64) -> Expr {
    if exp == 1 {
        base.clone()
    } else {
        pow(base.clone(), int(exp as i128))
    }
}

fn chain_mul(items: Vec<Expr>) -> Expr {
    let mut it = items.into_iter();
    let first = it.next().unwrap_or_else(|| int(1));
    it.fold(first, mul)
}

/// `sqrt(u)^k` is written with whole powers of `u` where possible:
/// `sqrt(u)^2 -> u`, `sqrt(u)^3 -> u*sqrt(u)`, `1/sqrt(u)^2 -> 1/u`.
/// (This relies on `u >= 0`, which `sqrt` requires anyway.)
fn fold_sqrt(p: &mut Prod) {
    for _ in 0..2 {
        let old = std::mem::take(&mut p.factors);
        for (base, exp) in old {
            if let Expr::Call(name, args) = &base {
                if name == "sqrt" && args.len() == 1 && exp.unsigned_abs() >= 2 {
                    let rest = if exp % 2 == 0 { 0 } else { exp.signum() };
                    let whole = (exp - rest) / 2;
                    decompose(&args[0], whole, p);
                    if rest != 0 {
                        p.push(base.clone(), rest);
                    }
                    continue;
                }
            }
            p.push(base, exp);
        }
    }
}

/// Rebuilds `coef * factors` as `[-] numerator / denominator`.
fn build_prod(p: &Prod) -> Expr {
    if p.coef.is_zero() {
        return int(0);
    }
    let negative = p.coef.n < 0;
    let n_abs = p.coef.n.abs();
    let mut num: Vec<Expr> = Vec::new();
    let mut den: Vec<Expr> = Vec::new();
    for (base, exp) in &p.factors {
        match exp.cmp(&0) {
            std::cmp::Ordering::Greater => num.push(base_with_exp(base, *exp)),
            std::cmp::Ordering::Less => den.push(base_with_exp(base, -*exp)),
            std::cmp::Ordering::Equal => {}
        }
    }
    if n_abs != 1 || num.is_empty() {
        num.insert(0, int(n_abs));
    }
    if p.coef.d != 1 {
        den.insert(0, int(p.coef.d));
    }
    let numerator = chain_mul(num);
    let body = if den.is_empty() {
        numerator
    } else {
        div(numerator, chain_mul(den))
    };
    if negative {
        neg(body)
    } else {
        body
    }
}

fn simplify_prod(e: &Expr) -> Expr {
    let mut p = Prod::new();
    decompose(e, 1, &mut p);
    fold_sqrt(&mut p);
    p.factors.retain(|(_, k)| *k != 0);
    build_prod(&p)
}

/// Collects the signed leaves of a sum.
fn flatten_sum(e: &Expr, sign: i128, out: &mut Vec<(i128, Expr)>) {
    match e {
        Expr::Bin(Op::Add, l, r) => {
            flatten_sum(l, sign, out);
            flatten_sum(r, sign, out);
        }
        Expr::Bin(Op::Sub, l, r) => {
            flatten_sum(l, sign, out);
            flatten_sum(r, -sign, out);
        }
        Expr::Neg(a) => flatten_sum(a, -sign, out),
        other => out.push((sign, other.clone())),
    }
}

fn factor_key(factors: &[(Expr, i64)]) -> String {
    let mut parts: Vec<String> = factors
        .iter()
        .filter(|(_, k)| *k != 0)
        .map(|(b, k)| format!("{}^{}", show(b), k))
        .collect();
    parts.sort();
    parts.join("|")
}

/// A like-terms group: (key, coefficient, factors), in order of first appearance.
type Term = (String, Q, Vec<(Expr, i64)>);

fn simplify_sum(e: &Expr) -> Expr {
    let mut leaves = Vec::new();
    flatten_sum(e, 1, &mut leaves);

    let mut terms: Vec<Term> = Vec::new();
    for (sign, leaf) in &leaves {
        let mut p = Prod::new();
        decompose(leaf, 1, &mut p);
        fold_sqrt(&mut p);
        p.factors.retain(|(_, k)| *k != 0);
        let coef = if *sign < 0 { p.coef.neg() } else { p.coef };
        let key = factor_key(&p.factors);
        match terms.iter_mut().find(|t| t.0 == key) {
            Some(t) => match t.1.add(coef) {
                Some(sum) => t.1 = sum,
                // Overflow while combining: keep the terms apart.
                None => terms.push((format!("{key}#{}", terms.len()), coef, p.factors)),
            },
            None => terms.push((key, coef, p.factors)),
        }
    }
    terms.retain(|t| !t.1.is_zero());
    if terms.is_empty() {
        return int(0);
    }

    // Positive terms first, then the negative ones as subtractions.
    let (pos, negs): (Vec<_>, Vec<_>) = terms.into_iter().partition(|t| t.1.n > 0);
    let mut acc: Option<Expr> = None;
    for (_, coef, factors) in pos {
        let term = build_prod(&Prod { coef, factors });
        acc = Some(match acc {
            Some(a) => add(a, term),
            None => term,
        });
    }
    for (_, coef, factors) in negs {
        let term = build_prod(&Prod {
            coef: coef.neg(),
            factors,
        });
        acc = Some(match acc {
            Some(a) => sub(a, term),
            None => neg(term),
        });
    }
    acc.unwrap_or_else(|| int(0))
}

/// Simplifies an expression (see the module documentation).
pub fn simplify(e: &Expr) -> Expr {
    match e {
        Expr::Num(_) | Expr::Var(_) => e.clone(),
        Expr::Neg(_) | Expr::Bin(Op::Add | Op::Sub, ..) => simplify_sum(e),
        Expr::Bin(Op::Mul | Op::Div, ..) => simplify_prod(e),
        Expr::Bin(Op::Pow, base, ex) => {
            if as_int(&simplify(ex)).is_some() {
                simplify_prod(e)
            } else {
                Expr::Bin(Op::Pow, b(simplify(base)), b(simplify(ex)))
            }
        }
        Expr::Bin(Op::Mod, ..) | Expr::Fact(_) | Expr::Call(..) | Expr::Assign(..) => {
            simplify_atom(e)
        }
    }
}

// ----------------------------------------------------------------- printing

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Bin(Op::Add | Op::Sub, ..) => 1,
        Expr::Bin(Op::Mul | Op::Div | Op::Mod, ..) => 2,
        Expr::Neg(_) => 3,
        Expr::Bin(Op::Pow, ..) => 4,
        _ => 5,
    }
}

fn wrap(e: &Expr, min: u8) -> String {
    if prec(e) < min {
        format!("({})", show(e))
    } else {
        show(e)
    }
}

/// Writes an expression with the fewest parentheses that keep its meaning.
pub fn show(e: &Expr) -> String {
    match e {
        Expr::Num(s) => s.clone(),
        Expr::Var(n) => n.clone(),
        Expr::Neg(a) => format!("-{}", wrap(a, 2)),
        Expr::Fact(a) => format!("{}!", wrap(a, 5)),
        Expr::Assign(n, a) => format!("{n} = {}", show(a)),
        Expr::Call(name, args) => {
            let list: Vec<String> = args.iter().map(show).collect();
            format!("{name}({})", list.join(", "))
        }
        Expr::Bin(op, l, r) => match op {
            Op::Add => format!("{} + {}", wrap(l, 1), wrap(r, 2)),
            Op::Sub => format!("{} - {}", wrap(l, 1), wrap(r, 2)),
            Op::Mul => format!("{}*{}", wrap(l, 2), wrap(r, 4)),
            Op::Div => format!("{}/{}", wrap(l, 2), wrap(r, 4)),
            Op::Mod => format!("{} mod {}", wrap(l, 2), wrap(r, 4)),
            Op::Pow => format!("{}^{}", wrap(l, 5), wrap_exponent(r)),
        },
    }
}

fn wrap_exponent(e: &Expr) -> String {
    // Right-associative: a plain power needs no parentheses; everything else
    // that is not an atom does (including a leading minus).
    if prec(e) >= 4 && !matches!(e, Expr::Neg(_)) {
        show(e)
    } else {
        format!("({})", show(e))
    }
}

// ----------------------------------------------------------- differentiation

struct Diff<'a> {
    v: &'a str,
    /// Radians per angle unit, as an expression.
    angle: Expr,
    notes: Vec<String>,
}

impl Diff<'_> {
    fn note(&mut self, text: &str) {
        if !self.notes.iter().any(|n| n == text) {
            self.notes.push(text.to_string());
        }
    }

    fn unsupported(name: &str, why: &str) -> CalcError {
        CalcError::Domain(format!("cannot differentiate {name}(): {why}"))
    }

    fn d(&mut self, e: &Expr) -> Result<Expr, CalcError> {
        if !depends_on(e, self.v) {
            return Ok(int(0));
        }
        match e {
            Expr::Num(_) => Ok(int(0)),
            Expr::Var(_) => Ok(int(1)),
            Expr::Neg(a) => Ok(neg(self.d(a)?)),
            Expr::Assign(..) => Err(CalcError::Syntax("cannot differentiate an assignment".into())),
            Expr::Fact(_) => Err(CalcError::Domain(
                "cannot differentiate a factorial of the variable (it needs the gamma function)"
                    .into(),
            )),
            Expr::Bin(op, l, r) => self.d_bin(*op, l, r),
            Expr::Call(name, args) => self.d_call(name, args),
        }
    }

    fn d_bin(&mut self, op: Op, l: &Expr, r: &Expr) -> Result<Expr, CalcError> {
        match op {
            Op::Add => Ok(add(self.d(l)?, self.d(r)?)),
            Op::Sub => Ok(sub(self.d(l)?, self.d(r)?)),
            Op::Mul => {
                let (dl, dr) = (self.d(l)?, self.d(r)?);
                Ok(add(mul(dl, r.clone()), mul(l.clone(), dr)))
            }
            Op::Div => {
                let (dl, dr) = (self.d(l)?, self.d(r)?);
                Ok(div(
                    sub(mul(dl, r.clone()), mul(l.clone(), dr)),
                    pow(r.clone(), int(2)),
                ))
            }
            Op::Mod => {
                if depends_on(r, self.v) {
                    return Err(CalcError::Domain(
                        "cannot differentiate 'mod' when the divisor depends on the variable"
                            .into(),
                    ));
                }
                self.note("'mod' is differentiated as if it were piecewise linear; the result is invalid at the jumps.");
                self.d(l)
            }
            Op::Pow => {
                let base_dep = depends_on(l, self.v);
                let exp_dep = depends_on(r, self.v);
                if !exp_dep {
                    // n * u^(n-1) * u'
                    let du = self.d(l)?;
                    Ok(mul(
                        mul(r.clone(), pow(l.clone(), sub(r.clone(), int(1)))),
                        du,
                    ))
                } else if !base_dep {
                    // a^g * ln(a) * g'
                    let dg = self.d(r)?;
                    Ok(mul(
                        mul(pow(l.clone(), r.clone()), call("ln", vec![l.clone()])),
                        dg,
                    ))
                } else {
                    // u^g * (g' ln u + g u' / u)
                    let (du, dg) = (self.d(l)?, self.d(r)?);
                    Ok(mul(
                        pow(l.clone(), r.clone()),
                        add(
                            mul(dg, call("ln", vec![l.clone()])),
                            div(mul(r.clone(), du), l.clone()),
                        ),
                    ))
                }
            }
        }
    }

    /// `u' * f'(u)` helper: multiplies by the inner derivative.
    fn chain(&mut self, u: &Expr, outer: Expr) -> Result<Expr, CalcError> {
        let du = self.d(u)?;
        Ok(mul(outer, du))
    }

    fn d_call(&mut self, name: &str, args: &[Expr]) -> Result<Expr, CalcError> {
        let k = self.angle.clone();
        let one_arg = |n: &str| -> Result<(), CalcError> {
            if args.len() == 1 {
                Ok(())
            } else {
                Err(CalcError::Arity {
                    name: n.to_string(),
                    expected: "1".into(),
                    got: args.len(),
                })
            }
        };
        let jump = |this: &mut Self| {
            this.note("abs, sign, floor, ceil, trunc, round and frac are differentiated where they are smooth; the result is invalid at their corners and jumps.");
        };
        match name {
            // ----- powers, roots, exponentials, logarithms
            "sqrt" => {
                one_arg(name)?;
                let u = &args[0];
                let du = self.d(u)?;
                Ok(div(du, mul(int(2), call("sqrt", vec![u.clone()]))))
            }
            "cbrt" => {
                one_arg(name)?;
                let u = &args[0];
                let du = self.d(u)?;
                Ok(div(
                    du,
                    mul(int(3), pow(call("cbrt", vec![u.clone()]), int(2))),
                ))
            }
            "root" => {
                if args.len() != 2 {
                    return Err(CalcError::Arity {
                        name: name.into(),
                        expected: "2".into(),
                        got: args.len(),
                    });
                }
                let (u, n) = (&args[0], &args[1]);
                match as_int(&simplify(n)) {
                    Some(nv) if nv > 0 && !depends_on(n, self.v) => {
                        let du = self.d(u)?;
                        Ok(div(
                            du,
                            mul(
                                n.clone(),
                                pow(call("root", vec![u.clone(), n.clone()]), int(nv as i128 - 1)),
                            ),
                        ))
                    }
                    _ => {
                        // root(u, n) = exp(ln(u)/n), valid for u > 0
                        self.d(&call("exp", vec![div(call("ln", vec![u.clone()]), n.clone())]))
                    }
                }
            }
            "exp" => {
                one_arg(name)?;
                let u = &args[0];
                self.chain(u, call("exp", vec![u.clone()]))
            }
            "ln" => {
                one_arg(name)?;
                let u = &args[0];
                let du = self.d(u)?;
                Ok(div(du, u.clone()))
            }
            "log" | "log10" => {
                one_arg(name)?;
                self.d(&div(call("ln", args.to_vec()), call("ln", vec![int(10)])))
            }
            "log2" => {
                one_arg(name)?;
                self.d(&div(call("ln", args.to_vec()), call("ln", vec![int(2)])))
            }
            "logb" => {
                if args.len() != 2 {
                    return Err(CalcError::Arity {
                        name: name.into(),
                        expected: "2".into(),
                        got: args.len(),
                    });
                }
                self.d(&div(
                    call("ln", vec![args[0].clone()]),
                    call("ln", vec![args[1].clone()]),
                ))
            }

            // ----- piecewise functions
            "abs" => {
                one_arg(name)?;
                jump(self);
                let u = &args[0];
                self.chain(u, call("sign", vec![u.clone()]))
            }
            "sign" | "floor" | "ceil" | "trunc" | "round" => {
                one_arg(name)?;
                jump(self);
                Ok(int(0))
            }
            "frac" => {
                one_arg(name)?;
                jump(self);
                self.d(&args[0])
            }

            // ----- trigonometric (angle mode aware)
            "sin" | "cos" | "tan" | "sec" | "csc" | "cot" => {
                one_arg(name)?;
                if !matches!(k, Expr::Num(_)) {
                    self.note("Angle mode is not radians, so trigonometric derivatives carry the unit-conversion factor (pi/180 for degrees, pi/200 for gradians).");
                }
                let u = &args[0];
                let c = |n: &str| call(n, vec![u.clone()]);
                let outer = match name {
                    "sin" => c("cos"),
                    "cos" => neg(c("sin")),
                    "tan" => div(int(1), pow(c("cos"), int(2))),
                    "sec" => mul(c("sec"), c("tan")),
                    "csc" => neg(mul(c("csc"), c("cot"))),
                    _ => neg(div(int(1), pow(c("sin"), int(2)))),
                };
                let du = self.d(u)?;
                Ok(mul(mul(outer, k), du))
            }
            "asin" | "acos" | "atan" | "asec" | "acsc" | "acot" => {
                one_arg(name)?;
                if !matches!(k, Expr::Num(_)) {
                    self.note("Angle mode is not radians, so inverse trigonometric derivatives carry the unit-conversion factor (180/pi for degrees, 200/pi for gradians).");
                }
                let u = &args[0];
                let u2 = || pow(u.clone(), int(2));
                let one_minus = || sqrt_of(sub(int(1), u2()));
                let minus_one = || sqrt_of(sub(u2(), int(1)));
                let outer = match name {
                    "asin" => div(int(1), one_minus()),
                    "acos" => neg(div(int(1), one_minus())),
                    "atan" => div(int(1), add(int(1), u2())),
                    "acot" => neg(div(int(1), add(int(1), u2()))),
                    "asec" => div(
                        int(1),
                        mul(call("abs", vec![u.clone()]), minus_one()),
                    ),
                    _ => neg(div(
                        int(1),
                        mul(call("abs", vec![u.clone()]), minus_one()),
                    )),
                };
                let du = self.d(u)?;
                Ok(div(mul(outer, du), k))
            }

            // ----- hyperbolic
            "sinh" | "cosh" | "tanh" | "sech" | "csch" | "coth" => {
                one_arg(name)?;
                let u = &args[0];
                let c = |n: &str| call(n, vec![u.clone()]);
                let outer = match name {
                    "sinh" => c("cosh"),
                    "cosh" => c("sinh"),
                    "tanh" => div(int(1), pow(c("cosh"), int(2))),
                    "sech" => neg(mul(c("sech"), c("tanh"))),
                    "csch" => neg(mul(c("csch"), c("coth"))),
                    _ => neg(div(int(1), pow(c("sinh"), int(2)))),
                };
                self.chain(u, outer)
            }
            "asinh" | "acosh" | "atanh" | "asech" | "acsch" | "acoth" => {
                one_arg(name)?;
                let u = &args[0];
                let u2 = || pow(u.clone(), int(2));
                let outer = match name {
                    "asinh" => div(int(1), sqrt_of(add(u2(), int(1)))),
                    "acosh" => div(int(1), sqrt_of(sub(u2(), int(1)))),
                    "atanh" | "acoth" => div(int(1), sub(int(1), u2())),
                    "asech" => neg(div(
                        int(1),
                        mul(u.clone(), sqrt_of(sub(int(1), u2()))),
                    )),
                    _ => neg(div(
                        int(1),
                        mul(call("abs", vec![u.clone()]), sqrt_of(add(int(1), u2()))),
                    )),
                };
                self.chain(u, outer)
            }

            // ----- n-ary helpers
            "sum" => {
                let mut total: Option<Expr> = None;
                for a in args {
                    let da = self.d(a)?;
                    total = Some(match total {
                        Some(t) => add(t, da),
                        None => da,
                    });
                }
                Ok(total.unwrap_or_else(|| int(0)))
            }
            "mean" => {
                let mut total: Option<Expr> = None;
                for a in args {
                    let da = self.d(a)?;
                    total = Some(match total {
                        Some(t) => add(t, da),
                        None => da,
                    });
                }
                Ok(div(
                    total.unwrap_or_else(|| int(0)),
                    int(args.len().max(1) as i128),
                ))
            }
            "min" | "max" => Err(Self::unsupported(name, "its result switches between arguments")),
            "fact" => Err(CalcError::Domain(
                "cannot differentiate a factorial of the variable (it needs the gamma function)"
                    .into(),
            )),
            "ncr" | "npr" => Err(Self::unsupported(name, "it is only defined for whole numbers")),
            "dms" | "deg" => Err(Self::unsupported(name, "it is a notation conversion, not a smooth function")),
            "rand" => Ok(int(0)),
            other => Err(CalcError::UnknownFunction(other.to_string())),
        }
    }
}

fn sqrt_of(e: Expr) -> Expr {
    call("sqrt", vec![e])
}

/// Radians per angle unit as an expression (`1` for radians).
fn angle_factor(mode: AngleMode) -> Expr {
    match mode {
        AngleMode::Radians => int(1),
        AngleMode::Degrees => div(var("pi"), int(180)),
        AngleMode::Gradians => div(var("pi"), int(200)),
    }
}

/// Differentiates `expr` with respect to `var`, `order` times, and returns the
/// simplified result. `angle` decides how trigonometric functions are treated.
pub fn differentiate(
    expr: &Expr,
    var: &str,
    order: usize,
    angle: AngleMode,
) -> Result<Derivative, CalcError> {
    let mut diff = Diff {
        v: var,
        angle: angle_factor(angle),
        notes: Vec::new(),
    };
    let mut current = expr.clone();
    for _ in 0..order.max(1) {
        let raw = diff.d(&current)?;
        current = simplify(&raw);
        if node_count(&current) > MAX_NODES {
            return Err(CalcError::Domain(
                "the derivative became too large to handle".into(),
            ));
        }
    }
    Ok(Derivative {
        expr: show(&current),
        latex: to_latex(&current),
        notes: diff.notes,
    })
}

// ------------------------------------------------------------------- LaTeX

/// Converts an expression to LaTeX math (for display purposes).
pub fn to_latex(e: &Expr) -> String {
    tex(e)
}

fn tex_var(name: &str) -> String {
    match name {
        "pi" => r"\pi".to_string(),
        n if n.chars().count() == 1 => n.to_string(),
        n => format!(r"\mathrm{{{}}}", n.replace('_', r"\_")),
    }
}

fn tex_is_atom(e: &Expr) -> bool {
    match e {
        Expr::Num(_) | Expr::Var(_) => true,
        Expr::Call(name, _) => !matches!(name.as_str(), "sqrt" | "cbrt" | "root" | "abs" | "exp"),
        _ => false,
    }
}

fn tex_tall(e: &Expr) -> bool {
    match e {
        Expr::Bin(Op::Div, ..) => true,
        Expr::Call(n, _) if matches!(n.as_str(), "sqrt" | "cbrt" | "root") => true,
        Expr::Bin(_, l, r) => tex_tall(l) || tex_tall(r),
        Expr::Neg(a) | Expr::Fact(a) => tex_tall(a),
        Expr::Call(_, args) => args.iter().any(tex_tall),
        _ => false,
    }
}

fn tex_paren(inner: &str, tall: bool) -> String {
    if tall {
        format!(r"\left({inner}\right)")
    } else {
        format!("({inner})")
    }
}

fn tex_wrap(e: &Expr, min: u8) -> String {
    if prec(e) < min {
        tex_paren(&tex(e), tex_tall(e))
    } else {
        tex(e)
    }
}

fn tex_func_name(name: &str) -> String {
    match name {
        "sin" | "cos" | "tan" | "sec" | "csc" | "cot" | "sinh" | "cosh" | "tanh" | "coth" | "ln"
        | "log" => format!(r"\{name}"),
        "asin" => r"\arcsin".into(),
        "acos" => r"\arccos".into(),
        "atan" => r"\arctan".into(),
        "log10" => r"\log_{10}".into(),
        "log2" => r"\log_{2}".into(),
        other => format!(r"\operatorname{{{other}}}"),
    }
}

fn starts_with_number(e: &Expr) -> bool {
    match e {
        Expr::Num(_) => true,
        Expr::Bin(Op::Pow | Op::Mul, l, _) => starts_with_number(l),
        _ => false,
    }
}

fn tex(e: &Expr) -> String {
    match e {
        Expr::Num(s) => s.clone(),
        Expr::Var(n) => tex_var(n),
        Expr::Neg(a) => format!("-{}", tex_wrap(a, 2)),
        Expr::Fact(a) => format!("{}!", tex_wrap(a, 5)),
        Expr::Assign(n, a) => format!("{} = {}", tex_var(n), tex(a)),
        Expr::Bin(op, l, r) => match op {
            Op::Add => format!("{} + {}", tex_wrap(l, 1), tex_wrap(r, 2)),
            Op::Sub => format!("{} - {}", tex_wrap(l, 1), tex_wrap(r, 2)),
            Op::Mul => {
                let sep = if starts_with_number(r) { r" \cdot " } else { r"\," };
                format!("{}{}{}", tex_wrap(l, 2), sep, tex_wrap(r, 4))
            }
            Op::Div => format!(r"\frac{{{}}}{{{}}}", tex(l), tex(r)),
            Op::Mod => format!(r"{} \bmod {}", tex_wrap(l, 2), tex_wrap(r, 4)),
            Op::Pow => {
                // sin(x)^2 reads best as sin^2(x)
                if let Expr::Call(name, args) = &**l {
                    if args.len() == 1
                        && matches!(
                            name.as_str(),
                            "sin" | "cos" | "tan" | "sec" | "csc" | "cot" | "sinh" | "cosh"
                                | "tanh" | "coth" | "ln" | "log"
                        )
                    {
                        return format!(
                            "{}^{{{}}}{}",
                            tex_func_name(name),
                            tex(r),
                            tex_paren(&tex(&args[0]), tex_tall(&args[0]))
                        );
                    }
                }
                let base = if tex_is_atom(l) && !matches!(&**l, Expr::Num(_) if false) {
                    tex(l)
                } else {
                    tex_paren(&tex(l), tex_tall(l))
                };
                format!("{base}^{{{}}}", tex(r))
            }
        },
        Expr::Call(name, args) => {
            let list = || args.iter().map(tex).collect::<Vec<_>>().join(", ");
            let tall = args.iter().any(tex_tall);
            match (name.as_str(), args.as_slice()) {
                ("sqrt", [a]) => format!(r"\sqrt{{{}}}", tex(a)),
                ("cbrt", [a]) => format!(r"\sqrt[3]{{{}}}", tex(a)),
                ("root", [a, n]) => format!(r"\sqrt[{}]{{{}}}", tex(n), tex(a)),
                ("abs", [a]) => format!(r"\left|{}\right|", tex(a)),
                ("exp", [a]) => format!("e^{{{}}}", tex(a)),
                _ => format!("{}{}", tex_func_name(name), tex_paren(&list(), tall)),
            }
        }
    }
}

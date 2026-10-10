//! Walks an [`Expr`] tree and computes its value.

use std::collections::HashMap;

use astro_float::{BigFloat, Radix};

use crate::error::CalcError;
use crate::functions::Registry;
use crate::num::{Ctx, Num, PRECISION, RM};
use crate::parser::{Expr, Op};

/// Everything an expression can read while it is being evaluated.
pub struct Env<'a> {
    pub ctx: &'a mut Ctx,
    pub registry: &'a Registry,
    pub vars: &'a HashMap<String, Num>,
    pub memory: Option<&'a Num>,
    pub ans: &'a Num,
}

/// Largest decimal exponent accepted in a literal. The library's binary exponent
/// is limited to +-2^31, which is about 6.46e8 in decimal; stay just inside.
const MAX_DECIMAL_EXPONENT: i64 = 646_000_000;

/// Names that cannot be assigned to.
pub fn is_reserved(name: &str) -> bool {
    matches!(name, "pi" | "e" | "ans" | "mem" | "mod")
}

/// Normalises literals such as `.5`, `5.` and `1.e3` and parses them.
fn parse_literal(ctx: &mut Ctx, text: &str) -> Result<Num, CalcError> {
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (text, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));

    // Reject literals outside the representable range *before* handing them to
    // the library, which takes unbounded time on tiny exponents ("1e-650000000").
    let int_digits = int.trim_start_matches('0');
    let frac_digits = frac.trim_start_matches('0');
    if int_digits.is_empty() && frac_digits.is_empty() {
        return Ok(Num::zero()); // 0, 0.0, 0e99999999999
    }
    let exp: i64 = match exponent {
        Some(e) => e
            .parse::<i64>()
            .unwrap_or(if e.starts_with('-') { i64::MIN / 4 } else { i64::MAX / 4 }),
        None => 0,
    };
    let lead = if int_digits.is_empty() {
        -((frac.len() - frac_digits.len()) as i64) - 1
    } else {
        int_digits.len() as i64 - 1
    };
    let magnitude = exp.saturating_add(lead);
    if magnitude > MAX_DECIMAL_EXPONENT {
        return Err(CalcError::Overflow);
    }
    if magnitude < -MAX_DECIMAL_EXPONENT {
        return Err(CalcError::Underflow);
    }

    let mut s = format!(
        "{}.{}",
        if int.is_empty() { "0" } else { int },
        if frac.is_empty() { "0" } else { frac }
    );
    if let Some(e) = exponent {
        s.push('e');
        s.push_str(e);
    }
    let v = BigFloat::parse(&s, Radix::Dec, PRECISION, RM, &mut ctx.cc);
    if v.is_nan() || v.is_inf() {
        return Err(CalcError::Overflow);
    }
    Ok(Num(v))
}

pub fn eval(expr: &Expr, env: &mut Env) -> Result<Num, CalcError> {
    match expr {
        Expr::Num(text) => parse_literal(env.ctx, text),

        Expr::Var(name) => match name.as_str() {
            "pi" => Ok(env.ctx.pi()),
            "e" => Ok(env.ctx.e()),
            "ans" => Ok(env.ans.clone()),
            "mem" => Ok(env.memory.cloned().unwrap_or_else(Num::zero)),
            _ => env
                .vars
                .get(name)
                .cloned()
                .ok_or_else(|| CalcError::UnknownIdentifier(name.clone())),
        },

        Expr::Neg(inner) => {
            let v = eval(inner, env)?;
            Ok(env.ctx.neg(&v))
        }

        Expr::Bin(op, l, r) => {
            let a = eval(l, env)?;
            let b = eval(r, env)?;
            match op {
                Op::Add => env.ctx.add(&a, &b),
                Op::Sub => env.ctx.sub(&a, &b),
                Op::Mul => env.ctx.mul(&a, &b),
                Op::Div => env.ctx.div(&a, &b),
                Op::Mod => env.ctx.modulo(&a, &b),
                Op::Pow => env.ctx.pow(&a, &b),
            }
        }

        Expr::Fact(inner) => {
            let v = eval(inner, env)?;
            env.ctx.factorial(&v)
        }

        Expr::Call(name, arg_exprs) => {
            let def = env
                .registry
                .get(name)
                .ok_or_else(|| CalcError::UnknownFunction(name.clone()))?;
            if !def.arity.accepts(arg_exprs.len()) {
                return Err(CalcError::Arity {
                    name: name.clone(),
                    expected: def.arity.to_string(),
                    got: arg_exprs.len(),
                });
            }
            let mut args = Vec::with_capacity(arg_exprs.len());
            for a in arg_exprs {
                args.push(eval(a, env)?);
            }
            (def.f)(env.ctx, &args)
        }

        Expr::Assign(..) => Err(CalcError::Syntax(
            "an assignment can only be a whole statement".into(),
        )),
    }
}

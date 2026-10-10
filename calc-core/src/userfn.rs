//! User-defined functions: `f(x, y) = expression`.
//!
//! Definitions are stored as parsed expressions and *inlined* at every use:
//! a call `f(a, b)` is replaced by the body of `f` with each parameter
//! substituted by the (already inlined) argument expression. Everything that
//! works on [`Expr`] trees (evaluation, symbolic differentiation, plotting)
//! therefore supports user functions without knowing about them.

use std::collections::{HashMap, HashSet};

use crate::error::CalcError;
use crate::lexer::{Tok, Token};
use crate::parser::Expr;

/// Calls nest at most this deep (guards against pathological definitions).
const MAX_DEPTH: usize = 64;
/// Upper bound for the node count of an inlined expression.
const MAX_NODES: usize = 200_000;

#[derive(Debug, Clone)]
pub struct UserFn {
    pub params: Vec<String>,
    pub body: Expr,
    /// Display form, e.g. `f(x, y) = x^2 + y`.
    pub text: String,
}

/// Recognises `name ( p1 , p2 ) = ...` at token level. Returns the function
/// name, its parameter names and the index of the first body token.
pub fn definition_head(tokens: &[Token]) -> Option<(String, Vec<String>, usize)> {
    let name = match tokens.first()?.tok {
        Tok::Ident(ref n) => n.clone(),
        _ => return None,
    };
    if !matches!(tokens.get(1)?.tok, Tok::LParen) {
        return None;
    }
    let mut params = Vec::new();
    let mut i = 2;
    if matches!(tokens.get(i)?.tok, Tok::RParen) {
        i += 1;
    } else {
        loop {
            match tokens.get(i)?.tok {
                Tok::Ident(ref p) => params.push(p.clone()),
                _ => return None,
            }
            i += 1;
            match tokens.get(i)?.tok {
                Tok::Comma => i += 1,
                Tok::RParen => {
                    i += 1;
                    break;
                }
                _ => return None,
            }
        }
    }
    if matches!(tokens.get(i)?.tok, Tok::Assign) {
        Some((name, params, i + 1))
    } else {
        None
    }
}

/// Names of all functions called in `e`.
pub fn calls(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Num(_) | Expr::Var(_) => {}
        Expr::Neg(a) | Expr::Fact(a) | Expr::Assign(_, a) => calls(a, out),
        Expr::Bin(_, a, b) => {
            calls(a, out);
            calls(b, out);
        }
        Expr::Call(n, args) => {
            out.insert(n.clone());
            args.iter().for_each(|a| calls(a, out));
        }
    }
}

pub fn uses_var(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Num(_) => false,
        Expr::Var(n) => n == name,
        Expr::Neg(a) | Expr::Fact(a) | Expr::Assign(_, a) => uses_var(a, name),
        Expr::Bin(_, a, b) => uses_var(a, name) || uses_var(b, name),
        Expr::Call(_, args) => args.iter().any(|a| uses_var(a, name)),
    }
}

/// Does user function `from` (directly or indirectly) call `target`?
pub fn reaches(fns: &HashMap<String, UserFn>, from: &str, target: &str) -> bool {
    let mut seen = HashSet::new();
    let mut stack = vec![from.to_string()];
    while let Some(n) = stack.pop() {
        if n == target {
            return true;
        }
        if !seen.insert(n.clone()) {
            continue;
        }
        if let Some(f) = fns.get(&n) {
            let mut c = HashSet::new();
            calls(&f.body, &mut c);
            stack.extend(c);
        }
    }
    false
}

/// Replaces every call of a user function by its inlined body.
pub fn inline(e: &Expr, fns: &HashMap<String, UserFn>) -> Result<Expr, CalcError> {
    if fns.is_empty() {
        return Ok(e.clone());
    }
    let mut budget = MAX_NODES;
    go(e, fns, 0, &mut budget)
}

fn spend(budget: &mut usize) -> Result<(), CalcError> {
    if *budget == 0 {
        return Err(CalcError::Domain(
            "the expression is too large after expanding functions".into(),
        ));
    }
    *budget -= 1;
    Ok(())
}

fn go(
    e: &Expr,
    fns: &HashMap<String, UserFn>,
    depth: usize,
    budget: &mut usize,
) -> Result<Expr, CalcError> {
    spend(budget)?;
    if depth > MAX_DEPTH {
        return Err(CalcError::Domain("functions are nested too deeply".into()));
    }
    Ok(match e {
        Expr::Num(_) | Expr::Var(_) => e.clone(),
        Expr::Neg(a) => Expr::Neg(Box::new(go(a, fns, depth, budget)?)),
        Expr::Fact(a) => Expr::Fact(Box::new(go(a, fns, depth, budget)?)),
        Expr::Assign(n, a) => Expr::Assign(n.clone(), Box::new(go(a, fns, depth, budget)?)),
        Expr::Bin(op, a, b) => Expr::Bin(
            *op,
            Box::new(go(a, fns, depth, budget)?),
            Box::new(go(b, fns, depth, budget)?),
        ),
        Expr::Call(name, args) => {
            let mut new_args = Vec::with_capacity(args.len());
            for a in args {
                new_args.push(go(a, fns, depth, budget)?);
            }
            match fns.get(name) {
                None => Expr::Call(name.clone(), new_args),
                Some(f) => {
                    if f.params.len() != new_args.len() {
                        return Err(CalcError::Arity {
                            name: name.clone(),
                            expected: f.params.len().to_string(),
                            got: new_args.len(),
                        });
                    }
                    let body = go(&f.body, fns, depth + 1, budget)?;
                    let map: HashMap<&str, &Expr> = f
                        .params
                        .iter()
                        .map(String::as_str)
                        .zip(new_args.iter())
                        .collect();
                    subst(&body, &map, budget)?
                }
            }
        }
    })
}

/// Simultaneous substitution of variables by expressions (one pass, so
/// substituted text is never substituted again).
fn subst(e: &Expr, map: &HashMap<&str, &Expr>, budget: &mut usize) -> Result<Expr, CalcError> {
    spend(budget)?;
    Ok(match e {
        Expr::Num(_) => e.clone(),
        Expr::Var(n) => match map.get(n.as_str()) {
            Some(r) => {
                let n = size(r);
                if *budget < n {
                    return Err(CalcError::Domain(
                        "the expression is too large after expanding functions".into(),
                    ));
                }
                *budget -= n;
                (*r).clone()
            }
            None => e.clone(),
        },
        Expr::Neg(a) => Expr::Neg(Box::new(subst(a, map, budget)?)),
        Expr::Fact(a) => Expr::Fact(Box::new(subst(a, map, budget)?)),
        Expr::Assign(n, a) => Expr::Assign(n.clone(), Box::new(subst(a, map, budget)?)),
        Expr::Bin(op, a, b) => Expr::Bin(
            *op,
            Box::new(subst(a, map, budget)?),
            Box::new(subst(b, map, budget)?),
        ),
        Expr::Call(n, args) => {
            let mut v = Vec::with_capacity(args.len());
            for a in args {
                v.push(subst(a, map, budget)?);
            }
            Expr::Call(n.clone(), v)
        }
    })
}

fn size(e: &Expr) -> usize {
    match e {
        Expr::Num(_) | Expr::Var(_) => 1,
        Expr::Neg(a) | Expr::Fact(a) | Expr::Assign(_, a) => 1 + size(a),
        Expr::Bin(_, a, b) => 1 + size(a) + size(b),
        Expr::Call(_, args) => 1 + args.iter().map(size).sum::<usize>(),
    }
}

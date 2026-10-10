//! Pratt parser producing an [`Expr`] tree.
//!
//! Precedence, lowest to highest:
//!
//! | level | operators                                        | associativity |
//! |-------|--------------------------------------------------|---------------|
//! | 1     | `+` `-`                                          | left          |
//! | 2     | `*` `/` `mod` `%` and implicit multiplication    | left          |
//! | 3     | unary `-` `+`                                    | prefix        |
//! | 4     | `^`                                              | right         |
//! | 5     | postfix `!` `²` `³`                              |               |
//!
//! So `-2^2` is `-4`, `2^3^2` is `512`, and `2^3!` is `2^(3!)`.

use crate::error::CalcError;
use crate::lexer::{Tok, Token};

const MAX_DEPTH: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A numeric literal, kept as text until evaluation.
    Num(String),
    Var(String),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Fact(Box<Expr>),
    Call(String, Vec<Expr>),
    /// `name = expr`; only valid as a whole statement.
    Assign(String, Box<Expr>),
}

struct Parser<'a> {
    toks: &'a [Token],
    i: usize,
    depth: usize,
    is_function: &'a dyn Fn(&str) -> bool,
}

/// Parses a token stream (as produced by [`crate::lexer::lex`]).
/// `is_function` tells the parser which names are functions.
pub fn parse(tokens: &[Token], is_function: &dyn Fn(&str) -> bool) -> Result<Expr, CalcError> {
    if tokens.len() <= 1 {
        return Err(CalcError::Syntax("empty expression".into()));
    }
    let mut p = Parser {
        toks: tokens,
        i: 0,
        depth: 0,
        is_function,
    };

    let expr = match (&tokens[0].tok, &tokens[1].tok) {
        (Tok::Ident(name), Tok::Assign) => {
            let name = name.clone();
            p.i = 2;
            let rhs = p.parse_expr(0)?;
            Expr::Assign(name, Box::new(rhs))
        }
        _ => p.parse_expr(0)?,
    };

    match &p.peek().tok {
        Tok::Eof => Ok(expr),
        _ => Err(unexpected(p.peek())),
    }
}

fn unexpected(t: &Token) -> CalcError {
    match &t.tok {
        Tok::Eof => CalcError::Syntax("the expression ends unexpectedly".into()),
        other => CalcError::Syntax(format!(
            "unexpected '{}' at position {}",
            other.describe(),
            t.pos
        )),
    }
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.toks[self.i.min(self.toks.len() - 1)]
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, CalcError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(CalcError::Syntax("expression is nested too deeply".into()));
        }
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix operators bind tighter than anything else.
            match self.peek().tok {
                Tok::Bang => {
                    self.bump();
                    lhs = Expr::Fact(Box::new(lhs));
                    continue;
                }
                Tok::Square => {
                    self.bump();
                    lhs = Expr::Bin(Op::Pow, Box::new(lhs), Box::new(Expr::Num("2".into())));
                    continue;
                }
                Tok::Cube => {
                    self.bump();
                    lhs = Expr::Bin(Op::Pow, Box::new(lhs), Box::new(Expr::Num("3".into())));
                    continue;
                }
                _ => {}
            }

            // (operator, left binding power, right binding power, consumes a token)
            let (op, l_bp, r_bp, consume) = match &self.peek().tok {
                Tok::Plus => (Op::Add, 10, 11, true),
                Tok::Minus => (Op::Sub, 10, 11, true),
                Tok::Star => (Op::Mul, 20, 21, true),
                Tok::Slash => (Op::Div, 20, 21, true),
                Tok::Percent => (Op::Mod, 20, 21, true),
                Tok::Ident(n) if n == "mod" => (Op::Mod, 20, 21, true),
                Tok::Caret => (Op::Pow, 31, 30, true),
                // Implicit multiplication: `2pi`, `3(4+1)`, `(1+2)(3+4)`, `2 sin(30)`.
                Tok::LParen | Tok::Ident(_) => (Op::Mul, 20, 21, false),
                _ => break,
            };
            if l_bp < min_bp {
                break;
            }
            if consume {
                self.bump();
            }
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }

        self.depth -= 1;
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, CalcError> {
        let t = self.bump();
        match t.tok {
            Tok::Number(s) => Ok(Expr::Num(s)),
            Tok::Minus => Ok(Expr::Neg(Box::new(self.parse_expr(25)?))),
            Tok::Plus => self.parse_expr(25),
            Tok::LParen => {
                let e = self.parse_expr(0)?;
                let close = self.bump();
                match close.tok {
                    Tok::RParen => Ok(e),
                    Tok::Eof => Err(CalcError::Syntax("missing closing ')'".into())),
                    _ => Err(unexpected(&close)),
                }
            }
            Tok::Ident(name) => {
                if name == "mod" {
                    return Err(CalcError::Syntax(format!(
                        "'mod' needs a value on its left (position {})",
                        t.pos
                    )));
                }
                if !(self.is_function)(&name) {
                    return Ok(Expr::Var(name));
                }
                if !matches!(self.peek().tok, Tok::LParen) {
                    return Err(CalcError::Syntax(format!(
                        "function '{name}' needs parentheses, e.g. {name}(x)"
                    )));
                }
                self.bump(); // '('
                let mut args = Vec::new();
                if matches!(self.peek().tok, Tok::RParen) {
                    self.bump();
                } else {
                    loop {
                        args.push(self.parse_expr(0)?);
                        let next = self.bump();
                        match next.tok {
                            Tok::Comma => continue,
                            Tok::RParen => break,
                            Tok::Eof => {
                                return Err(CalcError::Syntax("missing closing ')'".into()))
                            }
                            _ => return Err(unexpected(&next)),
                        }
                    }
                }
                Ok(Expr::Call(name, args))
            }
            _ => Err(unexpected(&t)),
        }
    }
}

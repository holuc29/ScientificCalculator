//! Programmer mode: fixed-width two's-complement integer arithmetic.
//!
//! Values are bit patterns of 8, 16, 32 or 64 bits. Arithmetic wraps around
//! like hardware (`0x7F + 1` is `-128` in 8 bits); only division by zero is an
//! error. Variables are shared with the scientific calculator but must hold
//! whole numbers that fit the word size.
//!
//! Syntax (precedence from lowest to highest, as in C):
//!
//! | operators                     | meaning                         |
//! |-------------------------------|---------------------------------|
//! | `\|` `nor`                    | OR, NOR                         |
//! | `^` `xor`                     | XOR                             |
//! | `&` `nand`                    | AND, NAND                       |
//! | `<<` `>>` `>>>`               | shift left, arithmetic right, logical right |
//! | `+` `-`                       | add, subtract                   |
//! | `*` `/` `%` `mod`             | multiply, truncating divide, remainder |
//! | unary `-` `~` `+`             | negate, bitwise NOT             |
//!
//! Functions: `rol(x, n)` and `ror(x, n)` rotate within the word size.
//! Literals: `123`, `0xFF`, `0b1010`, `0o17`, with optional `_` separators.

use std::fmt;

use crate::error::CalcError;

/// Number of bits in a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WordSize {
    Byte,
    Word,
    Dword,
    #[default]
    Qword,
}

impl WordSize {
    pub const ALL: [WordSize; 4] = [WordSize::Byte, WordSize::Word, WordSize::Dword, WordSize::Qword];

    pub fn bits(self) -> u32 {
        match self {
            WordSize::Byte => 8,
            WordSize::Word => 16,
            WordSize::Dword => 32,
            WordSize::Qword => 64,
        }
    }

    pub fn from_bits(bits: u32) -> Option<WordSize> {
        WordSize::ALL.into_iter().find(|w| w.bits() == bits)
    }

    fn mask(self) -> u64 {
        match self {
            WordSize::Qword => u64::MAX,
            w => (1u64 << w.bits()) - 1,
        }
    }
}

/// A number base for display and input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Hex,
    Dec,
    Oct,
    Bin,
}

impl Base {
    pub const ALL: [Base; 4] = [Base::Hex, Base::Dec, Base::Oct, Base::Bin];

    pub fn radix(self) -> u32 {
        match self {
            Base::Hex => 16,
            Base::Dec => 10,
            Base::Oct => 8,
            Base::Bin => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Base::Hex => "HEX",
            Base::Dec => "DEC",
            Base::Oct => "OCT",
            Base::Bin => "BIN",
        }
    }
}

/// A fixed-width integer: the bit pattern plus its width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgValue {
    pattern: u64,
    word: WordSize,
}

fn wrap_to(v: i128, word: WordSize) -> u64 {
    (v as u128 as u64) & word.mask()
}

impl ProgValue {
    /// Keeps only the low bits that fit the word size.
    pub fn from_pattern(pattern: u64, word: WordSize) -> ProgValue {
        ProgValue {
            pattern: pattern & word.mask(),
            word,
        }
    }

    /// Wraps any integer into the word size (two's complement).
    pub fn from_i128(v: i128, word: WordSize) -> ProgValue {
        ProgValue {
            pattern: wrap_to(v, word),
            word,
        }
    }

    pub fn zero(word: WordSize) -> ProgValue {
        ProgValue { pattern: 0, word }
    }

    pub fn word(self) -> WordSize {
        self.word
    }

    /// The raw bits (upper bits beyond the word size are zero).
    pub fn pattern(self) -> u64 {
        self.pattern
    }

    /// The value read as a signed (two's complement) integer.
    pub fn signed(self) -> i64 {
        sext(self.pattern, self.word) as i64
    }

    pub fn unsigned(self) -> u64 {
        self.pattern
    }

    /// Whether bit `index` (0 = least significant) is set.
    pub fn bit(self, index: u32) -> bool {
        index < self.word.bits() && (self.pattern >> index) & 1 == 1
    }

    /// The value with bit `index` flipped.
    pub fn toggled(self, index: u32) -> ProgValue {
        if index >= self.word.bits() {
            return self;
        }
        ProgValue {
            pattern: self.pattern ^ (1u64 << index),
            word: self.word,
        }
    }

    /// The same number in another word size: widening keeps the signed value,
    /// narrowing keeps the low bits.
    pub fn resized(self, word: WordSize) -> ProgValue {
        ProgValue::from_i128(self.signed() as i128, word)
    }

    /// Text in the given base. Hex and binary can be grouped in fours, octal in
    /// threes. Decimal is signed.
    pub fn format(self, base: Base, grouped: bool) -> String {
        let (digits, group) = match base {
            Base::Hex => (format!("{:X}", self.pattern), 4),
            Base::Oct => (format!("{:o}", self.pattern), 3),
            Base::Bin => (format!("{:b}", self.pattern), 4),
            Base::Dec => return self.signed().to_string(),
        };
        if !grouped {
            return digits;
        }
        let chars: Vec<char> = digits.chars().collect();
        let mut out = String::new();
        for (i, c) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i).is_multiple_of(group) {
                out.push(' ');
            }
            out.push(*c);
        }
        out
    }
}

impl fmt::Display for ProgValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.signed())
    }
}

/// Sign-extends the low `word` bits of `x` to a signed 128-bit integer.
fn sext(x: u64, word: WordSize) -> i128 {
    let bits = word.bits();
    if (x >> (bits - 1)) & 1 == 1 {
        x as i128 - (1i128 << bits)
    } else {
        x as i128
    }
}

fn fit_error(what: &str, word: WordSize) -> CalcError {
    CalcError::Domain(format!("{what} does not fit in {} bits", word.bits()))
}

/// Parses text typed into a base box (`FF`, `0xFF`, `-5`, `1010 0101`, ...).
/// Spaces and `_` are ignored. Only decimal accepts a minus sign. An empty text
/// is zero.
pub fn parse_in_base(text: &str, base: Base, word: WordSize) -> Result<ProgValue, CalcError> {
    let mut s: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect::<String>()
        .to_lowercase();
    if s.is_empty() {
        return Ok(ProgValue::zero(word));
    }
    let negative = s.starts_with('-');
    if negative {
        if base != Base::Dec {
            return Err(CalcError::Syntax(
                "a minus sign is only allowed in the DEC box".into(),
            ));
        }
        s.remove(0);
    }
    let prefix = match base {
        Base::Hex => "0x",
        Base::Oct => "0o",
        Base::Bin => "0b",
        Base::Dec => "",
    };
    if !prefix.is_empty() {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
        }
    }
    if s.is_empty() {
        return Err(CalcError::Syntax("no digits".into()));
    }
    let mut v: u128 = 0;
    for c in s.chars() {
        let d = c.to_digit(base.radix()).ok_or_else(|| {
            CalcError::Syntax(format!(
                "'{}' is not a valid {} digit",
                c.to_ascii_uppercase(),
                base.name()
            ))
        })?;
        v = v
            .checked_mul(base.radix() as u128)
            .and_then(|x| x.checked_add(d as u128))
            .ok_or_else(|| fit_error("the number", word))?;
    }
    check_literal(v, negative, word)
}

/// A magnitude (with sign) is valid if it fits the signed or unsigned range.
fn check_literal(magnitude: u128, negative: bool, word: WordSize) -> Result<ProgValue, CalcError> {
    let bits = word.bits();
    if negative {
        if magnitude > 1u128 << (bits - 1) {
            return Err(fit_error("the number", word));
        }
        Ok(ProgValue::from_i128(-(magnitude as i128), word))
    } else {
        if magnitude > word.mask() as u128 {
            return Err(fit_error("the number", word));
        }
        Ok(ProgValue::from_pattern(magnitude as u64, word))
    }
}

// ------------------------------------------------------------------- lexer

#[derive(Debug, Clone, PartialEq)]
enum T {
    Num(u128),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    Ushr,
    LParen,
    RParen,
    Comma,
    Assign,
    Eof,
}

fn describe(t: &T) -> String {
    match t {
        T::Num(n) => n.to_string(),
        T::Ident(s) => s.clone(),
        T::Plus => "+".into(),
        T::Minus => "-".into(),
        T::Star => "*".into(),
        T::Slash => "/".into(),
        T::Percent => "%".into(),
        T::Amp => "&".into(),
        T::Pipe => "|".into(),
        T::Caret => "^".into(),
        T::Tilde => "~".into(),
        T::Shl => "<<".into(),
        T::Shr => ">>".into(),
        T::Ushr => ">>>".into(),
        T::LParen => "(".into(),
        T::RParen => ")".into(),
        T::Comma => ",".into(),
        T::Assign => "=".into(),
        T::Eof => "end of input".into(),
    }
}

fn lex(input: &str) -> Result<Vec<(T, usize)>, CalcError> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let pos = i + 1;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let (radix, start) = match (c, chars.get(i + 1).map(|x| x.to_ascii_lowercase())) {
                ('0', Some('x')) => (16, i + 2),
                ('0', Some('b')) => (2, i + 2),
                ('0', Some('o')) => (8, i + 2),
                _ => (10, i),
            };
            let mut j = start;
            let mut value: u128 = 0;
            let mut digits = 0;
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                if chars[j] != '_' {
                    let d = chars[j].to_digit(radix).ok_or_else(|| {
                        CalcError::Syntax(format!(
                            "'{}' is not a valid digit in this number (position {})",
                            chars[j],
                            j + 1
                        ))
                    })?;
                    value = value
                        .checked_mul(radix as u128)
                        .and_then(|v| v.checked_add(d as u128))
                        .ok_or_else(|| CalcError::Domain("the number is too large".into()))?;
                    digits += 1;
                }
                j += 1;
            }
            if digits == 0 {
                return Err(CalcError::Syntax(format!("missing digits at position {pos}")));
            }
            if j < chars.len() && chars[j] == '.' && chars.get(j + 1).is_some_and(|d| d.is_ascii_digit()) {
                return Err(CalcError::Domain(
                    "decimal numbers are not allowed in Programmer mode (whole numbers only)".into(),
                ));
            }
            out.push((T::Num(value), pos));
            i = j;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect::<String>().to_lowercase();
            out.push((T::Ident(word), pos));
            i = j;
            continue;
        }
        let two = |a: char| chars.get(i + 1) == Some(&a);
        let (tok, len) = match c {
            '+' => (T::Plus, 1),
            '-' => (T::Minus, 1),
            '*' => (T::Star, 1),
            '/' => (T::Slash, 1),
            '%' => (T::Percent, 1),
            '&' => (T::Amp, 1),
            '|' => (T::Pipe, 1),
            '^' => (T::Caret, 1),
            '~' => (T::Tilde, 1),
            '(' => (T::LParen, 1),
            ')' => (T::RParen, 1),
            ',' => (T::Comma, 1),
            '=' => (T::Assign, 1),
            '<' if two('<') => (T::Shl, 2),
            '>' if two('>') && chars.get(i + 2) == Some(&'>') => (T::Ushr, 3),
            '>' if two('>') => (T::Shr, 2),
            '×' => (T::Star, 1),
            '÷' => (T::Slash, 1),
            '−' => (T::Minus, 1),
            other => {
                return Err(CalcError::Syntax(format!(
                    "unexpected character '{other}' at position {pos}"
                )))
            }
        };
        out.push((tok, pos));
        i += len;
    }
    out.push((T::Eof, chars.len() + 1));
    Ok(out)
}

// ------------------------------------------------------------------ parser

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum B {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Shl,
    Shr,
    Ushr,
}

#[derive(Debug, Clone)]
enum P {
    Lit(u128),
    Var(String),
    Neg(Box<P>),
    Not(Box<P>),
    Bin(B, Box<P>, Box<P>),
    Call(String, Vec<P>),
}

struct Parser {
    toks: Vec<(T, usize)>,
    i: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 200;

impl Parser {
    fn peek(&self) -> &(T, usize) {
        &self.toks[self.i]
    }

    fn next(&mut self) -> (T, usize) {
        let t = self.toks[self.i].clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }

    fn unexpected(&self) -> CalcError {
        let (t, pos) = self.peek();
        CalcError::Syntax(format!("unexpected '{}' at position {pos}", describe(t)))
    }

    /// The operator at this precedence level, if the next token is one.
    fn op_at(&self, level: u8) -> Option<B> {
        let op = match &self.peek().0 {
            T::Pipe => (1, B::Or),
            T::Ident(s) if s == "nor" => (1, B::Nor),
            T::Caret => (2, B::Xor),
            T::Ident(s) if s == "xor" => (2, B::Xor),
            T::Amp => (3, B::And),
            T::Ident(s) if s == "nand" => (3, B::Nand),
            T::Shl => (4, B::Shl),
            T::Shr => (4, B::Shr),
            T::Ushr => (4, B::Ushr),
            T::Plus => (5, B::Add),
            T::Minus => (5, B::Sub),
            T::Star => (6, B::Mul),
            T::Slash => (6, B::Div),
            T::Percent => (6, B::Mod),
            T::Ident(s) if s == "mod" => (6, B::Mod),
            _ => return None,
        };
        (op.0 == level).then_some(op.1)
    }

    fn level(&mut self, level: u8) -> Result<P, CalcError> {
        if level > 6 {
            return self.unary();
        }
        let mut left = self.level(level + 1)?;
        while let Some(op) = self.op_at(level) {
            self.next();
            let right = self.level(level + 1)?;
            left = P::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<P, CalcError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(CalcError::Syntax("expression is nested too deeply".into()));
        }
        let r = match self.peek().0.clone() {
            T::Minus => {
                self.next();
                Ok(P::Neg(Box::new(self.unary()?)))
            }
            T::Plus => {
                self.next();
                self.unary()
            }
            T::Tilde => {
                self.next();
                Ok(P::Not(Box::new(self.unary()?)))
            }
            _ => self.primary(),
        };
        self.depth -= 1;
        r
    }

    fn primary(&mut self) -> Result<P, CalcError> {
        match self.peek().0.clone() {
            T::Num(v) => {
                self.next();
                Ok(P::Lit(v))
            }
            T::Ident(name) => {
                self.next();
                if self.peek().0 == T::LParen {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek().0 != T::RParen {
                        loop {
                            args.push(self.level(1)?);
                            if self.peek().0 == T::Comma {
                                self.next();
                            } else {
                                break;
                            }
                        }
                    }
                    if self.peek().0 != T::RParen {
                        return Err(self.unexpected());
                    }
                    self.next();
                    Ok(P::Call(name, args))
                } else {
                    Ok(P::Var(name))
                }
            }
            T::LParen => {
                self.next();
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return Err(CalcError::Syntax("expression is nested too deeply".into()));
                }
                let inner = self.level(1)?;
                self.depth -= 1;
                if self.peek().0 != T::RParen {
                    return Err(self.unexpected());
                }
                self.next();
                Ok(inner)
            }
            _ => Err(self.unexpected()),
        }
    }
}

/// `name = expression` or just an expression.
struct Statement {
    target: Option<String>,
    body: P,
}

fn parse(input: &str) -> Result<Statement, CalcError> {
    let toks = lex(input)?;
    if toks.len() <= 1 {
        return Err(CalcError::Syntax("empty expression".into()));
    }
    let mut p = Parser {
        toks,
        i: 0,
        depth: 0,
    };
    let target = match (&p.toks[0].0, &p.toks[1].0) {
        (T::Ident(name), T::Assign) => {
            let name = name.clone();
            p.i = 2;
            Some(name)
        }
        _ => None,
    };
    let body = p.level(1)?;
    if p.peek().0 != T::Eof {
        return Err(p.unexpected());
    }
    Ok(Statement { target, body })
}

// --------------------------------------------------------------- evaluation

/// Supplies the value of a variable as an integer (or an error).
pub(crate) type Lookup<'a> = dyn FnMut(&str) -> Result<i128, CalcError> + 'a;

fn eval_p(p: &P, word: WordSize, lookup: &mut Lookup) -> Result<u64, CalcError> {
    let m = word.mask();
    let bits = word.bits();
    match p {
        P::Lit(v) => {
            if *v > m as u128 {
                return Err(fit_error(&format!("the number {v}"), word));
            }
            Ok(*v as u64)
        }
        P::Var(name) => {
            let v = lookup(name)?;
            let min = -(1i128 << (bits - 1));
            if v < min || v > m as i128 {
                return Err(fit_error(&format!("variable '{name}' = {v}"), word));
            }
            Ok(wrap_to(v, word))
        }
        P::Neg(a) => {
            let x = eval_p(a, word, lookup)?;
            Ok(wrap_to(-sext(x, word), word))
        }
        P::Not(a) => Ok(!eval_p(a, word, lookup)? & m),
        P::Call(name, args) => {
            let want = |n: usize| -> Result<(), CalcError> {
                if args.len() == n {
                    Ok(())
                } else {
                    Err(CalcError::Arity {
                        name: name.clone(),
                        expected: n.to_string(),
                        got: args.len(),
                    })
                }
            };
            match name.as_str() {
                "rol" | "ror" => {
                    want(2)?;
                    let x = eval_p(&args[0], word, lookup)?;
                    let n = sext(eval_p(&args[1], word, lookup)?, word);
                    if n < 0 {
                        return Err(CalcError::Domain("the rotate count must not be negative".into()));
                    }
                    let mut k = (n % bits as i128) as u32;
                    if name == "ror" {
                        k = (bits - k) % bits;
                    }
                    Ok(if k == 0 { x } else { ((x << k) | (x >> (bits - k))) & m })
                }
                other => Err(CalcError::UnknownFunction(other.to_string())),
            }
        }
        P::Bin(op, l, r) => {
            let a = eval_p(l, word, lookup)?;
            let b = eval_p(r, word, lookup)?;
            let (sa, sb) = (sext(a, word), sext(b, word));
            Ok(match op {
                B::Add => wrap_to(sa + sb, word),
                B::Sub => wrap_to(sa - sb, word),
                B::Mul => wrap_to(sa * sb, word),
                B::Div => {
                    if sb == 0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    wrap_to(sa / sb, word)
                }
                B::Mod => {
                    if sb == 0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    wrap_to(sa % sb, word)
                }
                B::And => a & b,
                B::Or => a | b,
                B::Xor => a ^ b,
                B::Nand => !(a & b) & m,
                B::Nor => !(a | b) & m,
                B::Shl | B::Shr | B::Ushr => {
                    if sb < 0 {
                        return Err(CalcError::Domain("the shift count must not be negative".into()));
                    }
                    let big = sb >= bits as i128;
                    let n = sb as u32;
                    match op {
                        B::Shl => {
                            if big {
                                0
                            } else {
                                (a << n) & m
                            }
                        }
                        B::Ushr => {
                            if big {
                                0
                            } else {
                                a >> n
                            }
                        }
                        _ => {
                            if big {
                                if sa < 0 {
                                    m
                                } else {
                                    0
                                }
                            } else {
                                wrap_to(sa >> n, word)
                            }
                        }
                    }
                }
            })
        }
    }
}

/// The result of one Programmer-mode statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub value: ProgValue,
    /// The variable that was assigned, if the input was `name = expression`.
    pub assigned: Option<String>,
}

/// Parses and evaluates one statement.
pub(crate) fn run(
    input: &str,
    word: WordSize,
    lookup: &mut Lookup,
) -> Result<(ProgValue, Option<String>), CalcError> {
    let stmt = parse(input)?;
    let pattern = eval_p(&stmt.body, word, lookup)?;
    Ok((ProgValue::from_pattern(pattern, word), stmt.target))
}

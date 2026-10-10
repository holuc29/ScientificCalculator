//! The number type, the evaluation context, and decimal rendering.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use astro_float::{BigFloat, Consts, Radix, RoundingMode};

use crate::error::CalcError;

/// Internal working precision in bits (about 77 decimal digits).
pub const PRECISION: usize = 256;
/// Rounding mode used for every operation.
pub const RM: RoundingMode = RoundingMode::ToEven;
/// Significant digits shown by default.
pub const DEFAULT_DISPLAY_DIGITS: usize = 32;
/// Upper bound for the display setting; kept well below the working precision
/// so that accumulated rounding error never becomes visible.
pub const MAX_DISPLAY_DIGITS: usize = 60;

/// An arbitrary-precision real number.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Num(pub(crate) BigFloat);

impl Num {
    pub fn zero() -> Num {
        Num(BigFloat::new(PRECISION))
    }

    pub fn one() -> Num {
        Num::from_i64(1)
    }

    pub fn from_i64(v: i64) -> Num {
        Num(BigFloat::from_i64(v, PRECISION))
    }

    pub fn from_u64(v: u64) -> Num {
        Num(BigFloat::from_u64(v, PRECISION))
    }

    pub fn from_f64(v: f64) -> Num {
        Num(BigFloat::from_f64(v, PRECISION))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        !self.0.is_zero() && self.0.is_negative()
    }

    pub fn is_int(&self) -> bool {
        self.0.is_zero() || self.0.is_int()
    }

    pub(crate) fn abs(&self) -> Num {
        Num(self.0.abs())
    }
}

/// How trigonometric functions interpret angles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AngleMode {
    #[default]
    Degrees,
    Radians,
    Gradians,
}

impl AngleMode {
    pub fn short_name(self) -> &'static str {
        match self {
            AngleMode::Degrees => "DEG",
            AngleMode::Radians => "RAD",
            AngleMode::Gradians => "GRAD",
        }
    }
}

impl fmt::Display for AngleMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AngleMode::Degrees => "degrees",
            AngleMode::Radians => "radians",
            AngleMode::Gradians => "gradians",
        })
    }
}

impl FromStr for AngleMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "deg" | "degree" | "degrees" => Ok(AngleMode::Degrees),
            "rad" | "radian" | "radians" => Ok(AngleMode::Radians),
            "grad" | "grads" | "gradian" | "gradians" | "gon" => Ok(AngleMode::Gradians),
            other => Err(format!(
                "unknown angle mode '{other}' (use deg, rad or grad)"
            )),
        }
    }
}

/// A decimal expansion `d0.d1d2d3... x 10^exp10`.
#[derive(Debug, Clone)]
pub struct DecParts {
    pub negative: bool,
    pub digits: Vec<u8>,
    pub exp10: i64,
}

impl DecParts {
    pub fn is_zero(&self) -> bool {
        self.digits.iter().all(|&d| d == 0)
    }
}

/// Mutable state needed to evaluate math: the constants cache, the angle
/// mode, and a small random number generator.
pub struct Ctx {
    pub(crate) cc: Consts,
    pub angle: AngleMode,
    rng: u64,
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

impl Ctx {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Ctx {
            cc: Consts::new().expect("failed to initialise the constants cache"),
            angle: AngleMode::default(),
            rng: seed | 1,
        }
    }

    /// A uniformly distributed number in `[0, 1)` (xorshift64*; not for cryptography).
    pub fn next_random(&mut self) -> Num {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let out = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        Num::from_f64((out >> 11) as f64 / (1u64 << 53) as f64)
    }

    /// Splits a number into sign, decimal digits and decimal exponent.
    pub fn decimal_parts(&mut self, n: &Num) -> Result<DecParts, CalcError> {
        let s = n
            .0
            .format(Radix::Dec, RM, &mut self.cc)
            .map_err(|_| CalcError::Overflow)?;
        let (negative, rest) = match s.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, s.as_str()),
        };
        if !rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            // "Inf" / "NaN" / "Err"
            return Err(CalcError::Overflow);
        }
        let (mantissa, exp) = match rest.split_once(['e', 'E']) {
            Some((m, e)) => (
                m,
                e.parse::<i64>()
                    .map_err(|_| CalcError::Syntax("bad exponent in number".into()))?,
            ),
            None => (rest, 0),
        };
        let digits: Vec<u8> = mantissa
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(|b| b - b'0')
            .collect();
        if digits.iter().all(|&d| d == 0) {
            return Ok(DecParts {
                negative: false,
                digits: vec![0],
                exp10: 0,
            });
        }
        Ok(DecParts {
            negative,
            digits,
            exp10: exp,
        })
    }

    /// The value as an unsigned integer, if it is a non-negative integer below 10^19.
    pub fn to_u64(&mut self, n: &Num) -> Option<u64> {
        let n = &self.as_integer(n)?;
        if n.is_zero() {
            return Some(0);
        }
        if n.is_negative() {
            return None;
        }
        let p = self.decimal_parts(n).ok()?;
        if p.exp10 < 0 || p.exp10 > 18 {
            return None;
        }
        let mut v: u64 = 0;
        for i in 0..=(p.exp10 as usize) {
            v = v * 10 + u64::from(p.digits.get(i).copied().unwrap_or(0));
        }
        Some(v)
    }

    /// The value as a signed 128-bit integer, if it is a whole number within
    /// that range (tiny binary noise around an integer is tolerated).
    pub fn to_i128(&mut self, n: &Num) -> Option<i128> {
        let n = &self.as_integer(n)?;
        if n.is_zero() {
            return Some(0);
        }
        let p = self.decimal_parts(n).ok()?;
        if p.exp10 < 0 || p.exp10 > 37 {
            return None;
        }
        let mut v: i128 = 0;
        for i in 0..=(p.exp10 as usize) {
            v = v
                .checked_mul(10)?
                .checked_add(i128::from(p.digits.get(i).copied().unwrap_or(0)))?;
        }
        Some(if p.negative { -v } else { v })
    }

    /// Nearest `f64` (for plotting and other lossy uses).
    pub fn to_f64(&mut self, n: &Num) -> Option<f64> {
        let p = self.decimal_parts(n).ok()?;
        let mut s = String::new();
        if p.negative {
            s.push('-');
        }
        s.push((b'0' + p.digits[0]) as char);
        s.push('.');
        for d in &p.digits[1..] {
            s.push((b'0' + d) as char);
        }
        s.push_str(&format!("e{}", p.exp10));
        s.parse::<f64>().ok()
    }

    /// Renders a number with at most `sig` significant digits.
    pub fn format(&mut self, n: &Num, sig: usize) -> String {
        match self.decimal_parts(n) {
            Ok(p) => render(&p, sig.clamp(1, MAX_DISPLAY_DIGITS)),
            Err(_) => "Overflow".to_string(),
        }
    }
}

fn digit_char(d: u8) -> char {
    (b'0' + d) as char
}

/// Rounds the decimal expansion to `sig` significant digits and prints it,
/// using plain notation for moderate magnitudes and scientific notation
/// (`1.5e-10`, `2e+40`) otherwise. The output can be fed back as input.
pub fn render(parts: &DecParts, sig: usize) -> String {
    if parts.is_zero() {
        return "0".to_string();
    }
    let mut digits = parts.digits.clone();
    let mut exp = parts.exp10;
    while digits.len() > 1 && digits[0] == 0 {
        digits.remove(0);
        exp -= 1;
    }
    if digits.len() > sig {
        let round_up = digits[sig] >= 5;
        digits.truncate(sig);
        if round_up {
            let mut i = sig;
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    digits.truncate(sig);
                    exp += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }

    let mut out = String::new();
    if parts.negative {
        out.push('-');
    }
    if (-6..sig as i64).contains(&exp) {
        if exp >= 0 {
            let int_len = exp as usize + 1;
            for i in 0..int_len {
                out.push(digit_char(digits.get(i).copied().unwrap_or(0)));
            }
            if digits.len() > int_len {
                out.push('.');
                out.extend(digits[int_len..].iter().map(|&d| digit_char(d)));
            }
        } else {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            out.extend(digits.iter().map(|&d| digit_char(d)));
        }
    } else {
        out.push(digit_char(digits[0]));
        if digits.len() > 1 {
            out.push('.');
            out.extend(digits[1..].iter().map(|&d| digit_char(d)));
        }
        out.push('e');
        if exp >= 0 {
            out.push('+');
        }
        out.push_str(&exp.to_string());
    }
    out
}

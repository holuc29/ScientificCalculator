//! Mathematical operations on [`Num`], all going through [`Ctx`].
//!
//! Every operation validates its domain and converts the NaN / infinity
//! values of the underlying library into proper [`CalcError`]s.

use astro_float::BigFloat;

use crate::error::CalcError;
use crate::num::{AngleMode, Ctx, Num, PRECISION, RM};

type R = Result<Num, CalcError>;

/// Largest argument accepted by `n!`.
pub const MAX_FACTORIAL: u64 = 10_000;

fn chk(op: &str, v: BigFloat) -> R {
    if v.is_nan() {
        Err(CalcError::Domain(format!("invalid input for {op}")))
    } else if v.is_inf() {
        Err(CalcError::Overflow)
    } else {
        Ok(Num(v))
    }
}

/// A zero result from non-zero inputs means the true value was too small to
/// represent; say so instead of silently returning 0.
fn underflow_check(r: Num, inputs_nonzero: bool) -> R {
    if r.is_zero() && inputs_nonzero {
        Err(CalcError::Underflow)
    } else {
        Ok(r)
    }
}

/// Residues below this are treated as rounding noise around an exact zero
/// (2^-200, i.e. about 6e-61, against a working precision of 2^-256).
fn tolerance() -> Num {
    Num::from_f64(2f64.powi(-200))
}

/// Relative size of rounding noise: 2^-248, i.e. about 256 ulps of the working
/// precision. A long chain of operations can accumulate this much error, so a
/// difference below it carries no information.
fn noise() -> Num {
    Num::from_f64(2f64.powi(-248))
}

/// If a sum or difference is pure cancellation noise relative to its operands
/// (`0.3 - 0.1*3`), return an exact zero instead of a residue like 1e-77.
fn flush_cancellation(r: BigFloat, a: &Num, b: &Num) -> BigFloat {
    if r.is_zero() || r.is_nan() || r.is_inf() {
        return r;
    }
    let (aa, ab) = (a.abs(), b.abs());
    let scale = if aa > ab { aa } else { ab };
    let limit = Num(scale.0.mul(&noise().0, PRECISION, RM));
    if Num(r.abs()) <= limit {
        BigFloat::new(PRECISION)
    } else {
        r
    }
}

impl Ctx {
    // ------------------------------------------------------- noise-tolerant integers

    /// If `x` lies within rounding noise of an integer, returns that integer.
    ///
    /// Inputs such as `0.7` are binary approximations, so `0.7 * 10` can come out
    /// one ulp away from 7. Every operation that cares about integrality (floor,
    /// mod, factorial, exact powers, ...) goes through this so that decimal
    /// arithmetic behaves the way it reads on paper.
    pub fn as_integer(&self, x: &Num) -> Option<Num> {
        if x.is_zero() {
            return Some(Num::zero());
        }
        let half = BigFloat::from_f64(0.5, PRECISION);
        let nearest = Num(x.0.add(&half, PRECISION, RM).floor());
        let diff = Num(x.0.sub(&nearest.0, PRECISION, RM).abs());
        let limit = Num(x.0.abs().mul(&noise().0, PRECISION, RM));
        if diff <= limit {
            Some(nearest)
        } else {
            None
        }
    }

    /// `as_integer`, or `x` itself when it is not close to an integer.
    pub(crate) fn snap_int(&self, x: &Num) -> Num {
        self.as_integer(x).unwrap_or_else(|| x.clone())
    }

    // ---------------------------------------------------------------- constants

    pub fn pi(&mut self) -> Num {
        Num(self.cc.pi(PRECISION, RM))
    }

    pub fn e(&mut self) -> Num {
        Num(self.cc.e(PRECISION, RM))
    }

    // -------------------------------------------------------------- arithmetic

    pub fn add(&self, a: &Num, b: &Num) -> R {
        chk("addition", flush_cancellation(a.0.add(&b.0, PRECISION, RM), a, b))
    }

    pub fn sub(&self, a: &Num, b: &Num) -> R {
        chk("subtraction", flush_cancellation(a.0.sub(&b.0, PRECISION, RM), a, b))
    }

    pub fn mul(&self, a: &Num, b: &Num) -> R {
        let r = chk("multiplication", a.0.mul(&b.0, PRECISION, RM))?;
        underflow_check(r, !a.is_zero() && !b.is_zero())
    }

    pub fn div(&self, a: &Num, b: &Num) -> R {
        if b.is_zero() {
            return Err(if a.is_zero() {
                CalcError::Undefined("0 / 0".into())
            } else {
                CalcError::DivisionByZero
            });
        }
        let r = chk("division", a.0.div(&b.0, PRECISION, RM))?;
        underflow_check(r, !a.is_zero())
    }

    pub fn neg(&self, a: &Num) -> Num {
        Num(a.0.neg())
    }

    /// Remainder with the sign of the dividend (like C's `fmod`).
    pub fn modulo(&self, a: &Num, b: &Num) -> R {
        if b.is_zero() {
            return Err(CalcError::DivisionByZero);
        }
        // If a / b is an integer up to noise, the remainder is zero ("0.3 mod 0.1").
        // Only for moderate quotients: beyond 2^128 the quotient's own resolution
        // is coarser than 1 and "integer up to noise" would be meaningless.
        let q = self.div(a, b)?;
        if q.abs() < Num::from_f64(2f64.powi(128)) && self.as_integer(&q).is_some() {
            return Ok(Num::zero());
        }
        chk("modulo", a.0.rem(&b.0))
    }

    pub fn pow(&mut self, x: &Num, y: &Num) -> R {
        let r = self.pow_unchecked(x, y)?;
        underflow_check(r, !x.is_zero())
    }

    fn pow_unchecked(&mut self, x: &Num, y: &Num) -> R {
        let (x, y) = (&self.snap_int(x), &self.snap_int(y));
        if x.is_zero() {
            if y.is_zero() {
                return Err(CalcError::Undefined("0 ^ 0".into()));
            }
            if y.is_negative() {
                return Err(CalcError::DivisionByZero);
            }
            return Ok(Num::zero());
        }
        if y.is_zero() || *x == Num::one() {
            return Ok(Num::one());
        }
        if y.is_int() {
            if let Some(n) = self.to_u64(&y.abs()).filter(|&n| n <= u64::from(u32::MAX)) {
                // Repeated squaring keeps small integer powers exact (2^10 == 1024).
                let r = match chk("power", x.0.powi(n as usize, PRECISION, RM)) {
                    // |x|^n too large with a negative exponent: the true value is tiny.
                    Err(CalcError::Overflow) if y.is_negative() && x.abs() > Num::one() => {
                        return Err(CalcError::Underflow)
                    }
                    other => other?,
                };
                if r.is_zero() {
                    // |x|^n underflowed; with a negative exponent the true value is huge.
                    return Err(if y.is_negative() {
                        CalcError::Overflow
                    } else {
                        CalcError::Underflow
                    });
                }
                return if y.is_negative() {
                    self.div(&Num::one(), &r)
                } else {
                    Ok(r)
                };
            }
        }
        if x.is_negative() {
            return Err(CalcError::Domain(
                "a negative base needs an integer exponent".into(),
            ));
        }
        if let Some(r) = self.pow_dyadic(x, y) {
            return r;
        }
        let r = chk("power", x.0.pow(&y.0, PRECISION, RM, &mut self.cc))?;
        Ok(self.snap_root(x, y, r))
    }

    /// `x^y` for exponents of the form `j / 2^k` (k <= 32), such as 0.5, 0.25 or 2.75.
    ///
    /// The library's general `pow` never returns when the exact result is
    /// representable (`4^0.5` is exactly 2), so these exponents are computed as
    /// `x^j` followed by `k` square roots, which is exact whenever the result is.
    ///
    /// This covers every case where the library would hang: an exact result of
    /// `x^(j/2^k)` requires `x` to be a 2^k-th power, which for k > 8 is only
    /// possible for a power of two (k <= 31 because of the exponent range) or for
    /// `x = 1`, and `x = 1` is handled by the caller.
    fn pow_dyadic(&mut self, x: &Num, y: &Num) -> Option<R> {
        let two = BigFloat::from_i32(2, PRECISION);
        let mut scaled = y.abs().0;
        for k in 1..=32u32 {
            scaled = scaled.mul(&two, PRECISION, RM);
            if scaled.is_int() {
                let j = usize::try_from(self.to_u64(&Num(scaled))?).ok()?;
                return match self.pow_dyadic_apply(x, j, k, y.is_negative()) {
                    // x^j may overflow even though x^(j/2^k) does not.
                    Err(CalcError::Overflow) => None,
                    other => Some(other),
                };
            }
        }
        None
    }

    fn pow_dyadic_apply(&mut self, x: &Num, j: usize, k: u32, negative: bool) -> R {
        let mut t = chk("power", x.0.powi(j, PRECISION, RM))?;
        for _ in 0..k {
            t = self.sqrt(&t)?;
        }
        if negative {
            self.div(&Num::one(), &t)
        } else {
            Ok(t)
        }
    }

    /// If `y` is `1/n` for a small integer `n` and `r` is within noise of an
    /// integer `k` with `k^n == x`, return `k` exactly (so `27^(1/3)` is 3, not
    /// 2.99999...), which keeps `floor(27^(1/3))` correct.
    fn snap_root(&mut self, x: &Num, y: &Num, r: Num) -> Num {
        if y.is_negative() {
            return r;
        }
        let Ok(q) = self.div(&Num::one(), y) else {
            return r;
        };
        let half = BigFloat::from_f64(0.5, PRECISION);
        let n = Num(q.0.add(&half, PRECISION, RM).floor());
        if n < Num::from_i64(2) || n > Num::from_i64(64) {
            return r;
        }
        if Num(q.0.sub(&n.0, PRECISION, RM).abs()) > tolerance() {
            return r;
        }
        let Some(n) = self.to_u64(&n) else { return r };
        let k = Num(r.0.add(&half, PRECISION, RM).floor());
        let kn = k.0.powi(n as usize, PRECISION, RM);
        let d = Num(kn.sub(&x.0, PRECISION, RM).abs());
        let limit = Num(x.0.abs().mul(
            &BigFloat::from_f64(2f64.powi(-240), PRECISION),
            PRECISION,
            RM,
        ));
        if d <= limit {
            k
        } else {
            r
        }
    }

    pub fn sqrt(&self, x: &Num) -> R {
        let x = &self.snap_int(x);
        if x.is_negative() {
            return Err(CalcError::Domain("square root of a negative number".into()));
        }
        chk("sqrt", x.0.sqrt(PRECISION, RM))
    }

    pub fn cbrt(&self, x: &Num) -> R {
        chk("cbrt", self.snap_int(x).0.cbrt(PRECISION, RM))
    }

    /// The `n`-th root of `x` (`root(x, 3)` is the cube root).
    pub fn root(&mut self, x: &Num, n: &Num) -> R {
        let n = &self.snap_int(n);
        if n.is_zero() {
            return Err(CalcError::Domain("the root degree must not be 0".into()));
        }
        let two = Num::from_i64(2);
        let three = Num::from_i64(3);
        if *n == two {
            return self.sqrt(x);
        }
        if *n == three {
            return self.cbrt(x);
        }
        if x.is_negative() {
            // A real root of a negative number exists only for odd integer degrees.
            let odd = n.is_int() && {
                let half = self.div(n, &two)?;
                !half.is_int()
            };
            if !odd {
                return Err(CalcError::Domain(
                    "even root of a negative number".into(),
                ));
            }
            let r = self.root(&x.abs(), n)?;
            return Ok(self.neg(&r));
        }
        let inv = self.div(&Num::one(), n)?;
        self.pow(x, &inv)
    }

    // ------------------------------------------------------- rounding helpers

    pub fn abs(&self, x: &Num) -> Num {
        x.abs()
    }

    pub fn floor(&self, x: &Num) -> Num {
        Num(self.snap_int(x).0.floor())
    }

    pub fn ceil(&self, x: &Num) -> Num {
        Num(self.snap_int(x).0.ceil())
    }

    pub fn trunc(&self, x: &Num) -> Num {
        Num(self.snap_int(x).0.int())
    }

    pub fn frac(&self, x: &Num) -> Num {
        Num(self.snap_int(x).0.fract())
    }

    /// Rounds half away from zero. Values within noise of a half-integer
    /// (`1.005 * 100` is 100.5 on paper) are treated as exactly that half.
    pub fn round(&self, x: &Num) -> R {
        let two = Num::from_i64(2);
        let twice = self.mul(x, &two)?;
        let exact;
        let x = match self.as_integer(&twice) {
            Some(n) => {
                exact = self.div(&n, &two)?; // n / 2 is exact in binary
                &exact
            }
            None => x,
        };
        let half = Num::from_f64(0.5);
        if x.is_negative() {
            let r = self.add(&x.abs(), &half)?;
            Ok(self.neg(&self.floor(&r)))
        } else {
            let r = self.add(x, &half)?;
            Ok(self.floor(&r))
        }
    }

    pub fn sign(&self, x: &Num) -> Num {
        if x.is_zero() {
            Num::zero()
        } else if x.is_negative() {
            Num::from_i64(-1)
        } else {
            Num::one()
        }
    }

    // ------------------------------------------------- exponentials, logarithms

    pub fn exp(&mut self, x: &Num) -> R {
        let r = chk("exp", x.0.exp(PRECISION, RM, &mut self.cc))?;
        underflow_check(r, true)
    }

    pub fn ln(&mut self, x: &Num) -> R {
        if x.is_zero() || x.is_negative() {
            return Err(CalcError::Domain("logarithm of a non-positive number".into()));
        }
        chk("ln", x.0.ln(PRECISION, RM, &mut self.cc))
    }

    pub fn log10(&mut self, x: &Num) -> R {
        if x.is_zero() || x.is_negative() {
            return Err(CalcError::Domain("logarithm of a non-positive number".into()));
        }
        let r = chk("log10", x.0.log10(PRECISION, RM, &mut self.cc))?;
        Ok(self.snap_log(x, &Num::from_i64(10), r))
    }

    pub fn log2(&mut self, x: &Num) -> R {
        if x.is_zero() || x.is_negative() {
            return Err(CalcError::Domain("logarithm of a non-positive number".into()));
        }
        let r = chk("log2", x.0.log2(PRECISION, RM, &mut self.cc))?;
        Ok(self.snap_log(x, &Num::from_i64(2), r))
    }

    /// Logarithm of `x` to an arbitrary base.
    pub fn logb(&mut self, x: &Num, base: &Num) -> R {
        if base.is_zero() || base.is_negative() || *base == Num::one() {
            return Err(CalcError::Domain(
                "the logarithm base must be positive and not 1".into(),
            ));
        }
        let lx = self.ln(x)?;
        let lb = self.ln(base)?;
        let r = self.div(&lx, &lb)?;
        Ok(self.snap_log(x, base, r))
    }

    /// If `r` is within noise of an integer `k` and `base^k == x`, return `k` exactly,
    /// so that e.g. `floor(log10(1000))` is 3 rather than 2.
    fn snap_log(&mut self, x: &Num, base: &Num, r: Num) -> Num {
        let half = BigFloat::from_f64(0.5, PRECISION);
        let k = Num(r.0.add(&half, PRECISION, RM).floor());
        let diff = Num(r.0.sub(&k.0, PRECISION, RM).abs());
        if diff < tolerance() {
            if let Ok(p) = self.pow(base, &k) {
                let d = Num(p.0.sub(&x.0, PRECISION, RM).abs());
                let limit = Num(x.0.abs().mul(
                    &BigFloat::from_f64(2f64.powi(-240), PRECISION),
                    PRECISION,
                    RM,
                ));
                if d <= limit {
                    return k;
                }
            }
        }
        r
    }

    // ------------------------------------------------------ factorial and co.

    pub fn factorial(&mut self, x: &Num) -> R {
        let x = match self.as_integer(x) {
            Some(n) if !n.is_negative() => n,
            _ => {
                return Err(CalcError::Domain(
                    "factorial needs a non-negative integer".into(),
                ))
            }
        };
        let x = &x;
        let n = self.to_u64(x).ok_or(CalcError::Overflow)?;
        if n > MAX_FACTORIAL {
            return Err(CalcError::Domain(format!(
                "factorial is limited to arguments up to {MAX_FACTORIAL}"
            )));
        }
        let mut acc = BigFloat::from_i32(1, PRECISION);
        for i in 2..=n {
            acc = acc.mul(&BigFloat::from_u64(i, PRECISION), PRECISION, RM);
        }
        chk("factorial", acc)
    }

    /// Reads `n` and `k` for combinatorics: non-negative integers with `k <= n`.
    fn comb_args(&mut self, n: &Num, k: &Num) -> Result<(u64, u64), CalcError> {
        let bad = || CalcError::Domain("n and k must be non-negative integers with k <= n".into());
        let n = self.to_u64(n).ok_or_else(bad)?;
        let k = self.to_u64(k).ok_or_else(bad)?;
        if k > n || n > 1_000_000 {
            return Err(bad());
        }
        Ok((n, k))
    }

    /// Number of ordered selections of `k` items from `n`.
    pub fn npr(&mut self, n: &Num, k: &Num) -> R {
        let (n, k) = self.comb_args(n, k)?;
        let mut acc = BigFloat::from_i32(1, PRECISION);
        for i in 0..k {
            acc = acc.mul(&BigFloat::from_u64(n - i, PRECISION), PRECISION, RM);
        }
        chk("nPr", acc)
    }

    /// Number of unordered selections of `k` items from `n`.
    pub fn ncr(&mut self, n: &Num, k: &Num) -> R {
        let (n, k) = self.comb_args(n, k)?;
        let k = k.min(n - k);
        let mut acc = BigFloat::from_i32(1, PRECISION);
        for i in 1..=k {
            acc = acc.mul(&BigFloat::from_u64(n - k + i, PRECISION), PRECISION, RM);
            acc = acc.div(&BigFloat::from_u64(i, PRECISION), PRECISION, RM);
        }
        chk("nCr", acc)
    }

    // ---------------------------------------------------------------- angles

    fn angle_to_rad(&mut self, x: &Num) -> R {
        // A huge angle that had to be rounded to fit has lost its low digits, so the
        // sine of the stored value says nothing about the sine of the number the
        // user meant (`sin(1e200)`). Refuse instead of returning a confident wrong
        // answer. Exactly representable angles (`sin(1e100)`) are fine at any size.
        if x.0.inexact() && x.abs() > Num::from_f64(2f64.powi(128)) {
            return Err(CalcError::Domain(
                "the angle is too large to evaluate accurately".into(),
            ));
        }
        let (period, half_turn) = match self.angle {
            AngleMode::Radians => return Ok(x.clone()),
            AngleMode::Degrees => (360, 180),
            AngleMode::Gradians => (400, 200),
        };
        // Reducing first keeps huge angles accurate and exact multiples exact.
        let reduced = chk("angle", x.0.rem(&BigFloat::from_i32(period, PRECISION)))?;
        let pi = self.pi();
        let t = self.mul(&reduced, &pi)?;
        self.div(&t, &Num::from_i64(half_turn))
    }

    fn rad_to_angle(&mut self, r: &Num) -> R {
        let half_turn = match self.angle {
            AngleMode::Radians => return Ok(r.clone()),
            AngleMode::Degrees => 180,
            AngleMode::Gradians => 200,
        };
        let pi = self.pi();
        let t = self.mul(r, &Num::from_i64(half_turn))?;
        self.div(&t, &pi)
    }

    /// Flushes rounding residue (~1e-77) to an exact zero, but only where the
    /// true value can be zero: far from the origin, at multiples of pi/2.
    fn snap_zero(&self, r: Num, arg: &Num) -> Num {
        if arg.abs() > Num::one() && r.abs() < tolerance() {
            Num::zero()
        } else {
            r
        }
    }

    fn sin_rad(&mut self, rad: &Num) -> R {
        let r = chk("sin", rad.0.sin(PRECISION, RM, &mut self.cc))?;
        Ok(self.snap_zero(r, rad))
    }

    fn cos_rad(&mut self, rad: &Num) -> R {
        let r = chk("cos", rad.0.cos(PRECISION, RM, &mut self.cc))?;
        Ok(self.snap_zero(r, rad))
    }

    fn undefined_trig(name: &str) -> CalcError {
        CalcError::Undefined(format!("{name} is not defined at this angle"))
    }

    // ----------------------------------------------------------- trigonometry

    pub fn sin(&mut self, x: &Num) -> R {
        let rad = self.angle_to_rad(x)?;
        self.sin_rad(&rad)
    }

    pub fn cos(&mut self, x: &Num) -> R {
        let rad = self.angle_to_rad(x)?;
        self.cos_rad(&rad)
    }

    pub fn tan(&mut self, x: &Num) -> R {
        let rad = self.angle_to_rad(x)?;
        let c = self.cos_rad(&rad)?;
        if c.is_zero() {
            return Err(Self::undefined_trig("tan"));
        }
        let s = self.sin_rad(&rad)?;
        self.div(&s, &c)
    }

    pub fn cot(&mut self, x: &Num) -> R {
        let rad = self.angle_to_rad(x)?;
        let s = self.sin_rad(&rad)?;
        if s.is_zero() {
            return Err(Self::undefined_trig("cot"));
        }
        let c = self.cos_rad(&rad)?;
        self.div(&c, &s)
    }

    pub fn sec(&mut self, x: &Num) -> R {
        let rad = self.angle_to_rad(x)?;
        let c = self.cos_rad(&rad)?;
        if c.is_zero() {
            return Err(Self::undefined_trig("sec"));
        }
        self.div(&Num::one(), &c)
    }

    pub fn csc(&mut self, x: &Num) -> R {
        let rad = self.angle_to_rad(x)?;
        let s = self.sin_rad(&rad)?;
        if s.is_zero() {
            return Err(Self::undefined_trig("csc"));
        }
        self.div(&Num::one(), &s)
    }

    fn check_unit_range(x: &Num, name: &str) -> Result<(), CalcError> {
        if x.abs() > Num::one() {
            Err(CalcError::Domain(format!("{name} needs an argument between -1 and 1")))
        } else {
            Ok(())
        }
    }

    pub fn asin(&mut self, x: &Num) -> R {
        Self::check_unit_range(x, "asin")?;
        let r = chk("asin", x.0.asin(PRECISION, RM, &mut self.cc))?;
        self.rad_to_angle(&r)
    }

    pub fn acos(&mut self, x: &Num) -> R {
        Self::check_unit_range(x, "acos")?;
        let r = chk("acos", x.0.acos(PRECISION, RM, &mut self.cc))?;
        self.rad_to_angle(&r)
    }

    pub fn atan(&mut self, x: &Num) -> R {
        let r = chk("atan", x.0.atan(PRECISION, RM, &mut self.cc))?;
        self.rad_to_angle(&r)
    }

    pub fn asec(&mut self, x: &Num) -> R {
        if x.abs() < Num::one() {
            return Err(CalcError::Domain("asec needs |x| >= 1".into()));
        }
        let inv = self.div(&Num::one(), x)?;
        self.acos(&inv)
    }

    pub fn acsc(&mut self, x: &Num) -> R {
        if x.abs() < Num::one() {
            return Err(CalcError::Domain("acsc needs |x| >= 1".into()));
        }
        let inv = self.div(&Num::one(), x)?;
        self.asin(&inv)
    }

    /// Inverse cotangent with range (0, pi), continuous at 0.
    pub fn acot(&mut self, x: &Num) -> R {
        let pi = self.pi();
        let half_pi = self.div(&pi, &Num::from_i64(2))?;
        let at = chk("acot", x.0.atan(PRECISION, RM, &mut self.cc))?;
        let r = self.sub(&half_pi, &at)?;
        self.rad_to_angle(&r)
    }

    // ------------------------------------------------------------- hyperbolic

    pub fn sinh(&mut self, x: &Num) -> R {
        chk("sinh", x.0.sinh(PRECISION, RM, &mut self.cc))
    }

    pub fn cosh(&mut self, x: &Num) -> R {
        chk("cosh", x.0.cosh(PRECISION, RM, &mut self.cc))
    }

    pub fn tanh(&mut self, x: &Num) -> R {
        chk("tanh", x.0.tanh(PRECISION, RM, &mut self.cc))
    }

    pub fn sech(&mut self, x: &Num) -> R {
        let c = self.cosh(x)?;
        self.div(&Num::one(), &c)
    }

    pub fn csch(&mut self, x: &Num) -> R {
        if x.is_zero() {
            return Err(Self::undefined_trig("csch"));
        }
        let s = self.sinh(x)?;
        self.div(&Num::one(), &s)
    }

    pub fn coth(&mut self, x: &Num) -> R {
        if x.is_zero() {
            return Err(Self::undefined_trig("coth"));
        }
        let t = self.tanh(x)?;
        self.div(&Num::one(), &t)
    }

    pub fn asinh(&mut self, x: &Num) -> R {
        chk("asinh", x.0.asinh(PRECISION, RM, &mut self.cc))
    }

    pub fn acosh(&mut self, x: &Num) -> R {
        if *x < Num::one() {
            return Err(CalcError::Domain("acosh needs x >= 1".into()));
        }
        chk("acosh", x.0.acosh(PRECISION, RM, &mut self.cc))
    }

    pub fn atanh(&mut self, x: &Num) -> R {
        if x.abs() >= Num::one() {
            return Err(CalcError::Domain("atanh needs |x| < 1".into()));
        }
        chk("atanh", x.0.atanh(PRECISION, RM, &mut self.cc))
    }

    pub fn asech(&mut self, x: &Num) -> R {
        if x.is_zero() || x.is_negative() || *x > Num::one() {
            return Err(CalcError::Domain("asech needs 0 < x <= 1".into()));
        }
        let inv = self.div(&Num::one(), x)?;
        self.acosh(&inv)
    }

    pub fn acsch(&mut self, x: &Num) -> R {
        let inv = self.div(&Num::one(), x)?;
        self.asinh(&inv)
    }

    pub fn acoth(&mut self, x: &Num) -> R {
        if x.abs() <= Num::one() {
            return Err(CalcError::Domain("acoth needs |x| > 1".into()));
        }
        let inv = self.div(&Num::one(), x)?;
        self.atanh(&inv)
    }

    // ------------------------------------------------------ degrees/minutes/seconds

    /// Decimal degrees -> `d.mmss` (10.5 becomes 10.3, i.e. 10 deg 30 min 0 s).
    pub fn to_dms(&mut self, x: &Num) -> R {
        let hundred = Num::from_i64(100);
        let sixty = Num::from_i64(60);
        let d = self.trunc(x);
        let rest = self.sub(x, &d)?;
        let m_full = self.mul(&rest, &sixty)?;
        let m = self.trunc(&m_full);
        let s = {
            let frac = self.sub(&m_full, &m)?;
            self.mul(&frac, &sixty)?
        };
        let m_part = self.div(&m, &hundred)?;
        let s_part = self.div(&s, &Num::from_i64(10_000))?;
        let t = self.add(&d, &m_part)?;
        self.add(&t, &s_part)
    }

    /// `d.mmss` -> decimal degrees (10.3 becomes 10.5).
    pub fn from_dms(&mut self, x: &Num) -> R {
        let hundred = Num::from_i64(100);
        let d = self.trunc(x);
        let rest = self.sub(x, &d)?;
        let m_full = self.mul(&rest, &hundred)?;
        let m = self.trunc(&m_full);
        let s = {
            let frac = self.sub(&m_full, &m)?;
            self.mul(&frac, &hundred)?
        };
        let m_part = self.div(&m, &Num::from_i64(60))?;
        let s_part = self.div(&s, &Num::from_i64(3600))?;
        let t = self.add(&d, &m_part)?;
        self.add(&t, &s_part)
    }
}

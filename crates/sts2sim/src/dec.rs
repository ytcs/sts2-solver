use core::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

// Decimal fixed point like the game's C# decimal: f64 mis-truncates (0.7 * 10).
const SCALE: i128 = 1_000_000_000_000;
const SCALE64: i64 = SCALE as i64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Dec(i128);

impl Dec {
    pub const ZERO: Dec = Dec(0);
    pub const ONE: Dec = Dec(SCALE);
    pub const MAX: Dec = Dec(i128::MAX / 4);

    #[inline(always)]
    pub const fn int(v: i64) -> Dec {
        Dec(v as i128 * SCALE)
    }

    pub const fn frac(num: i64, digits: u32) -> Dec {
        let mut d = 1i128;
        let mut i = 0;
        while i < digits {
            d *= 10;
            i += 1;
        }
        Dec(num as i128 * (SCALE / d))
    }

    #[inline(always)]
    pub fn trunc(self) -> i32 {
        if let Ok(x) = i64::try_from(self.0) {
            return (x / SCALE64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
        let v = self.0 / SCALE;
        v.clamp(i32::MIN as i128, i32::MAX as i128) as i32
    }

    #[inline(always)]
    pub fn truncate(self) -> Dec {
        if let Ok(x) = i64::try_from(self.0) {
            return Dec((x / SCALE64 * SCALE64) as i128);
        }
        Dec(self.0 / SCALE * SCALE)
    }

    #[inline(always)]
    pub fn min(self, o: Dec) -> Dec {
        if self < o { self } else { o }
    }
    #[inline(always)]
    pub fn max(self, o: Dec) -> Dec {
        if self > o { self } else { o }
    }
    #[inline(always)]
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }
}

impl From<i32> for Dec {
    #[inline(always)]
    fn from(v: i32) -> Dec {
        Dec(v as i128 * SCALE)
    }
}

impl Add for Dec {
    type Output = Dec;
    #[inline(always)]
    fn add(self, o: Dec) -> Dec {
        Dec(self.0 + o.0)
    }
}
impl Sub for Dec {
    type Output = Dec;
    #[inline(always)]
    fn sub(self, o: Dec) -> Dec {
        Dec(self.0 - o.0)
    }
}
impl Mul for Dec {
    type Output = Dec;
    #[inline(always)]
    fn mul(self, o: Dec) -> Dec {
        Dec(self.0 * o.0 / SCALE)
    }
}
impl Neg for Dec {
    type Output = Dec;
    #[inline(always)]
    fn neg(self) -> Dec {
        Dec(-self.0)
    }
}
impl AddAssign for Dec {
    #[inline(always)]
    fn add_assign(&mut self, o: Dec) {
        self.0 += o.0
    }
}
impl SubAssign for Dec {
    #[inline(always)]
    fn sub_assign(&mut self, o: Dec) {
        self.0 -= o.0
    }
}
impl MulAssign for Dec {
    #[inline(always)]
    fn mul_assign(&mut self, o: Dec) {
        *self = *self * o
    }
}

impl core::fmt::Debug for Dec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.to_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_worked_examples() {
        let v = (Dec::int(6) + Dec::int(3)) * Dec::frac(15, 1) * Dec::frac(75, 2);
        assert_eq!(v.trunc(), 10);
        assert_eq!(v, Dec::frac(10125, 3));
        assert_eq!(Dec::frac(75, 1).trunc(), 7);
        assert_eq!((Dec::frac(7, 1) * Dec::int(10)).trunc(), 7);
        assert_eq!((-Dec::frac(15, 1)).trunc(), -1);
    }
}

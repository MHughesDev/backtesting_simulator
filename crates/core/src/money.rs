use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A monetary value backed by `rust_decimal::Decimal`. Never use floating-point
/// for P&L or order pricing (ADR from spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Money(pub Decimal);

impl Money {
    pub const ZERO: Money = Money(Decimal::ZERO);

    pub fn new(d: Decimal) -> Self {
        Money(d)
    }

    pub fn inner(self) -> Decimal {
        self.0
    }

    pub fn is_negative(self) -> bool {
        self.0.is_sign_negative()
    }

    pub fn is_positive(self) -> bool {
        self.0.is_sign_positive() && !self.0.is_zero()
    }

    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }

    pub fn abs(self) -> Self {
        Money(self.0.abs())
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Decimal> for Money {
    fn from(d: Decimal) -> Self {
        Money(d)
    }
}

impl From<Money> for Decimal {
    fn from(m: Money) -> Self {
        m.0
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 += rhs.0;
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Money) {
        self.0 -= rhs.0;
    }
}

impl Mul<Decimal> for Money {
    type Output = Money;
    fn mul(self, rhs: Decimal) -> Money {
        Money(self.0 * rhs)
    }
}

impl Mul<Money> for Decimal {
    type Output = Money;
    fn mul(self, rhs: Money) -> Money {
        Money(self * rhs.0)
    }
}

impl Div<Decimal> for Money {
    type Output = Money;
    fn div(self, rhs: Decimal) -> Money {
        Money(self.0 / rhs)
    }
}

impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(-self.0)
    }
}

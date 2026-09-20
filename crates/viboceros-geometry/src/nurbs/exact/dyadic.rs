//! Exact power-of-two arithmetic for homogeneous recurrences. No rounding:
//! n * 2^e keeps an odd integer n (or canonical zero). General rational knot
//! ratios or controls decline this specialization and use BigRational instead.
use super::*;
use num_bigint::BigInt;

#[derive(Clone)]
struct Dyadic {
    n: BigInt,
    e: i64,
}

impl Dyadic {
    fn normalized(n: BigInt, e: i64) -> Option<Self> {
        let Some(zeros) = n.trailing_zeros() else {
            return Some(Self { n, e: 0 });
        };
        Some(Self {
            n: n >> usize::try_from(zeros).ok()?,
            e: e.checked_add(i64::try_from(zeros).ok()?)?,
        })
    }

    fn from_rational(value: &Rational) -> Option<Self> {
        let exponent = value.denom().bits().checked_sub(1)?;
        if value.denom().trailing_zeros()? != exponent {
            return None;
        }
        Self::normalized(value.numer().clone(), -i64::try_from(exponent).ok()?)
    }

    fn rational(self) -> Option<Rational> {
        if self.e >= 0 {
            Some(Rational::from_integer(
                self.n << usize::try_from(self.e).ok()?,
            ))
        } else {
            // n is odd, so the power-of-two denominator is already reduced.
            Some(Rational::new_raw(
                self.n,
                BigInt::one() << usize::try_from(self.e.checked_neg()?).ok()?,
            ))
        }
    }

    fn add(&self, other: &Self, subtract: bool) -> Option<Self> {
        let e = self.e.min(other.e);
        let a = &self.n << usize::try_from(self.e.checked_sub(e)?).ok()?;
        let b = &other.n << usize::try_from(other.e.checked_sub(e)?).ok()?;
        Self::normalized(if subtract { a - b } else { a + b }, e)
    }

    fn multiply(&self, other: &Self) -> Option<Self> {
        Self::normalized(&self.n * &other.n, self.e.checked_add(other.e)?)
    }
}

fn controls<const D: usize>(net: &[Homogeneous<D>]) -> Option<Vec<Vec<Dyadic>>> {
    net.iter()
        .map(|h| h.iter().map(Dyadic::from_rational).collect())
        .collect()
}

fn homogeneous<const D: usize>(h: Vec<Dyadic>) -> Option<Homogeneous<D>> {
    h.into_iter()
        .map(Dyadic::rational)
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
}

pub(super) fn evaluate<const D: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: &Rational,
    net: &[Homogeneous<D>],
) -> Option<Homogeneous<D>> {
    let mut work = controls(net)?;
    let one = Dyadic {
        n: BigInt::one(),
        e: 0,
    };
    for level in 1..=degree {
        for local in (level..=degree).rev() {
            let index = span - degree + local;
            let left = rational(knots[index]);
            let width = rational(knots[index + degree - level + 1]) - &left;
            if width <= Rational::zero() {
                return None;
            }
            let alpha = Dyadic::from_rational(&((parameter - left) / width))?;
            let complement = one.add(&alpha, true)?;
            work[local] = (0..D)
                .map(|i| {
                    work[local - 1][i]
                        .multiply(&complement)?
                        .add(&work[local][i].multiply(&alpha)?, false)
                })
                .collect::<Option<_>>()?;
        }
    }
    homogeneous(work.swap_remove(degree))
}

pub(super) fn derivative_controls<const D: usize>(
    direction: Direction<'_>,
    net: &[Homogeneous<D>],
    width: usize,
    along_u: bool,
) -> Option<Vec<Homogeneous<D>>> {
    let net = controls(net)?;
    let height = net.len() / width;
    let (output_width, output_height) = if along_u {
        (width - 1, height)
    } else {
        (width, height - 1)
    };
    let mut output = Vec::with_capacity(output_width * output_height);
    for j in 0..output_height {
        for i in 0..output_width {
            let index = direction.span - direction.degree + if along_u { i } else { j };
            let interval = rational(direction.knots[index + direction.degree + 1])
                - rational(direction.knots[index + 1]);
            if interval <= Rational::zero() {
                return None;
            }
            let factor = Dyadic::from_rational(
                &(Rational::from_integer(direction.degree.into()) / interval),
            )?;
            let lower = &net[j * width + i];
            let upper = &net[j * width + i + if along_u { 1 } else { width }];
            let h = (0..D)
                .map(|k| upper[k].add(&lower[k], true)?.multiply(&factor))
                .collect::<Option<_>>()?;
            output.push(homogeneous(h)?);
        }
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(actual: Dyadic, expected: Rational) {
        let actual = actual.rational().unwrap();
        // Check canonical form as well as value: new_raw requires reduced terms.
        assert_eq!(actual.numer(), expected.numer());
        assert_eq!(actual.denom(), expected.denom());
    }

    #[test]
    fn exact_dyadic_operations_match_reduced_big_rationals_across_binary64_range() {
        let mut state = 7_u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let bits = if state & (0x7ff << 52) == 0x7ff << 52 {
                state ^ (1 << 52)
            } else {
                state
            };
            Real::from_bits(bits)
        };
        let mut values = vec![
            0.,
            -0.,
            Real::from_bits(1),
            -Real::from_bits(1),
            Real::MIN_POSITIVE,
            Real::MAX,
            -Real::MAX,
            1.,
            -1.,
        ];
        values.extend((0..256).map(|_| next()));
        for pair in values.windows(2) {
            let (a, b) = (rational(pair[0]), rational(pair[1]));
            let (x, y) = (
                Dyadic::from_rational(&a).unwrap(),
                Dyadic::from_rational(&b).unwrap(),
            );
            same(x.clone(), a.clone());
            same(x.add(&y, false).unwrap(), &a + &b);
            same(x.add(&y, true).unwrap(), &a - &b);
            same(x.multiply(&y).unwrap(), &a * &b);
            same(x.add(&x, true).unwrap(), Rational::zero());
        }
        assert!(Dyadic::from_rational(&Rational::new(1.into(), 3.into())).is_none());
    }

    #[test]
    fn dyadic_recurrence_declines_nonbinary_blends_but_not_cancellable_knot_ratios() {
        let controls = vec![[rational(1.)], [rational(-2.)]];
        let knots = [0., 0., 1.5, 1.5];
        assert!(evaluate(&knots, 1, 1, &rational(0.5), &controls).is_none());
        let value = evaluate(&knots, 1, 1, &rational(0.75), &controls).unwrap();
        assert_eq!(value, [rational(-0.5)]);
        let nonbinary = vec![[Rational::new(1.into(), 3.into())], [rational(1.)]];
        assert!(evaluate(&knots, 1, 1, &rational(0.75), &nonbinary).is_none());
    }
}

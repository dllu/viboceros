//! Exact cut coefficients formed from the stored planar endpoint coordinates.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::Zero;
#[derive(Debug, PartialEq)]
pub(super) enum LineCuts {
    None,
    Point([f64; 2]),
    Overlap { first: [f64; 2], second: [f64; 2] },
}
fn cross(a: &[Rational; 2], b: &[Rational; 2]) -> Rational {
    &a[0] * &b[1] - &a[1] * &b[0]
}
fn difference(a: [f64; 2], b: [f64; 2]) -> [Rational; 2] {
    std::array::from_fn(|i| rational(a[i]) - rational(b[i]))
}
fn station(value: &Rational) -> Result<f64, GeometryError> {
    check_scalar(value)?;
    let result = scalar(value)?;
    if (!value.is_zero() && result == 0.) || (*value != rational(1.) && result == 1.) {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Ok(result)
}
pub(super) fn line_line(a: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> Result<LineCuts, GeometryError> {
    let r = difference(a[1], a[0]);
    let s = difference(b[1], b[0]);
    let offset = difference(b[0], a[0]);
    if r.iter().all(Zero::is_zero) || s.iter().all(Zero::is_zero) {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    let determinant = cross(&r, &s);
    check_scalar(&determinant)?;
    let zero = Rational::zero();
    let one = rational(1.);
    if !determinant.is_zero() {
        let t = cross(&offset, &s) / &determinant;
        let u = cross(&offset, &r) / &determinant;
        check_scalar(&t)?;
        check_scalar(&u)?;
        if t < zero || t > one || u < zero || u > one {
            return Ok(LineCuts::None);
        }
        return Ok(LineCuts::Point([station(&t)?, station(&u)?]));
    }
    if !cross(&offset, &r).is_zero() {
        return Ok(LineCuts::None);
    }
    let axis = usize::from(r[0].is_zero());
    let t0 = &offset[axis] / &r[axis];
    let t1 = (rational(b[1][axis]) - rational(a[0][axis])) / &r[axis];
    let lo = t0.clone().min(t1.clone()).max(zero.clone());
    let hi = t0.max(t1).min(one);
    if lo > hi {
        return Ok(LineCuts::None);
    }
    let u0 = (&r[axis] * &lo - &offset[axis]) / &s[axis];
    let u1 = (&r[axis] * &hi - &offset[axis]) / &s[axis];
    if lo == hi {
        return Ok(LineCuts::Point([station(&lo)?, station(&u0)?]));
    }
    let first = [station(&lo)?, station(&hi)?];
    let second = [station(&u0)?, station(&u1)?];
    if first[0] == first[1] || second[0] == second[1] {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Ok(LineCuts::Overlap { first, second })
}
fn square_root(value: &Rational) -> Result<f64, GeometryError> {
    if value.is_zero() {
        return Ok(0.);
    }
    // Normalize by an even power so neither the square nor its scale needs
    // to fit binary64. Only the normalized sqrt and final root are rounded.
    let exponent = value.numer().bits() as i64 - value.denom().bits() as i64;
    let even = exponent.div_euclid(2) * 2;
    let power = if even >= 0 {
        Rational::from_integer(num_bigint::BigInt::from(1u8) << even as usize)
    } else {
        Rational::new(
            num_bigint::BigInt::from(1u8),
            num_bigint::BigInt::from(1u8) << (-even) as usize,
        )
    };
    let normalized = scalar(&(value / &power))?.sqrt();
    let result = scalar(
        &(rational(normalized)
            * if even / 2 >= 0 {
                Rational::from_integer(num_bigint::BigInt::from(1u8) << (even / 2) as usize)
            } else {
                Rational::new(
                    num_bigint::BigInt::from(1u8),
                    num_bigint::BigInt::from(1u8) << (-even / 2) as usize,
                )
            }),
    )?;
    if result == 0. {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Ok(result)
}
pub(super) fn line_circle(
    line: [[f64; 2]; 2],
    center: [f64; 2],
    radius: f64,
) -> Result<Vec<f64>, GeometryError> {
    let direction = difference(line[1], line[0]);
    let delta = difference(line[0], center);
    let aa = &direction[0] * &direction[0] + &direction[1] * &direction[1];
    if aa.is_zero() {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    let bb = &delta[0] * &direction[0] + &delta[1] * &direction[1];
    let cc = &delta[0] * &delta[0] + &delta[1] * &delta[1] - rational(radius) * rational(radius);
    let discriminant = &bb * &bb - &aa * &cc;
    for v in [&aa, &bb, &cc, &discriminant] {
        check_scalar(v)?;
    }
    if discriminant < Rational::zero() {
        return Ok(vec![]);
    }
    if discriminant.is_zero() {
        let candidate = -&bb / &aa;
        return if candidate >= Rational::zero() && candidate <= rational(1.) {
            Ok(vec![station(&candidate)?])
        } else {
            Ok(vec![])
        };
    }
    // Factor exact endpoint roots before sqrt rounding can move them across
    // the admitted interval. The remaining root is a rational ratio.
    if cc.is_zero() || (&aa + &bb * rational(2.) + &cc).is_zero() {
        let candidates = if cc.is_zero() {
            vec![Rational::zero(), -&bb * rational(2.) / &aa]
        } else {
            vec![rational(1.), &cc / &aa]
        };
        let mut values = Vec::new();
        for candidate in candidates {
            if candidate >= Rational::zero() && candidate <= rational(1.) {
                let value = station(&candidate)?;
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        values.sort_by(f64::total_cmp);
        return Ok(values);
    }
    let base = -bb / &aa;
    let square = discriminant.clone() / (&aa * &aa);
    check_scalar(&base)?;
    check_scalar(&square)?;
    let root = square_root(&square)?;
    let offset = rational(root);
    // Add away from zero; recover the other root through Vieta's product.
    let far = &base
        + if base < Rational::zero() {
            -offset
        } else {
            offset
        };
    let candidates = if far.is_zero() {
        vec![base]
    } else {
        vec![far.clone(), cc / (&aa * &far)]
    };
    let mut values = Vec::new();
    for candidate in candidates {
        check_scalar(&candidate)?;
        if candidate < Rational::zero() || candidate > rational(1.) {
            continue;
        }
        let value = station(&candidate)?;
        if values.contains(&value) {
            if !discriminant.is_zero() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        } else {
            values.push(value);
        }
    }
    values.sort_by(f64::total_cmp);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_cuts_recover_cancellation_and_full_range_determinants() {
        let n = 2f64.powi(27);
        assert_eq!((n + 1.) * (n - 1.) - n * n, 0.);
        assert_eq!(
            line_line([[0., 0.], [n + 1., n]], [[0.5, 0.5], [n + 0.5, n - 0.5]]).unwrap(),
            LineCuts::Point([0.5, 0.5])
        );
        for exponent in [-550, 0, 600] {
            let scale = 2f64.powi(exponent);
            assert_eq!(
                line_line(
                    [[0., 0.], [scale, 0.]],
                    [[scale * 0.5, -scale * 0.5], [scale * 0.5, scale * 0.5]]
                )
                .unwrap(),
                LineCuts::Point([0.5, 0.5])
            );
        }
    }
    #[test]
    fn line_cuts_keep_overlap_direction_and_endpoint_contacts() {
        assert_eq!(
            line_line([[0., 0.], [4., 0.]], [[3., 0.], [1., 0.]]).unwrap(),
            LineCuts::Overlap {
                first: [0.25, 0.75],
                second: [1., 0.]
            }
        );
        assert_eq!(
            line_line([[0., 0.], [1., 0.]], [[1., 0.], [2., 0.]]).unwrap(),
            LineCuts::Point([1., 0.])
        );
        assert_eq!(
            line_line([[0., 0.], [1., 0.]], [[2., 0.], [3., 0.]]).unwrap(),
            LineCuts::None
        );
        assert_eq!(
            line_line([[0., 0.], [1., 0.]], [[0., 1.], [1., 1.]]).unwrap(),
            LineCuts::None
        );
    }
    #[test]
    fn line_circle_cuts_normalize_before_rounding_squared_coefficients() {
        for exponent in [-550, 0, 600] {
            let scale = 2f64.powi(exponent);
            assert_eq!(
                line_circle([[-2. * scale, 0.], [2. * scale, 0.]], [0., 0.], scale).unwrap(),
                vec![0.25, 0.75]
            );
        }
        assert_eq!(
            line_circle([[-2., 1.], [2., 1.]], [0., 0.], 1.).unwrap(),
            vec![0.5]
        );
        assert_eq!(
            line_circle([[-1., 1.], [2., 1.]], [0., 0.], 1.).unwrap(),
            vec![1. / 3.]
        );
        assert!(
            line_circle([[-2., 2.], [2., 2.]], [0., 0.], 1.)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            line_circle([[-2., 0.], [2., 0.]], [0., 0.], 0.).unwrap(),
            vec![0.5]
        );
    }
    #[test]
    fn small_endpoint_roots_survive_quadratic_cancellation() {
        let small = 2f64.powi(-500);
        let cuts = line_circle([[small, 0.], [2., 0.]], [1., 0.], 1.).unwrap();
        assert_eq!(cuts, vec![1.]);
        let cuts = line_circle([[-small, 0.], [2., 0.]], [1., 0.], 1.).unwrap();
        assert_eq!(cuts[0].to_bits(), (small / 2.).to_bits());
        assert_eq!(cuts[1], 1.);
        let general = line_circle([[-small, 0.], [3., 0.]], [1., 0.], 1.).unwrap();
        assert!(general[0] > 0.);
        assert!((general[0] / (small / 3.) - 1.).abs() < 2e-15);
        assert!((general[1] - 2. / 3.).abs() < 2e-15);
        let enormous = 2f64.powi(600);
        assert_eq!(
            line_circle([[0., 0.], [1., 0.]], [enormous, 0.], enormous).unwrap(),
            vec![0.]
        );
    }
    #[test]
    fn distinct_cuts_that_cannot_fit_the_parameter_domain_are_rejected() {
        let huge = 2f64.powi(600);
        assert!(matches!(
            line_circle([[-huge, 0.], [huge, 0.]], [0., 0.], 1.),
            Err(GeometryError::UnrepresentableBrepBoolean)
        ));
        assert!(matches!(
            station(&(rational(2f64.powi(-1000)) * rational(2f64.powi(-1000)))),
            Err(GeometryError::UnrepresentableBrepBoolean)
        ));
    }
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    fn cut_stations_match_independent_fraction_reference_across_binary_scales() {
        let q: serde_json::Value =
            serde_json::from_str(include_str!("cuts/reference.json")).unwrap();
        assert_eq!(q["cases"].as_array().unwrap().len(), 206);
        for row in q["cases"].as_array().unwrap() {
            let actual = line_line(
                serde_json::from_value(row["a"].clone()).unwrap(),
                serde_json::from_value(row["b"].clone()).unwrap(),
            )
            .unwrap();
            let expected = &row["expected"];
            let first = expected["first"].as_array();
            let second = expected["second"].as_array();
            match actual {
                LineCuts::None => assert_eq!(expected["kind"], "none"),
                LineCuts::Point([a, b]) => {
                    assert_eq!(expected["kind"], "point");
                    assert_eq!(a.to_bits(), first.unwrap()[0].as_f64().unwrap().to_bits());
                    assert_eq!(b.to_bits(), second.unwrap()[0].as_f64().unwrap().to_bits());
                }
                LineCuts::Overlap {
                    first: a,
                    second: b,
                } => {
                    assert_eq!(expected["kind"], "overlap");
                    for i in 0..2 {
                        assert_eq!(
                            a[i].to_bits(),
                            first.unwrap()[i].as_f64().unwrap().to_bits()
                        );
                        assert_eq!(
                            b[i].to_bits(),
                            second.unwrap()[i].as_f64().unwrap().to_bits()
                        );
                    }
                }
            }
        }
    }
}

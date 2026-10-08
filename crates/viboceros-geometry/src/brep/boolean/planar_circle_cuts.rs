//! Circle-pair intersection coefficients from exact stored binary64 scalars.
use super::*;

#[derive(Debug, PartialEq)]
pub(super) enum CircleCuts {
    Coincident,
    None,
    Cross { along: f64, height: f64 },
}

pub(super) fn coefficients(
    distance: f64,
    first_radius: f64,
    second_radius: f64,
    tolerance: Tolerance,
) -> Result<CircleCuts, GeometryError> {
    require_finite(
        [distance, first_radius, second_radius],
        "planar circle cuts",
    )?;
    if distance < 0. || first_radius <= 0. || second_radius <= 0. {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    let d = rational(distance);
    let r = rational(first_radius);
    let s = rational(second_radius);
    let epsilon = rational(tolerance.absolute());
    if d <= epsilon && (&r - &s).abs() <= epsilon {
        return Ok(CircleCuts::Coincident);
    }
    // Preserve the planar region policy: tolerance-band tangencies do not
    // create positive-length intersection intervals.
    if d <= epsilon || d >= &r + &s - &epsilon || d <= (&r - &s).abs() + &epsilon {
        return Ok(CircleCuts::None);
    }
    let along = (&d * &d + &r * &r - &s * &s) / (rational(2.) * &d);
    let height_squared = &r * &r - &along * &along;
    check_scalar(&along)?;
    check_scalar(&height_squared)?;
    if height_squared <= Rational::zero() {
        return Err(GeometryError::UnrepresentableBrepBoolean);
    }
    Ok(CircleCuts::Cross {
        along: scalar(&along)?,
        height: super::planar_cut_scalar::square_root(&height_squared)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_coefficients_match_independent_fraction_decimal_reference() {
        let q: serde_json::Value =
            serde_json::from_str(include_str!("circle_cut_reference.json")).unwrap();
        assert_eq!(q["cases"].as_array().unwrap().len(), 172);
        for row in q["cases"].as_array().unwrap() {
            let d = row["distance"].as_f64().unwrap();
            let tolerance = Tolerance::try_new(d * 1e-12, 1e-12, 1e-9).unwrap();
            let CircleCuts::Cross { along, height } = coefficients(
                d,
                row["first_radius"].as_f64().unwrap(),
                row["second_radius"].as_f64().unwrap(),
                tolerance,
            )
            .unwrap() else {
                panic!()
            };
            assert_eq!(along.to_bits(), row["along"].as_f64().unwrap().to_bits());
            let expected = row["height"].as_f64().unwrap();
            assert!(
                height.to_bits().abs_diff(expected.to_bits()) <= 1,
                "height {height} reference {expected}"
            );
        }
    }
    #[test]
    fn circle_coefficients_keep_crossings_when_squared_terms_leave_binary64() {
        for exponent in [-550, 0, 600] {
            let scale = 2f64.powi(exponent);
            let tolerance = Tolerance::try_new(scale * 1e-7, 1e-12, 1e-9).unwrap();
            let CircleCuts::Cross { along, height } =
                coefficients(6. * scale, 5. * scale, 5. * scale, tolerance).unwrap()
            else {
                panic!()
            };
            assert_eq!(along, 3. * scale);
            assert_eq!(height, 4. * scale);
        }
    }
    #[test]
    fn circle_coefficients_preserve_contact_and_coincidence_policy() {
        let t = Tolerance::DEFAULT;
        assert_eq!(coefficients(0., 2., 2., t).unwrap(), CircleCuts::Coincident);
        for (d, r, s) in [
            (0., 2., 1.),
            (1., 2., 1.),
            (3., 2., 1.),
            (4., 2., 1.),
            (0.5, 2., 1.),
        ] {
            assert_eq!(coefficients(d, r, s, t).unwrap(), CircleCuts::None);
        }
        let tolerance = Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap();
        let result = coefficients(
            1. + 2f64.powi(-20),
            2f64.powi(20),
            2f64.powi(20) - 1.,
            tolerance,
        )
        .unwrap();
        let CircleCuts::Cross { along, height } = result else {
            panic!()
        };
        assert!(along.is_finite() && height > 0.);
        let d = 1. + 2f64.powi(-20);
        let r = 2f64.powi(20);
        let s = r - 1.;
        let old_along = ((d / r).powi(2) + 1. - (s / r).powi(2)) / (2. * d / r) * r;
        let old_height = (1. - (old_along / r).powi(2)).sqrt() * r;
        assert!((old_height - height).abs() > 6e-4);
        assert!((height - 1448.1529615364102).abs() < 3e-13);
    }
    #[test]
    fn circle_coefficients_swap_radii_without_changing_height() {
        let t = Tolerance::DEFAULT;
        let CircleCuts::Cross {
            along: a,
            height: h,
        } = coefficients(6., 5., 4., t).unwrap()
        else {
            panic!()
        };
        let CircleCuts::Cross {
            along: b,
            height: k,
        } = coefficients(6., 4., 5., t).unwrap()
        else {
            panic!()
        };
        assert_eq!(a + b, 6.);
        assert_eq!(h, k);
        for (d, r, s) in [
            (-1., 2., 2.),
            (1., 0., 2.),
            (1., 2., -1.),
            (f64::INFINITY, 2., 2.),
        ] {
            assert!(coefficients(d, r, s, t).is_err());
        }
    }
}

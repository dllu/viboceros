//! Exact rejection of unattainable hull bounds, using only ordered comparisons.
use super::*;

/// For same-sign weights and X >= bound throughout the active net, the
/// homogeneous X-bound*W coefficients share a sign. They can sum to zero only
/// if every strictly higher control has zero basis influence. Interpret the
/// nonnegative de Boor blends over booleans: positive iff a positive input has
/// a strictly positive coefficient. No subtraction, division, tolerance, or
/// floating basis evaluation is needed, including at repeated knots/endpoints.
pub(super) fn strictly_above(
    surface: &NurbsSurface,
    spans: [usize; 2],
    parameters: [Real; 2],
    bound: Real,
) -> bool {
    let mut higher = Vec::with_capacity((surface.degree_u + 1) * (surface.degree_v + 1));
    let mut sign = None;
    for j in spans[1] - surface.degree_v..=spans[1] {
        for i in spans[0] - surface.degree_u..=spans[0] {
            let control = surface.control_points[surface.control_index(i, j)];
            let weight = control.weight();
            if weight == 0.
                || control.point().x() < bound
                || sign.is_some_and(|s| s != weight.is_sign_positive())
            {
                return false;
            }
            sign = Some(weight.is_sign_positive());
            higher.push(control.point().x() > bound);
        }
    }
    let rows = higher
        .chunks_mut(surface.degree_u + 1)
        .map(|row| {
            positive_blend(
                &surface.knots_u,
                surface.degree_u,
                spans[0],
                parameters[0],
                row,
            )
        })
        .collect::<Option<Vec<_>>>();
    let Some(mut rows) = rows else {
        return false;
    };
    positive_blend(
        &surface.knots_v,
        surface.degree_v,
        spans[1],
        parameters[1],
        &mut rows,
    )
    .unwrap_or(false)
}

// Input flags correspond to the degree+1 controls of this active span. The
// caller has selected the domain's right-hand span (interior span at its end).
// Guard convexity explicitly; an unsupported interval falls back to rationals.
fn positive_blend(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
    work: &mut [bool],
) -> Option<bool> {
    for level in 1..=degree {
        for local in (level..=degree).rev() {
            let index = span - degree + local;
            let (left, right) = (knots[index], knots[index + degree - level + 1]);
            if !(left < right && left <= parameter && parameter <= right) {
                return None;
            }
            work[local] =
                (parameter < right && work[local - 1]) || (parameter > left && work[local]);
        }
    }
    Some(work[degree])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WeightedPoint3;
    use crate::nurbs::exact::{Direction, rational};
    use num_traits::Signed;

    #[test]
    fn positive_blends_agree_with_exact_rational_basis_including_knot_sides_and_ranges() {
        let mut count = 0;
        for degree in [1, 2, 3, 7] {
            for multiplicity in [0, 1, degree, degree + 1] {
                for scale in [1., 2_f64.powi(-500), 2_f64.powi(500)] {
                    let knots = [
                        vec![-scale; degree + 1],
                        vec![0.; multiplicity],
                        vec![scale; degree + 1],
                    ]
                    .concat();
                    let controls = knots.len() - degree - 1;
                    for parameter in [
                        -scale,
                        -scale / 2.,
                        -scale / 16.,
                        0.,
                        scale / 16.,
                        scale / 2.,
                        scale,
                    ] {
                        let span = checked_span(degree, controls, &knots, parameter).unwrap();
                        for seed in 0..=degree + 2 {
                            let flags = (0..=degree)
                                .map(|i| seed == i || seed == degree + 2)
                                .collect::<Vec<_>>();
                            let exact = Direction {
                                knots: &knots,
                                degree,
                                span,
                                parameter,
                            }
                            .evaluate(
                                flags
                                    .iter()
                                    .map(|b| [rational(if *b { 1. } else { 0. })])
                                    .collect(),
                            )
                            .unwrap()[0]
                                .is_positive();
                            assert_eq!(
                                positive_blend(&knots, degree, span, parameter, &mut flags.clone()),
                                Some(exact)
                            );
                            count += 1;
                        }
                    }
                }
            }
        }
        assert!(count > 1000);
        // Extrapolation does not have nonnegative blend coefficients.
        assert_eq!(
            positive_blend(&[0., 0., 1., 1.], 1, 1, 2., &mut [false, true]),
            None
        );
    }

    #[test]
    fn support_rejection_preserves_exact_contacts_mixed_weight_errors_and_negative_gauges() {
        let (mut rejected, mut poles) = (0, 0);
        for raised in [None, Some((4, 1.)), Some((4, -1.))] {
            for gauge in [1., -2., 2_f64.powi(-500), 2_f64.powi(500)] {
                for mixed in [false, true] {
                    let controls = (0..9)
                        .map(|i| {
                            let x = raised
                                .filter(|(index, _)| *index == i)
                                .map_or(0., |(_, x)| x);
                            let weight = if mixed && i == 4 { -3. * gauge } else { gauge };
                            WeightedPoint3::try_new(
                                Point3::try_new(x, (i % 3) as f64, (i / 3) as f64).unwrap(),
                                weight,
                            )
                            .unwrap()
                        })
                        .collect();
                    let surface = NurbsSurface::try_new_rational(
                        2,
                        2,
                        3,
                        3,
                        controls,
                        vec![0., 0., 0., 1., 1., 1.],
                        vec![0., 0., 0., 1., 1., 1.],
                    )
                    .unwrap();
                    for u in [0., 0.25, 0.5, 1.] {
                        for v in [0., 0.5, 0.75, 1.] {
                            let exact = exact::ExactJetNet::new(&surface, [2, 2])
                                .minimum_x_support_sense([u, v], 0.);
                            let skip = strictly_above(&surface, [2, 2], [u, v], 0.);
                            poles += usize::from(matches!(
                                exact,
                                Err(GeometryError::ZeroWeightAtParameter)
                            ));
                            if skip {
                                assert_eq!(exact, Ok(None));
                                rejected += 1;
                            }
                            if mixed || raised == Some((4, -1.)) {
                                assert!(!skip);
                            }
                            assert_eq!(surface.minimum_x_support_sense_at(u, v, 0.), exact);
                        }
                    }
                }
            }
        }
        assert!(rejected >= 16);
        assert!(poles > 0);
    }
}

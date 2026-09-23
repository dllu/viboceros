//! Exact Bernstein composition for curved UV spans contained in one surface knot rectangle.
use super::{StepError, binomial, join_rational_spans, spline_surface_basis, weighted3};
use monstertruck::step::load::step_geometry::Surface;
use viboceros_geometry::{NurbsCurve, NurbsCurve2, NurbsSurface, Point3, WeightedPoint3};

pub(super) fn compose(
    basis: &Surface,
    uv: &NurbsCurve2,
    id: u64,
) -> Result<Option<NurbsCurve>, StepError> {
    if !matches!(basis, Surface::NurbsSurface(_) | Surface::BsplineSurface(_)) {
        return Ok(None);
    }
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let sign = uv.control_points()[0].weight().is_sign_positive();
    if uv
        .control_points()
        .iter()
        .any(|control| control.weight().is_sign_positive() != sign)
    {
        return Err(unsupported("curved p-curve UV weights change sign"));
    }
    let lifted = NurbsCurve::try_new_rational(
        uv.degree(),
        uv.control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(
                    Point3::try_new(control.point().x(), control.point().y(), 0.)?,
                    control.weight(),
                )
                .map_err(StepError::from)
            })
            .collect::<Result<Vec<_>, _>>()?,
        uv.knots().to_vec(),
    )?;
    let surface = spline_surface_basis(basis, id)?.try_clamped_to_active_domain()?;
    let u_rectangles = surface.spans_u().collect::<Vec<_>>();
    let v_rectangles = surface.spans_v().collect::<Vec<_>>();
    let mut spans = Vec::new();
    for uv_span in lifted.try_bezier_spans()? {
        let bounds = uv_span.control_points().iter().fold(
            [
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ],
            |mut bounds, control| {
                let point = control.point();
                bounds[0] = bounds[0].min(point.x());
                bounds[1] = bounds[1].max(point.x());
                bounds[2] = bounds[2].min(point.y());
                bounds[3] = bounds[3].max(point.y());
                bounds
            },
        );
        // At a path confined to an interior knot line, evaluation uses the
        // following span. Prefer it if both adjacent rectangles contain the
        // complete UV control hull.
        let u = u_rectangles
            .iter()
            .copied()
            .rfind(|(from, to)| *from <= bounds[0] && bounds[1] <= *to);
        let v = v_rectangles
            .iter()
            .copied()
            .rfind(|(from, to)| *from <= bounds[2] && bounds[3] <= *to);
        let (Some((u0, u1)), Some((v0, v1))) = (u, v) else {
            return Err(unsupported(
                "curved p-curve crosses a surface knot within a UV span",
            ));
        };
        let patch = surface.try_trimmed(u0..=u1, v0..=v1)?;
        spans.push(compose_span(&patch, &uv_span, id)?);
    }
    Ok(Some(join_rational_spans(&spans, uv.domain(), id)?))
}

fn compose_span(patch: &NurbsSurface, uv: &NurbsCurve, id: u64) -> Result<NurbsCurve, StepError> {
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let p = patch.degree_u();
    let q = patch.degree_v();
    let r = uv.degree();
    let degree = p
        .checked_add(q)
        .and_then(|degree| degree.checked_mul(r))
        .ok_or_else(|| unsupported("curved p-curve composition degree overflows"))?;
    if degree > 64 {
        return Err(unsupported("curved p-curve composition degree exceeds 64"));
    }
    if patch.control_point_count_u() != p + 1 || patch.control_point_count_v() != q + 1 {
        return Err(unsupported(
            "curved p-curve surface rectangle is not a Bezier patch",
        ));
    }
    let surface_sign = patch.control_points()[0].weight().is_sign_positive();
    if patch
        .control_points()
        .iter()
        .any(|control| control.weight().is_sign_positive() != surface_sign)
    {
        return Err(unsupported("curved p-curve surface weights change sign"));
    }
    let uv_sign = uv.control_points()[0].weight().is_sign_positive();
    if uv
        .control_points()
        .iter()
        .any(|control| control.weight().is_sign_positive() != uv_sign)
    {
        return Err(unsupported("curved p-curve UV weights change sign"));
    }
    let u_domain = patch.domain_u();
    let v_domain = patch.domain_v();
    let u_start = *u_domain.start();
    let u_width = u_domain.end() - u_start;
    let v_start = *v_domain.start();
    let v_width = v_domain.end() - v_start;
    let uv_scale = uv
        .control_points()
        .iter()
        .map(|control| control.weight().abs())
        .fold(0_f64, f64::max);
    let mut u = Vec::with_capacity(r + 1);
    let mut u_bar = Vec::with_capacity(r + 1);
    let mut v = Vec::with_capacity(r + 1);
    let mut v_bar = Vec::with_capacity(r + 1);
    for control in uv.control_points() {
        let point = control.point();
        let local_u = (point.x() - u_start) / u_width;
        let local_v = (point.y() - v_start) / v_width;
        if !u_domain.contains(&point.x())
            || !v_domain.contains(&point.y())
            || !local_u.is_finite()
            || !local_v.is_finite()
        {
            return Err(unsupported(
                "curved p-curve leaves its Bezier surface patch",
            ));
        }
        let weight = control.weight().abs() / uv_scale;
        u.push(local_u * weight);
        u_bar.push((1. - local_u) * weight);
        v.push(local_v * weight);
        v_bar.push((1. - local_v) * weight);
    }
    let up = powers(&u, p);
    let ubp = powers(&u_bar, p);
    let vp = powers(&v, q);
    let vbp = powers(&v_bar, q);
    let u_basis = (0..=p)
        .map(|i| {
            product(&up[i], &ubp[p - i])
                .into_iter()
                .map(|value| value * binomial(p, i))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let v_basis = (0..=q)
        .map(|j| {
            product(&vp[j], &vbp[q - j])
                .into_iter()
                .map(|value| value * binomial(q, j))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let surface_scale = patch
        .control_points()
        .iter()
        .map(|control| control.weight().abs())
        .fold(0_f64, f64::max);
    let mut controls = vec![[0.; 4]; degree + 1];
    for (i, u_polynomial) in u_basis.iter().enumerate() {
        for (j, v_polynomial) in v_basis.iter().enumerate() {
            let control = patch.control_point(i, j).unwrap();
            let weight = control.weight().abs() / surface_scale;
            let point = control.point();
            let homogeneous = [
                point.x() * weight,
                point.y() * weight,
                point.z() * weight,
                weight,
            ];
            for (target, factor) in controls.iter_mut().zip(product(u_polynomial, v_polynomial)) {
                for (coordinate, source) in target.iter_mut().zip(homogeneous) {
                    *coordinate += factor * source;
                }
            }
        }
    }
    let controls = controls
        .into_iter()
        .map(|point| weighted3(point[0], point[1], point[2], point[3], id))
        .collect::<Result<Vec<_>, _>>()?;
    let domain = uv.domain();
    let mut knots = vec![*domain.start(); degree + 1];
    knots.extend(vec![*domain.end(); degree + 1]);
    Ok(NurbsCurve::try_new_rational(degree, controls, knots)?)
}

fn powers(base: &[f64], degree: usize) -> Vec<Vec<f64>> {
    let mut result = vec![vec![1.]];
    for index in 1..=degree {
        result.push(product(&result[index - 1], base));
    }
    result
}

/// Product of two polynomials stored in Bernstein control form.
fn product(a: &[f64], b: &[f64]) -> Vec<f64> {
    let m = a.len() - 1;
    let n = b.len() - 1;
    let mut result = vec![0.; m + n + 1];
    for (i, value_a) in a.iter().enumerate() {
        for (j, value_b) in b.iter().enumerate() {
            result[i + j] +=
                value_a * value_b * binomial(m, i) * binomial(n, j) / binomial(m + n, i + j);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use monstertruck::meshing::prelude::ParametricSurface;
    use monstertruck::modeling::{
        BsplineSurface, KnotVector, NurbsSurface as TruckNurbsSurface, Point3 as TruckPoint3,
        Vector4,
    };
    use viboceros_geometry::{Point2, WeightedPoint2};

    fn uv_curve(degree: usize, controls: &[([f64; 2], f64)], knots: Vec<f64>) -> NurbsCurve2 {
        NurbsCurve2::try_new_rational(
            degree,
            controls
                .iter()
                .map(|(point, weight)| {
                    WeightedPoint2::try_new(Point2::try_new(point[0], point[1]).unwrap(), *weight)
                        .unwrap()
                })
                .collect(),
            knots,
        )
        .unwrap()
    }

    #[test]
    fn curved_uv_bezier_spans_compose_with_polynomial_and_rational_surface_patches() {
        let polynomial = Surface::BsplineSurface(BsplineSurface::new(
            (KnotVector::bezier_knot(1), KnotVector::bezier_knot(1)),
            vec![
                vec![TruckPoint3::new(0., 0., 0.), TruckPoint3::new(0., 1., 0.)],
                vec![TruckPoint3::new(1., 0., 0.), TruckPoint3::new(1., 1., 1.)],
            ],
        ));
        let rational = Surface::NurbsSurface(TruckNurbsSurface::new(BsplineSurface::new(
            (KnotVector::bezier_knot(2), KnotVector::bezier_knot(2)),
            (0..3)
                .map(|u| {
                    (0..3)
                        .map(|v| {
                            let x = u as f64;
                            let y = v as f64;
                            let weight = 1. + 0.2 * x + 0.3 * y;
                            Vector4::new(x * weight, y * weight, x * y * weight, weight)
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
        )));
        for (basis, uv, expected_degree) in [
            (
                polynomial,
                uv_curve(
                    2,
                    &[([0.1, 0.2], 1.), ([0.8, 0.1], 0.5), ([0.9, 0.9], 2.)],
                    vec![5., 5., 5., 9., 9., 9.],
                ),
                4,
            ),
            (
                rational,
                uv_curve(
                    3,
                    &[
                        ([0.1, 0.2], -1.),
                        ([0.2, 0.9], -0.8),
                        ([0.8, 0.1], -1.2),
                        ([0.9, 0.8], -1.),
                    ],
                    vec![5., 5., 5., 5., 9., 9., 9., 9.],
                ),
                12,
            ),
        ] {
            let curve = compose(&basis, &uv, 1).unwrap().unwrap();
            assert_eq!(curve.degree(), expected_degree);
            assert_eq!(curve.domain(), uv.domain());
            for t in [5., 5.17, 6., 7., 8.83, 9.] {
                let point = uv.evaluate(t).unwrap();
                let expected = basis.evaluate(point.x(), point.y());
                let actual = curve.evaluate(t).unwrap();
                assert!((actual.x() - expected.x).abs() < 1e-9);
                assert!((actual.y() - expected.y).abs() < 1e-9);
                assert!((actual.z() - expected.z).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn curved_uv_spans_aligned_with_surface_knots_join_at_original_parameters() {
        let basis = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 0., 0., 0.5, 1., 1., 1.]),
                KnotVector::bezier_knot(1),
            ),
            (0..4)
                .map(|u| {
                    (0..2)
                        .map(|v| TruckPoint3::new(u as f64, v as f64, (u * v) as f64))
                        .collect::<Vec<_>>()
                })
                .collect(),
        ));
        let uv = uv_curve(
            2,
            &[
                ([0.1, 0.1], 1.),
                ([0.2, 0.4], 1.),
                ([0.5, 0.5], 1.),
                ([0.8, 0.7], 1.),
                ([0.9, 0.9], 1.),
            ],
            vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
        );
        let curve = compose(&basis, &uv, 1).unwrap().unwrap();
        assert_eq!(curve.degree(), 6);
        assert_eq!(curve.knots().iter().filter(|knot| **knot == 0.5).count(), 6);
        for t in [0., 0.17, 0.5 - 1e-6, 0.5, 0.5 + 1e-6, 0.83, 1.] {
            let point = uv.evaluate(t).unwrap();
            let expected = basis.evaluate(point.x(), point.y());
            let actual = curve.evaluate(t).unwrap();
            assert!((actual.x() - expected.x).abs() < 1e-9);
            assert!((actual.y() - expected.y).abs() < 1e-9);
            assert!((actual.z() - expected.z).abs() < 1e-9);
        }

        let crossing = uv_curve(
            2,
            &[([0.1, 0.1], 1.), ([0.5, 0.4], 1.), ([0.9, 0.9], 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        );
        assert!(matches!(
            compose(&basis, &crossing, 1),
            Err(StepError::UnsupportedNativeShell {
                reason: "curved p-curve crosses a surface knot within a UV span",
                ..
            })
        ));
    }

    #[test]
    fn curved_uv_path_on_discontinuous_knot_uses_evaluators_following_span() {
        let basis = Surface::BsplineSurface(BsplineSurface::new(
            (
                KnotVector::from(vec![0., 0., 0.5, 0.5, 1., 1.]),
                KnotVector::bezier_knot(1),
            ),
            [0., 1., 10., 11.]
                .into_iter()
                .map(|x| vec![TruckPoint3::new(x, 0., 0.), TruckPoint3::new(x, 1., 0.)])
                .collect(),
        ));
        let uv = uv_curve(
            2,
            &[([0.5, 0.1], 1.), ([0.5, 0.5], 1.), ([0.5, 0.9], 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        );
        let curve = compose(&basis, &uv, 1).unwrap().unwrap();
        for t in [0., 0.17, 0.5, 0.83, 1.] {
            let point = uv.evaluate(t).unwrap();
            let expected = basis.evaluate(point.x(), point.y());
            let actual = curve.evaluate(t).unwrap();
            assert!((actual.x() - expected.x).abs() < 1e-10);
            assert!((actual.y() - expected.y).abs() < 1e-10);
            assert!((actual.z() - expected.z).abs() < 1e-10);
        }
    }
}

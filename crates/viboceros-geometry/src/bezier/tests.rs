use super::*;
use crate::{ControlPointCurveClosure, ParameterSide};

#[test]
fn directional_strips_keep_the_other_complete_knot_vector_and_rational_partials() {
    let s = NurbsSurface::try_new_rational(
        2,
        2,
        4,
        4,
        controls(&[
            1., 0.7, 1.4, 0.8, 1.2, 2., 1., 0.5, 1., 1., 0.7, 1.3, 1., 0.8, 2., 1.,
        ]),
        vec![-3., -2., -1., 1., 4., 5., 6.],
        vec![8., 9., 10., 13., 18., 19., 20.],
    )
    .unwrap();
    for direction in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
        let u = direction == SurfaceKnotDirection::U;
        let strips = s.try_single_span_patches(direction).unwrap();
        assert_eq!(strips.len(), 2);
        for strip in strips {
            assert_eq!(
                if u { strip.knots_v() } else { strip.knots_u() },
                if u { s.knots_v() } else { s.knots_u() }
            );
            assert_eq!(
                (strip.control_point_count_u(), strip.control_point_count_v()),
                if u { (3, 4) } else { (4, 3) }
            );
            let du = strip.domain_u();
            let dv = strip.domain_v();
            for i in 1..16 {
                for j in 1..16 {
                    let a = du.start() + (du.end() - du.start()) * i as f64 / 16.;
                    let b = dv.start() + (dv.end() - dv.start()) * j as f64 / 16.;
                    let (p, x, y) = strip.evaluate_with_derivatives(a, b).unwrap();
                    let (q, xx, yy) = s.evaluate_with_derivatives(a, b).unwrap();
                    assert!(p.distance_to(q).unwrap() < 2e-12);
                    for (a, b) in x
                        .to_array()
                        .into_iter()
                        .chain(y.to_array())
                        .zip(xx.to_array().into_iter().chain(yy.to_array()))
                    {
                        assert!((a - b).abs() < 2e-11);
                    }
                }
            }
        }
    }
}

#[test]
fn directional_extraction_keeps_extreme_independent_control_line_gauges() {
    let c = (0..6)
        .map(|i| {
            WeightedPoint3::try_new(
                Point3::try_new((i % 3) as f64 * 2., (i / 3) as f64, 0.).unwrap(),
                if i < 3 { 1e-280 } else { 1e280 },
            )
            .unwrap()
        })
        .collect();
    let s = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        c,
        vec![-2., -1., 0., 1., 2., 3.],
        vec![10., 10., 18., 18.],
    )
    .unwrap();
    let patches = s.try_single_span_patches(SurfaceKnotDirection::U).unwrap();
    assert_eq!(patches.len(), 1);
    for (i, c) in patches[0].control_points().iter().enumerate() {
        assert_eq!(c.weight(), if i < 3 { 1e-280 } else { 1e280 });
        assert_eq!(
            c.point().to_array(),
            [1. + (i % 3) as f64, (i / 3) as f64, 0.]
        );
    }
}

#[test]
fn periodic_directional_strips_keep_the_original_locus_and_untouched_axis() {
    let profile = NurbsCurve::try_control_point_curve_with_closure(
        3,
        controls(&[1.; 7]).into_iter().map(|c| c.point()).collect(),
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    let surface = NurbsSurface::try_extruded_curve(
        &profile,
        crate::Vector3::try_new(0., 0., 0.).unwrap(),
        crate::Vector3::try_new(0., 0., 5.).unwrap(),
    )
    .unwrap();
    assert!(surface.is_periodic_u());
    for (strip, (a, b)) in surface
        .try_single_span_patches(SurfaceKnotDirection::U)
        .unwrap()
        .iter()
        .zip(surface.spans_u())
    {
        assert!(!strip.is_periodic_u());
        assert_eq!(strip.knots_v(), surface.knots_v());
        for i in 0..=16 {
            let u = a + (b - a) * i as f64 / 16.;
            for v in [0., 2.5, 5.] {
                assert!(
                    strip
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(surface.evaluate(u, v).unwrap())
                        .unwrap()
                        < 2e-12
                );
            }
        }
    }
}

fn controls(weights: &[f64]) -> Vec<WeightedPoint3> {
    weights
        .iter()
        .enumerate()
        .map(|(i, w)| {
            WeightedPoint3::try_new(
                Point3::try_new(i as f64, (i % 3) as f64, (i % 2) as f64).unwrap(),
                *w,
            )
            .unwrap()
        })
        .collect()
}

fn compare_curve(curve: &NurbsCurve) -> Vec<NurbsCurve> {
    let pieces = curve.try_bezier_spans().unwrap();
    assert_eq!(pieces.len(), curve.spans().count());
    for (piece, (a, b)) in pieces.iter().zip(curve.spans()) {
        assert_eq!(piece.domain(), a..=b);
        assert_eq!(piece.degree(), curve.degree());
        assert_eq!(piece.control_points().len(), curve.degree() + 1);
        assert_eq!(piece.knots(), unit_span_knots(curve.degree(), a, b));
        for i in 0..=32 {
            let t = piece.parameter_at(i as f64 / 32.).unwrap();
            let side = if i == 32 {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            let (p, d, dd) = piece.evaluate_with_second_derivative(t).unwrap();
            let (q, e, ee) = curve
                .evaluate_with_second_derivative_on_side(t, side)
                .unwrap();
            assert!(p.distance_to(q).unwrap() < 2e-12, "{t}: {p:?} {q:?}");
            for (x, y) in d
                .to_array()
                .into_iter()
                .chain(dd.to_array())
                .zip(e.to_array().into_iter().chain(ee.to_array()))
            {
                assert!((x - y).abs() < 2e-10, "{t}: {x} {y}");
            }
        }
    }
    pieces
}

#[test]
fn polynomial_and_rational_nonuniform_spans_keep_two_derivatives() {
    for weights in [[1.; 6], [1., 0.7, 1.4, 0.8, 2., 1.]] {
        compare_curve(
            &NurbsCurve::try_new_rational(
                3,
                controls(&weights),
                vec![-2., -2., -2., -2., 0.5, 1., 4., 4., 4., 4.],
            )
            .unwrap(),
        );
    }
}

#[test]
fn single_unclamped_span_is_actually_clamped() {
    let curve = NurbsCurve::try_new_rational(
        3,
        controls(&[1., 0.7, 1.4, 1.]),
        vec![-3., -2., -1., 0., 2., 3., 4., 5.],
    )
    .unwrap();
    let pieces = compare_curve(&curve);
    assert_ne!(pieces[0].control_points(), curve.control_points());
}

#[test]
fn periodic_curve_keeps_seam_and_span_order() {
    let points = controls(&[1.; 7]).into_iter().map(|c| c.point()).collect();
    let curve = NurbsCurve::try_control_point_curve_with_closure(
        3,
        points,
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert!(curve.is_periodic());
    assert!(compare_curve(&curve).iter().all(|p| !p.is_periodic()));
}

#[test]
fn isolated_discontinuous_blocks_keep_their_independent_weight_gauges() {
    let weights = [1e-250, 2e-250, 1e-250, -1e250, -2e250, -1e250];
    let curve = NurbsCurve::try_new_rational(
        2,
        controls(&weights),
        vec![0., 0., 0., 1., 1., 1., 2., 2., 2.],
    )
    .unwrap();
    let pieces = compare_curve(&curve);
    assert_eq!(pieces[0].control_points(), &curve.control_points()[..3]);
    assert_eq!(pieces[1].control_points(), &curve.control_points()[3..]);
}

#[test]
fn extraction_is_invariant_under_extreme_common_weight_scaling() {
    let base = NurbsCurve::try_new_rational(
        3,
        controls(&[1., 0.7, 1.4, 1.]),
        vec![-3., -2., -1., 0., 2., 3., 4., 5.],
    )
    .unwrap();
    let expected = base.try_bezier_spans().unwrap();
    for scale in [1e-280, 1e280, -1e-280, -1e280] {
        let source = NurbsCurve::try_new_rational(
            3,
            base.control_points()
                .iter()
                .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * scale).unwrap())
                .collect(),
            base.knots().to_vec(),
        )
        .unwrap();
        let pieces = compare_curve(&source);
        for (a, b) in pieces[0]
            .control_points()
            .iter()
            .zip(expected[0].control_points())
        {
            assert!(a.point().distance_to(b.point()).unwrap() < 2e-14);
            assert!((a.weight() / scale - b.weight()).abs() < 1e-14);
        }
    }
}

fn compare_surface(surface: &NurbsSurface) -> Vec<NurbsSurface> {
    let patches = surface.try_bezier_patches().unwrap();
    let domains = surface
        .spans_u()
        .flat_map(|u| surface.spans_v().map(move |v| (u, v)));
    for (patch, ((a, b), (c, d))) in patches.iter().zip(domains) {
        assert_eq!(patch.domain_u(), a..=b);
        assert_eq!(patch.domain_v(), c..=d);
        assert_eq!(patch.control_point_count_u(), surface.degree_u() + 1);
        assert_eq!(patch.control_point_count_v(), surface.degree_v() + 1);
        for i in 0..=8 {
            for j in 0..=8 {
                let u = a + (b - a) * i as f64 / 8.;
                let v = c + (d - c) * j as f64 / 8.;
                let (p, du, dv) = patch.evaluate_with_derivatives(u, v).unwrap();
                let (q, eu, ev) = surface
                    .evaluate_with_derivatives_on_sides(
                        u,
                        v,
                        if i == 8 {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                        if j == 8 {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                    )
                    .unwrap();
                assert!(p.distance_to(q).unwrap() < 3e-12);
                for (x, y) in du
                    .to_array()
                    .into_iter()
                    .chain(dv.to_array())
                    .zip(eu.to_array().into_iter().chain(ev.to_array()))
                {
                    assert!((x - y).abs() < 3e-11, "{u} {v}: {x} {y}");
                }
            }
        }
    }
    patches
}

#[test]
fn rational_tensor_product_patches_keep_domain_order_and_derivatives() {
    let surface = NurbsSurface::try_new_rational(
        2,
        2,
        4,
        4,
        controls(&[
            1., 0.7, 1.4, 0.8, 1.2, 2., 1., 0.5, 1., 1., 0.7, 1.3, 1., 0.8, 2., 1.,
        ]),
        vec![-2., -2., -2., 1., 4., 4., 4.],
        vec![10., 10., 10., 13., 18., 18., 18.],
    )
    .unwrap();
    assert_eq!(compare_surface(&surface).len(), 4);
}

#[test]
fn intermediate_zero_weight_surface_controls_are_not_rejected() {
    // U extraction produces a zero-weight control in its first row, but V
    // extraction removes it. The final surface denominator is strictly positive.
    let surface = NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        controls(&[1., -1., 3., 3., 3., 3., 3., 3., 3.]),
        vec![-2., -1., 0., 1., 2., 3.],
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let patches = compare_surface(&surface);
    assert!(patches[0].control_points().iter().all(|c| c.weight() > 0.));
}

#[test]
fn final_zero_weight_control_is_an_explicit_error() {
    let curve =
        NurbsCurve::try_new_rational(2, controls(&[1., -1., 3.]), vec![-2., -1., 0., 1., 2., 3.])
            .unwrap();
    assert_eq!(
        curve.try_bezier_spans(),
        Err(GeometryError::UnrepresentableBezierControl)
    );
}

#[test]
fn translating_large_coordinates_preserves_local_extraction_precision() {
    let curve = NurbsCurve::try_new_rational(
        2,
        controls(&[1., 1., 1.])
            .into_iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_new(1e15 + c.point().x(), c.point().y(), 0.).unwrap(),
                    c.weight(),
                )
                .unwrap()
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let piece = &curve.try_bezier_spans().unwrap()[0];
    assert_eq!(piece.control_points()[0].point().x(), 1e15 + 0.5);
    assert_eq!(piece.control_points()[1].point().x(), 1e15 + 1.);
    assert_eq!(piece.control_points()[2].point().x(), 1e15 + 1.5);
}

#[test]
fn output_and_work_budgets_fail_before_unbounded_allocation() {
    let mut budget = Budget::default();
    assert_eq!(
        budget.output(MAX_BEZIER_CONTROL_POINTS + 1),
        Err(GeometryError::BezierDecompositionLimit)
    );
    assert_eq!(
        Budget::default().charge(usize::MAX),
        Err(GeometryError::BezierDecompositionLimit)
    );
    // An unclamped high-degree span fits the output budget but exceeds work.
    let p = 330;
    let curve = NurbsCurve::try_new_rational(
        p,
        controls(&vec![1.; p + 1]),
        (0..2 * p + 2).map(|i| i as f64).collect(),
    )
    .unwrap();
    assert_eq!(
        curve.try_bezier_spans(),
        Err(GeometryError::BezierDecompositionLimit)
    );
}

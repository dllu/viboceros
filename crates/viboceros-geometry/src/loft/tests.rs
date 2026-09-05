use super::*;
use crate::{Circle3, ParameterSide, Point3, Tolerance, UnitVector3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn circle(radius: Real, z: Real) -> NurbsCurve {
    Circle3::try_new(
        point(0.0, 0.0, z),
        radius,
        UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

fn check_sections(source: &[NurbsCurve], fitted: &NurbsSurface, closed: bool) {
    let prepared = compatible::prepare(source).unwrap();
    let degree = fitted.degree_u();
    for (i, curve) in prepared.iter().enumerate() {
        let u = fitted.knots_u()[degree + i];
        for k in 0..=128 {
            let fraction = k as Real / 128.0;
            for side in [ParameterSide::Left, ParameterSide::Right] {
                let expected = curve
                    .evaluate_on_side(curve.parameter_at(fraction).unwrap(), side)
                    .unwrap();
                let actual = fitted
                    .evaluate_on_sides(
                        u,
                        fitted.parameter_at_v(fraction).unwrap(),
                        ParameterSide::Right,
                        side,
                    )
                    .unwrap();
                assert!(
                    expected.distance_to(actual).unwrap() < 5e-11,
                    "section {i}, sample {k}"
                );
            }
        }
    }
    if closed {
        for i in 0..=16 {
            let v = fitted.parameter_at_v(i as Real / 16.0).unwrap();
            let a = fitted
                .evaluate_with_derivatives(*fitted.domain_u().start(), v)
                .unwrap();
            let b = fitted
                .evaluate_with_derivatives(*fitted.domain_u().end(), v)
                .unwrap();
            assert!(a.0.distance_to(b.0).unwrap() < 1e-12);
            if degree > 1 {
                for (a, b) in a.1.to_array().into_iter().zip(b.1.to_array()) {
                    assert!((a - b).abs() < 1e-12);
                }
            }
        }
    }
}

#[test]
fn interpolating_styles_pass_through_rational_profiles_and_closed_seams() {
    for n in [3, 4, 6] {
        for closed in [false, true] {
            let source = (0..n)
                .map(|i| circle(1.0 + (i as Real).sin() * 0.3, i as Real))
                .collect::<Vec<_>>();
            for style in [
                LoftStyle::Normal,
                LoftStyle::Tight,
                LoftStyle::Uniform,
                LoftStyle::Straight,
            ] {
                let fitted = try_loft_nurbs_curves(&source, style, closed).unwrap();
                check_sections(&source, &fitted, closed);
            }
        }
    }
}

#[test]
fn two_profiles_have_exact_ruled_images_even_when_the_loft_degree_is_cubic() {
    let source = [circle(1.0, 0.0), circle(2.0, 3.0)];
    for style in [
        LoftStyle::Normal,
        LoftStyle::Loose,
        LoftStyle::Tight,
        LoftStyle::Straight,
        LoftStyle::Uniform,
    ] {
        let fitted = try_loft_nurbs_curves(&source, style, false).unwrap();
        for i in 0..=16 {
            for j in 0..=16 {
                let u = i as Real / 16.0;
                let v = j as Real / 16.0;
                let a = source[0]
                    .evaluate(source[0].parameter_at(v).unwrap())
                    .unwrap();
                let expected = point(a.x() * (1.0 + u), a.y() * (1.0 + u), 3.0 * u);
                let actual = fitted
                    .evaluate(
                        fitted.parameter_at_u(u).unwrap(),
                        fitted.parameter_at_v(v).unwrap(),
                    )
                    .unwrap();
                assert!(actual.distance_to(expected).unwrap() < 2e-12);
            }
        }
    }
}

#[test]
fn loose_lofts_keep_compatible_controls_and_a_periodic_cubic_net_for_three_sections() {
    let source = [circle(1.0, 0.0), circle(2.0, 1.0), circle(1.0, 3.0)];
    for closed in [false, true] {
        let fitted = try_loft_nurbs_curves(&source, LoftStyle::Loose, closed).unwrap();
        assert_eq!(fitted.degree_u(), if closed { 3 } else { 2 });
        let prepared = compatible::prepare(&source).unwrap();
        for v in 0..fitted.control_point_count_v() {
            for u in 0..fitted.control_point_count_u() {
                assert_eq!(
                    fitted.control_points()[v * fitted.control_point_count_u() + u],
                    prepared[(u + if closed { 2 } else { 0 }) % 3].control_points()[v]
                );
            }
        }
    }
}

#[test]
fn exact_compatibility_preserves_different_degrees_knots_and_positional_jumps() {
    let source = [
        NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
            vec![-7.0, -7.0, 13.0, 13.0],
        )
        .unwrap(),
        NurbsCurve::try_new_rational(
            2,
            (0..6)
                .map(|i| {
                    WeightedPoint3::try_new(
                        point(i as Real / 5.0, (i as Real).sin(), 2.0),
                        if i % 3 == 1 { 0.7 } else { 1.0 },
                    )
                    .unwrap()
                })
                .collect(),
            vec![0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0],
        )
        .unwrap(),
    ];
    let fitted = try_loft_nurbs_curves(&source, LoftStyle::Normal, false).unwrap();
    let prepared = compatible::prepare(&source).unwrap();
    for (a, b) in source.iter().zip(&prepared) {
        for i in 0..=64 {
            for side in [ParameterSide::Left, ParameterSide::Right] {
                assert!(
                    a.evaluate_on_side(a.parameter_at(i as Real / 64.0).unwrap(), side)
                        .unwrap()
                        .distance_to(b.evaluate_on_side(i as Real / 64.0, side).unwrap())
                        .unwrap()
                        < 2e-12
                );
            }
        }
    }
    // With two sections, the final section is at the last cubic knot, not its first repetition.
    assert_eq!(fitted.degree_v(), 2);
    assert_eq!(
        fitted
            .knots_v()
            .iter()
            .filter(|&&k| k == fitted.parameter_at_v(0.5).unwrap())
            .count(),
        3
    );
}

#[test]
fn independent_common_weight_signs_and_scales_do_not_change_the_loft() {
    let source = [circle(1.0, 0.0), circle(2.0, 1.0), circle(0.5, 3.0)];
    let expected = try_loft_nurbs_curves(&source, LoftStyle::Normal, false).unwrap();
    for scales in [[1e-200, -1e200, 2.0], [1e-320, 1e-320, 1e-320]] {
        let scaled = source
            .iter()
            .zip(scales)
            .map(|(c, scale)| {
                NurbsCurve::try_new_rational(
                    c.degree(),
                    c.control_points()
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * scale).unwrap())
                        .collect(),
                    c.knots().to_vec(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let fitted = try_loft_nurbs_curves(&scaled, LoftStyle::Normal, false).unwrap();
        // Subnormal source weights have already quantized the input geometry.
        let epsilon = if scales[0] < 1e-300 { 2e-3 } else { 2e-11 };
        for i in 0..=16 {
            for j in 0..=16 {
                let a = expected
                    .evaluate(
                        expected.parameter_at_u(i as Real / 16.0).unwrap(),
                        expected.parameter_at_v(j as Real / 16.0).unwrap(),
                    )
                    .unwrap();
                let b = fitted
                    .evaluate(
                        fitted.parameter_at_u(i as Real / 16.0).unwrap(),
                        fitted.parameter_at_v(j as Real / 16.0).unwrap(),
                    )
                    .unwrap();
                assert!(a.distance_to(b).unwrap() < epsilon);
            }
        }
    }
}

#[test]
fn rhino_common_basis_end_weight_policy_is_not_unconditionally_shape_preserving() {
    let first = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.0, 0.0, 0.0), 2.0),
            (point(0.3, 0.5, 0.0), 0.7),
            (point(0.7, -0.5, 0.0), 1.0),
            (point(1.0, 0.0, 0.0), 1.0),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![0.0, 0.0, 0.0, 0.3, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let second = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.0, 0.0, 1.0), 1.0),
            (point(0.3, -0.5, 1.0), 1.0),
            (point(0.7, 1.0, 1.0), 1.0),
            (point(1.0, 0.0, 1.0), 3.0),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![0.0, 0.0, 0.0, 0.7, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let prepared = compatible::prepare(&[first, second.clone()]).unwrap();
    assert_eq!(prepared[0].knots(), prepared[1].knots());
    assert!((prepared[0].knots()[4] - 0.6226295222633872).abs() < 1e-14);
    assert!((prepared[1].control_points()[1].point().x() - 0.1285714285714286).abs() < 1e-14);
    let target = prepared[1].evaluate(0.5).unwrap();
    let closest = second
        .closest_parameter(target, Tolerance::DEFAULT)
        .unwrap();
    let deviation = second
        .evaluate(closest)
        .unwrap()
        .distance_to(target)
        .unwrap();
    assert!(
        deviation > 1e-3,
        "expected documented Rhino profile displacement, got {deviation}"
    );
}

#[test]
fn invalid_section_counts_and_compatibility_limits_fail_before_fitting() {
    let c = circle(1.0, 0.0);
    assert!(matches!(
        try_loft_nurbs_curves(&[], LoftStyle::Normal, false),
        Err(GeometryError::InsufficientLoftSections { .. })
    ));
    assert!(matches!(
        try_loft_nurbs_curves(&[c.clone(), c.clone()], LoftStyle::Normal, true),
        Err(GeometryError::InsufficientLoftSections { .. })
    ));
    assert!(matches!(
        try_loft_nurbs_curves(
            &vec![c.clone(); MAX_LOFT_SECTIONS + 1],
            LoftStyle::Normal,
            false
        ),
        Err(GeometryError::LoftResourceLimit { .. })
    ));
    let huge = NurbsCurve::try_clamped_uniform(
        1,
        (0..=MAX_LOFT_SECTION_CONTROLS)
            .map(|i| point(i as Real, 0.0, 1.0))
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        try_loft_nurbs_curves(&[c, huge], LoftStyle::Normal, false),
        Err(GeometryError::LoftResourceLimit { .. })
    ));
}

#[test]
fn equal_weight_section_metrics_preserve_small_offsets_at_large_world_coordinates() {
    let source = [circle(1.0, 0.0), circle(2.0, 1.0), circle(0.5, 3.0)];
    let offset = [1e12, -2e12, 3e12];
    let moved = source
        .iter()
        .map(|c| {
            NurbsCurve::try_new_rational(
                c.degree(),
                c.control_points()
                    .iter()
                    .map(|c| {
                        WeightedPoint3::try_new(
                            Point3::try_from(std::array::from_fn(|i| {
                                c.point().to_array()[i] + offset[i]
                            }))
                            .unwrap(),
                            c.weight(),
                        )
                        .unwrap()
                    })
                    .collect(),
                c.knots().to_vec(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for style in [LoftStyle::Normal, LoftStyle::Tight, LoftStyle::Uniform] {
        let expected = try_loft_nurbs_curves(&source, style, false).unwrap();
        let actual = try_loft_nurbs_curves(&moved, style, false).unwrap();
        for (a, b) in actual
            .control_points()
            .iter()
            .zip(expected.control_points())
        {
            for (i, (a, b)) in a
                .point()
                .to_array()
                .into_iter()
                .zip(b.point().to_array())
                .enumerate()
            {
                assert!((a - (b + offset[i])).abs() <= 0.0005); // At most about two world-coordinate ulps.
            }
            assert!((a.weight() - b.weight()).abs() < 1e-14);
        }
    }
}

use super::*;
use crate::{Ellipse3, Frame3, Vector3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn patch(offset: Real) -> NurbsSurface {
    let controls = (0..3)
        .flat_map(|v| {
            (0..3).map(move |u| {
                WeightedPoint3::try_new(
                    point(
                        u as Real * 0.7,
                        offset + v as Real * 0.5,
                        (u * v + v * v) as Real * 0.1,
                    ),
                    1. + (u + v) as Real * 0.05,
                )
                .unwrap()
            })
        })
        .collect();
    NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn near_vector(a: Vector3, b: Vector3) {
    let a = a.to_array();
    let b = b.to_array();
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < 1e-8, "{a:?} != {b:?}");
    }
}
#[test]
fn rational_curved_parents_preserve_full_boundary_jets_with_unequal_bases() {
    let a = patch(0.)
        .try_insert_knot_u(0.23, 1)
        .unwrap()
        .try_insert_knot_v(0.47, 1)
        .unwrap()
        .try_reparameterized(-7.0..=19., 11.0..=83.)
        .unwrap();
    let b = patch(3.)
        .try_change_degree(3, 3, false)
        .unwrap()
        .try_insert_knot_u(0.67, 2)
        .unwrap()
        .try_insert_knot_v(0.73, 1)
        .unwrap()
        .try_reversed_u()
        .unwrap()
        .try_reparameterized(4.0..=5., -100.0..=-20.)
        .unwrap();
    let originals = [a.clone(), b.clone()];
    let options = SurfaceBlendOptions {
        continuity: [SurfaceBlendContinuity::Curvature; 2],
        bulges: [0.3, 1.7],
        reverse_second: None,
    };
    let result = try_blend_nurbs_surfaces(
        &a,
        SurfaceExtensionEdge::North,
        &b,
        SurfaceExtensionEdge::South,
        options,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(result.reversed_second);
    assert_eq!(result.surface.degree_v(), 5);
    assert_eq!(result.surface.degree_u(), 3);
    assert_eq!([a.clone(), b.clone()], originals);
    assert!(result.diagnostics.maximum_second_derivative_relative_error < 1e-11);
    for i in 0..=51 {
        let t = i as Real / 51.;
        let parents = [
            a.evaluate_with_second_derivatives(-7. + 26. * t, 83.)
                .unwrap(),
            b.evaluate_with_second_derivatives(5. - t, -100.).unwrap(),
        ];
        for (end, p) in parents.into_iter().enumerate() {
            let q = result
                .surface
                .evaluate_with_second_derivatives(t, end as Real)
                .unwrap();
            let du = if end == 0 { 26. } else { -1. };
            let dv = if end == 0 { 72. * 0.15 } else { 80. * 0.85 };
            assert!(q.point.distance_to(p.point).unwrap() < 1e-10);
            near_vector(q.derivative_u, p.derivative_u.scaled(du).unwrap());
            near_vector(q.derivative_v, p.derivative_v.scaled(dv).unwrap());
            near_vector(q.derivative_uu, p.derivative_uu.scaled(du * du).unwrap());
            near_vector(q.derivative_uv, p.derivative_uv.scaled(du * dv).unwrap());
            near_vector(q.derivative_vv, p.derivative_vv.scaled(dv * dv).unwrap());
        }
    }
}

#[test]
fn every_natural_edge_orientation_and_mixed_continuity_has_exact_positions() {
    let sides = [
        SurfaceExtensionEdge::West,
        SurfaceExtensionEdge::South,
        SurfaceExtensionEdge::East,
        SurfaceExtensionEdge::North,
    ];
    for first in sides {
        for second in sides {
            for continuity in [
                [SurfaceBlendContinuity::Position; 2],
                [SurfaceBlendContinuity::Tangency; 2],
                [
                    SurfaceBlendContinuity::Curvature,
                    SurfaceBlendContinuity::Position,
                ],
                [
                    SurfaceBlendContinuity::Position,
                    SurfaceBlendContinuity::Curvature,
                ],
                [SurfaceBlendContinuity::Curvature; 2],
            ] {
                let a = patch(0.);
                let b = patch(4.);
                let result = try_blend_nurbs_surfaces(
                    &a,
                    first,
                    &b,
                    second,
                    SurfaceBlendOptions {
                        continuity,
                        reverse_second: Some(true),
                        ..Default::default()
                    },
                    Tolerance::DEFAULT,
                )
                .unwrap();
                let aa = orient(&a, first, true).unwrap();
                let bb = orient(&b, second, false)
                    .unwrap()
                    .try_reversed_u()
                    .unwrap()
                    .try_reparameterized(0.0..=1., 0.0..=1.)
                    .unwrap();
                assert_eq!(
                    result.surface.degree_v(),
                    continuity[0].order() + continuity[1].order() + 1
                );
                for t in [0., 0.11, 0.5, 0.89, 1.] {
                    assert!(
                        result
                            .surface
                            .evaluate(t, 0.)
                            .unwrap()
                            .distance_to(aa.evaluate(t, 1.).unwrap())
                            .unwrap()
                            < 1e-12
                    );
                    assert!(
                        result
                            .surface
                            .evaluate(t, 1.)
                            .unwrap()
                            .distance_to(bb.evaluate(t, 0.).unwrap())
                            .unwrap()
                            < 1e-12
                    );
                }
            }
        }
    }
}

#[test]
fn offset_independently_sized_rational_elliptical_collars_accept_g2() {
    let ellipse = |origin: Point3, radii: [Real; 2], height: Real| {
        let frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = Ellipse3::try_new(
            origin,
            radii[0],
            radii[1],
            frame.x_axis(),
            frame.y_axis(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        NurbsSurface::try_extruded_curve(
            &curve,
            Vector3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., height).unwrap(),
        )
        .unwrap()
    };
    let a = ellipse(point(0., 0., 0.), [3., 2.], 2.);
    let b = ellipse(point(0.3, -0.2, 4.), [1.2, 0.8], 0.7)
        .try_reparameterized(7.0..=1007., -8.0..=-6.)
        .unwrap();
    let result = try_blend_nurbs_surfaces(
        &a,
        SurfaceExtensionEdge::North,
        &b,
        SurfaceExtensionEdge::South,
        SurfaceBlendOptions {
            continuity: [SurfaceBlendContinuity::Curvature; 2],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(result.diagnostics.maximum_position_error < 1e-12);
    assert!(result.diagnostics.maximum_tangent_plane_angle < 1e-12);
    assert!(result.diagnostics.maximum_second_derivative_relative_error < 1e-12);
    for v in [0., 0.1, 0.5, 0.9, 1.] {
        assert!(
            result
                .surface
                .evaluate(0., v)
                .unwrap()
                .distance_to(result.surface.evaluate(1., v).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn singular_boundaries_invalid_bulges_and_nonpositive_weights_fail_explicitly() {
    let a = patch(0.);
    let b = patch(4.);
    for bulge in [0., -1., Real::INFINITY, Real::NAN] {
        assert!(
            try_blend_nurbs_surfaces(
                &a,
                SurfaceExtensionEdge::North,
                &b,
                SurfaceExtensionEdge::South,
                SurfaceBlendOptions {
                    bulges: [bulge, 1.],
                    ..Default::default()
                },
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
    let collapsed = NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(1., 0., 0.),
        point(1., 0., 0.),
        point(0., 0., 0.),
    ])
    .unwrap();
    assert!(
        try_blend_nurbs_surfaces(
            &collapsed,
            SurfaceExtensionEdge::North,
            &b,
            SurfaceExtensionEdge::South,
            Default::default(),
            Tolerance::DEFAULT
        )
        .is_err()
    );
    let result = try_blend_nurbs_surfaces(
        &collapsed,
        SurfaceExtensionEdge::North,
        &b,
        SurfaceExtensionEdge::South,
        SurfaceBlendOptions {
            continuity: [SurfaceBlendContinuity::Position; 2],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(result.diagnostics.singular_boundary_samples > 0);
    let dangerous = SurfaceBlendOptions {
        bulges: [1., 10000.],
        ..Default::default()
    };
    assert!(matches!(
        try_blend_nurbs_surfaces(
            &a,
            SurfaceExtensionEdge::North,
            &b,
            SurfaceExtensionEdge::South,
            dangerous,
            Tolerance::DEFAULT
        ),
        Err(GeometryError::InvalidSurfaceBlend { .. })
    ));
}

#[test]
fn supplied_astra_collar_parents_reproduce_independent_minimum_jerk_transition() {
    // Geometry fixture supplied with the modelling issue report on 2026-10-12:
    // ASTRA-07-V3-Source-and-Validation/c2-shroud/freeform-patches-and-seams.json.
    // It contains independently built parents and the expected transition;
    // this test does not call the generator or share its construction routine.
    let rows: serde_json::Value = serde_json::from_str(include_str!("astra-collars.json")).unwrap();
    let surfaces = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let coordinates = serde_json::from_value::<Vec<[Real; 3]>>(
                s["control_points_v_rows_u_fastest"].clone(),
            )
            .unwrap();
            let weights = serde_json::from_value::<Vec<Real>>(s["weights"].clone()).unwrap();
            NurbsSurface::try_new_rational(
                s["degree_u"].as_u64().unwrap() as usize,
                s["degree_v"].as_u64().unwrap() as usize,
                s["control_count_u"].as_u64().unwrap() as usize,
                s["control_count_v"].as_u64().unwrap() as usize,
                coordinates
                    .into_iter()
                    .zip(weights)
                    .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
                    .collect(),
                serde_json::from_value(s["knots_u"].clone()).unwrap(),
                serde_json::from_value(s["knots_v"].clone()).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let result = try_blend_nurbs_surfaces(
        &surfaces[0],
        SurfaceExtensionEdge::East,
        &surfaces[2],
        SurfaceExtensionEdge::West,
        SurfaceBlendOptions {
            continuity: [SurfaceBlendContinuity::Curvature; 2],
            bulges: [2. * 5.2 / 0.55, 2. * 5.2 / 0.45],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(!result.reversed_second);
    let mut max_position: Real = 0.;
    for i in 0..=40 {
        for j in 0..=40 {
            let t = i as Real / 40.;
            let v = j as Real / 40.;
            let actual = result
                .surface
                .evaluate_with_second_derivatives(t, v)
                .unwrap();
            let expected = surfaces[1]
                .evaluate_with_second_derivatives(v * 5.2, t)
                .unwrap();
            max_position = max_position.max(actual.point.distance_to(expected.point).unwrap());
            near_vector(
                actual.derivative_v,
                expected.derivative_u.scaled(5.2).unwrap(),
            );
            near_vector(
                actual.derivative_vv,
                expected.derivative_uu.scaled(5.2 * 5.2).unwrap(),
            );
            near_vector(
                actual.derivative_uv,
                expected.derivative_uv.scaled(5.2).unwrap(),
            );
        }
    }
    assert!(
        max_position < 1e-12,
        "maximum ASTRA oracle deviation {max_position}"
    );
}

#[test]
fn excessive_edge_basis_is_rejected_before_refinement() {
    let n = MAX_EDGE_CONTROLS + 1;
    let controls = (0..2)
        .flat_map(|v| (0..n).map(move |u| point(u as Real / (n - 1) as Real, v as Real, 0.)))
        .collect();
    let mut knots = vec![0., 0.];
    knots.extend((1..n - 1).map(|i| i as Real / (n - 1) as Real));
    knots.extend([1., 1.]);
    let large = NurbsSurface::try_new(1, 1, n, 2, controls, knots, vec![0., 0., 1., 1.]).unwrap();
    let result = try_blend_nurbs_surfaces(
        &large,
        SurfaceExtensionEdge::North,
        &patch(4.),
        SurfaceExtensionEdge::South,
        Default::default(),
        Tolerance::DEFAULT,
    );
    assert!(matches!(
        result,
        Err(GeometryError::InvalidSurfaceBlend { .. })
    ));
}

#[test]
fn delegated_boundary_interpolation_rejects_bad_controls_and_retains_sources() {
    let parents = [patch(0.), patch(3.)];
    let before = parents.clone();
    for failure in 0..3 {
        let mut interpolate = |input: &SurfaceBlendInterpolation<'_>| {
            let mut net = try_interpolate_surface_blend(input)?;
            match failure {
                0 => net.clear(),
                1 => net[0][3] = 0.,
                2 => net[0][0] += 1.,
                _ => unreachable!(),
            }
            Ok::<_, GeometryError>(net)
        };
        let result = try_blend_nurbs_surfaces_with_operations(
            &parents[0],
            SurfaceExtensionEdge::North,
            &parents[1],
            SurfaceExtensionEdge::South,
            Default::default(),
            Tolerance::DEFAULT,
            &mut SurfaceBlendOperations {
                clamp: &mut portable_clamp,
                elevate: &mut portable_elevate,
                refine: &mut portable_refine,
                interpolate: &mut interpolate,
            },
        );
        assert!(matches!(
            result,
            Err(GeometryError::InvalidSurfaceBlend { .. })
        ));
        assert_eq!(parents, before);
    }
}

#[test]
fn boundary_interpolation_validates_dimensions_and_finite_jets() {
    for count in [0, usize::MAX, MAX_EDGE_CONTROLS + 1, 1] {
        let result = try_interpolate_surface_blend(&SurfaceBlendInterpolation {
            continuity: [SurfaceBlendContinuity::Curvature; 2],
            edge_control_count: count,
            targets: &[],
        });
        assert!(result.is_err());
    }
    let mut targets = [[0., 0., 0., 1.]; 2];
    targets[0][0] = Real::NAN;
    assert!(
        try_interpolate_surface_blend(&SurfaceBlendInterpolation {
            continuity: [SurfaceBlendContinuity::Position; 2],
            edge_control_count: 1,
            targets: &targets,
        })
        .is_err()
    );
}

#![cfg(feature = "native")]
use viboceros_geometry::{
    GeometryError, NurbsSurface, Point3, SurfaceBlendContinuity as G, SurfaceBlendOptions,
    SurfaceBlendResult, SurfaceExtensionEdge as Edge, Tolerance, WeightedPoint3,
    try_blend_nurbs_surfaces,
};
use viboceros_smlib::{Error, blend_surfaces, trim_blend_parent};

fn patch(offset: f64) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            u as f64 * 0.7,
                            offset + v as f64 * 0.5,
                            (u * v + v * v) as f64 * 0.1,
                        )
                        .unwrap(),
                        1. + (u + v) as f64 * 0.05,
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn parents() -> [NurbsSurface; 2] {
    [
        patch(0.)
            .try_insert_knot_u(0.23, 1)
            .unwrap()
            .try_insert_knot_v(0.47, 1)
            .unwrap()
            .try_reparameterized(-7.0..=19., 11.0..=83.)
            .unwrap(),
        patch(3.)
            .try_change_degree(3, 3, false)
            .unwrap()
            .try_insert_knot_u(0.67, 2)
            .unwrap()
            .try_insert_knot_v(0.73, 1)
            .unwrap()
            .try_reversed_u()
            .unwrap()
            .try_reparameterized(4.0..=5., -100.0..=-20.)
            .unwrap(),
    ]
}
fn compare(a: &SurfaceBlendResult, b: &SurfaceBlendResult) {
    let (x, y) = (&a.surface, &b.surface);
    assert_eq!(a.reversed_second, b.reversed_second);
    assert_eq!((x.degree_u(), x.degree_v()), (y.degree_u(), y.degree_v()));
    assert_eq!(x.knots_u(), y.knots_u());
    assert_eq!(x.knots_v(), y.knots_v());
    assert_eq!(x.control_points().len(), y.control_points().len());
    for (a, b) in x.control_points().iter().zip(y.control_points()) {
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-10);
        assert!((a.weight() - b.weight()).abs() < 1e-12);
    }
    for i in 0..=12 {
        for j in 0..=12 {
            assert!(
                x.evaluate(i as f64 / 12., j as f64 / 12.)
                    .unwrap()
                    .distance_to(y.evaluate(i as f64 / 12., j as f64 / 12.).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
    assert!(a.diagnostics.maximum_position_error < 1e-10);
    assert!(a.diagnostics.maximum_first_derivative_relative_error < 1e-9);
    assert!(a.diagnostics.maximum_second_derivative_relative_error < 1e-8);
}

#[test]
fn native_boundary_interpolation_preserves_all_orientations_mixed_jets_and_directions() {
    let parents = parents();
    let before = parents.clone();
    for first in [Edge::West, Edge::South, Edge::East, Edge::North] {
        for second in [Edge::West, Edge::South, Edge::East, Edge::North] {
            for continuity1 in [G::Position, G::Tangency, G::Curvature] {
                for continuity2 in [G::Position, G::Tangency, G::Curvature] {
                    for reverse_second in [None, Some(false), Some(true)] {
                        let options = SurfaceBlendOptions {
                            continuity: [continuity1, continuity2],
                            bulges: [0.3, 1.7],
                            reverse_second,
                        };
                        let a = blend_surfaces(
                            &parents[0],
                            first,
                            &parents[1],
                            second,
                            options,
                            Tolerance::DEFAULT,
                        )
                        .unwrap();
                        let b = try_blend_nurbs_surfaces(
                            &parents[0],
                            first,
                            &parents[1],
                            second,
                            options,
                            Tolerance::DEFAULT,
                        )
                        .unwrap();
                        compare(&a, &b);
                    }
                }
            }
        }
    }
    assert_eq!(parents, before);
}

#[test]
fn native_unclamped_preparation_and_trimming_match_independent_bilinear_limits() {
    // Linear precision gives U Greville controls [-0.5,0.4,1.4,2.5] in this
    // unclamped quadratic basis. Independent geometry is S(u,v)=(u,v,uv/5).
    let make = |offset| {
        NurbsSurface::try_new(
            2,
            1,
            4,
            2,
            (0..2)
                .flat_map(|v| {
                    [-0.5, 0.4, 1.4, 2.5].map(move |u| {
                        Point3::try_new(u, v as f64 + offset, u * (v as f64 + offset) / 5.).unwrap()
                    })
                })
                .collect(),
            vec![-2., -1., 0., 0.8, 2., 3., 4.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let (a, b) = (make(0.), make(2.));
    let options = SurfaceBlendOptions {
        continuity: [G::Curvature; 2],
        bulges: [1., 1.],
        reverse_second: Some(false),
    };
    let output = blend_surfaces(
        &a,
        Edge::North,
        &b,
        Edge::South,
        options,
        Tolerance::DEFAULT,
    )
    .unwrap();
    compare(
        &output,
        &try_blend_nurbs_surfaces(
            &a,
            Edge::North,
            &b,
            Edge::South,
            options,
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    for i in 0..=32 {
        let t = i as f64 / 32.;
        let left = Point3::try_new(2. * t, 1., 2. * t / 5.).unwrap();
        let right = Point3::try_new(2. * t, 2., 4. * t / 5.).unwrap();
        assert!(
            output
                .surface
                .evaluate(t, 0.)
                .unwrap()
                .distance_to(left)
                .unwrap()
                < 1e-12
        );
        assert!(
            output
                .surface
                .evaluate(t, 1.)
                .unwrap()
                .distance_to(right)
                .unwrap()
                < 1e-12
        );
    }
    let trimmed = trim_blend_parent(&a, 1e-9..=1.75, 0.1..=0.8).unwrap();
    for i in 0..=10 {
        for j in 0..=10 {
            let u = 1e-9 + (1.75 - 1e-9) * i as f64 / 10.;
            let v = 0.1 + 0.7 * j as f64 / 10.;
            assert!(
                trimmed
                    .evaluate(u, v)
                    .unwrap()
                    .distance_to(Point3::try_new(u, v, u * v / 5.).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
}

#[test]
fn native_blends_preserve_common_signed_extreme_gauges() {
    let [a, b] = parents();
    let options = SurfaceBlendOptions {
        continuity: [G::Curvature; 2],
        bulges: [0.3, 1.7],
        reverse_second: Some(true),
    };
    let baseline = blend_surfaces(
        &a,
        Edge::North,
        &b,
        Edge::South,
        options,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let scale = |s: &NurbsSurface, g| {
        NurbsSurface::try_new_rational(
            s.degree_u(),
            s.degree_v(),
            s.control_point_count_u(),
            s.control_point_count_v(),
            s.control_points()
                .iter()
                .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * g).unwrap())
                .collect(),
            s.knots_u().to_vec(),
            s.knots_v().to_vec(),
        )
        .unwrap()
    };
    for gauge in [1e280, 1e-280, -1e280, -1e-280] {
        let output = blend_surfaces(
            &scale(&a, gauge),
            Edge::North,
            &scale(&b, gauge),
            Edge::South,
            options,
            Tolerance::DEFAULT,
        )
        .unwrap();
        compare(&output, &baseline);
    }
}

#[test]
fn native_blend_preparation_keeps_the_portable_high_degree_fallback() {
    let a = patch(0.).try_change_degree(18, 2, false).unwrap();
    let b = patch(3.);
    let options = SurfaceBlendOptions {
        continuity: [G::Position; 2],
        reverse_second: Some(false),
        ..Default::default()
    };
    compare(
        &blend_surfaces(
            &a,
            Edge::North,
            &b,
            Edge::South,
            options,
            Tolerance::DEFAULT,
        )
        .unwrap(),
        &try_blend_nurbs_surfaces(
            &a,
            Edge::North,
            &b,
            Edge::South,
            options,
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let before = a.clone();
    assert!(matches!(
        blend_surfaces(
            &a,
            Edge::North,
            &b,
            Edge::South,
            SurfaceBlendOptions {
                bulges: [0., 1.],
                ..options
            },
            Tolerance::DEFAULT
        ),
        Err(Error::Geometry(GeometryError::InvalidSurfaceBlend { .. }))
    ));
    assert_eq!(a, before);
}

#[test]
fn native_prepared_astra_collars_match_independent_transition_and_second_derivatives() {
    let records: serde_json::Value = serde_json::from_str(include_str!(
        "../../viboceros-geometry/src/surface_blend/astra-collars.json"
    ))
    .unwrap();
    let surfaces = records
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let points: Vec<[f64; 3]> =
                serde_json::from_value(s["control_points_v_rows_u_fastest"].clone()).unwrap();
            let weights: Vec<f64> = serde_json::from_value(s["weights"].clone()).unwrap();
            NurbsSurface::try_new_rational(
                s["degree_u"].as_u64().unwrap() as usize,
                s["degree_v"].as_u64().unwrap() as usize,
                s["control_count_u"].as_u64().unwrap() as usize,
                s["control_count_v"].as_u64().unwrap() as usize,
                points
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
    let output = blend_surfaces(
        &surfaces[0],
        Edge::East,
        &surfaces[2],
        Edge::West,
        SurfaceBlendOptions {
            continuity: [G::Curvature; 2],
            bulges: [2. * 5.2 / 0.55, 2. * 5.2 / 0.45],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    for i in 0..=40 {
        for j in 0..=40 {
            let t = i as f64 / 40.;
            let v = j as f64 / 40.;
            let actual = output
                .surface
                .evaluate_with_second_derivatives(t, v)
                .unwrap();
            let expected = surfaces[1]
                .evaluate_with_second_derivatives(v * 5.2, t)
                .unwrap();
            assert!(actual.point.distance_to(expected.point).unwrap() < 1e-12);
            for (a, b, scale) in [
                (actual.derivative_v, expected.derivative_u, 5.2),
                (actual.derivative_vv, expected.derivative_uu, 5.2 * 5.2),
                (actual.derivative_uv, expected.derivative_uv, 5.2),
            ] {
                for (x, y) in a.to_array().into_iter().zip(b.to_array()) {
                    assert!((x - y * scale).abs() < 1e-8);
                }
            }
        }
    }
}

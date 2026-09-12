use super::*;

#[test]
fn uv_normalization_handles_all_origin_corners_and_floating_point_scales() {
    let unit = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
    for radius in [f64::MIN_POSITIVE, 1e-100, 1., 1e100, f64::MAX] {
        for origin in 0..4 {
            let corners = [
                [-radius, -radius],
                [radius, -radius],
                [radius, radius],
                [-radius, radius],
            ];
            let points = (0..4)
                .map(|i| {
                    let p = corners[(i + origin) % 4];
                    Point2::try_new(p[0], p[1]).unwrap()
                })
                .collect::<Vec<_>>();
            let normalization = TrimParameterNormalization::try_from_points(&points)
                .unwrap()
                .unwrap();
            for (i, point) in points.iter().enumerate() {
                let index = (i + origin) % 4;
                let expected = [
                    unit[index][0] - unit[origin][0],
                    unit[index][1] - unit[origin][1],
                ];
                assert_eq!(normalization.normalize(*point).unwrap(), expected);
            }
        }
    }
    assert!(
        TrimParameterNormalization::try_from_points(&[])
            .unwrap()
            .is_none()
    );
    let repeated = vec![Point2::try_new(1e100, -1e100).unwrap(); 4096];
    assert!(
        TrimParameterNormalization::try_from_points(&repeated)
            .unwrap()
            .is_none()
    );
}

#[test]
fn loop_winding_is_independent_of_linear_weights_and_handles_extreme_uv_ranges() {
    for (radius, weights) in [
        (1., [1., 1e12]),
        (1., [1e12, 1.]),
        (1., [-1., -1e12]),
        (1., [-1e12, -1.]),
        (1e100, [1., 1e12]),
        (1e100, [1e12, 1.]),
        (f64::MAX, [1., 1.]),
    ] {
        for reversed in [false, true] {
            let mut points = [
                [-radius, -radius],
                [radius, -radius],
                [radius, radius],
                [-radius, radius],
            ];
            if reversed {
                points.reverse();
            }
            let trims = (0..4)
                .map(|i| {
                    let j = (i + 1) % 4;
                    let controls = [points[i], points[j]]
                        .into_iter()
                        .zip(weights)
                        .map(|(p, w)| {
                            WeightedPoint2::try_new(Point2::try_new(p[0], p[1]).unwrap(), w)
                                .unwrap()
                        })
                        .collect();
                    BrepTrim::try_new(
                        [i, j],
                        Some(i),
                        false,
                        NurbsCurve2::try_new_rational(1, controls, vec![0., 0., 1., 1.]).unwrap(),
                        BrepTrimType::Boundary,
                        SurfaceIso::NotIso,
                        [0.; 2],
                    )
                    .unwrap()
                })
                .collect();
            let face_loop = BrepLoop::try_new(
                if reversed {
                    BrepLoopType::Inner
                } else {
                    BrepLoopType::Outer
                },
                trims,
            )
            .unwrap();
            let area = sampled_loop_signed_area(&face_loop).unwrap();
            let expected = if reversed { -2. } else { 2. };
            assert!(
                (area - expected).abs() < 1e-12,
                "radius={radius}, weights={weights:?}"
            );
        }
    }
}

#[test]
fn linear_loop_winding_shortcut_does_not_hide_a_rational_pole() {
    let mut boundary = square().faces()[0].loops()[0].clone();
    let trim = &mut boundary.trims[0];
    let controls = trim
        .curve
        .control_points()
        .iter()
        .zip([1., -1.])
        .map(|(control, weight)| WeightedPoint2::try_new(control.point(), weight).unwrap())
        .collect();
    trim.curve = NurbsCurve2::try_new_rational(1, controls, vec![0., 0., 1., 1.]).unwrap();
    // Endpoints alone look like an ordinary side, but the midpoint has a pole.
    assert!(trim.curve.start_point().is_ok());
    assert!(trim.curve.end_point().is_ok());
    assert!(trim.curve.evaluate(0.5).is_err());
    assert!(sampled_loop_signed_area(&boundary).is_err());
}

#[test]
fn linear_loop_winding_rejects_poles_between_sample_stations() {
    for weights in [[1., -2.], [-2., 1.], [1e-100, -2e-100], [1e100, -2e100]] {
        let mut boundary = square().faces()[0].loops()[0].clone();
        let trim = &mut boundary.trims[0];
        let controls = trim
            .curve
            .control_points()
            .iter()
            .zip(weights)
            .map(|(control, weight)| WeightedPoint2::try_new(control.point(), weight).unwrap())
            .collect();
        trim.curve = NurbsCurve2::try_new_rational(1, controls, vec![-3., -3., 7., 7.]).unwrap();
        // Its denominator is linear with opposite-sign endpoint values: a
        // zero lies at one third or two thirds, outside the quarter samples.
        for i in 0..=4 {
            assert!(trim.curve.evaluate(-3. + 2.5 * f64::from(i)).is_ok());
        }
        assert!(sampled_loop_signed_area(&boundary).is_err(), "{weights:?}");
    }
}

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn square() -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn rebuild(brep: Brep) -> Result<Brep, GeometryError> {
    Brep::try_new(brep.vertices, brep.edges, brep.faces, Tolerance::DEFAULT)
}

#[test]
fn matching_endpoints_do_not_validate_a_bulging_spatial_edge() {
    let mut brep = square();
    brep.edges[0].curve = NurbsCurve::try_clamped_uniform(
        2,
        vec![p(0.0, 0.0, 0.0), p(0.5, 0.0, 0.5), p(1.0, 0.0, 0.0)],
    )
    .unwrap();
    assert!(rebuild(brep).is_err());
}

#[test]
fn matching_endpoints_do_not_validate_a_bulging_parameter_trim() {
    let mut brep = square();
    let trim = &mut brep.faces[0].loops[0].trims[0];
    trim.curve = NurbsCurve2::try_new(
        2,
        vec![
            Point2::try_new(0.0, 0.0).unwrap(),
            Point2::try_new(0.5, 0.3).unwrap(),
            Point2::try_new(1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    trim.iso = SurfaceIso::NotIso;
    assert!(rebuild(brep).is_err());
}

#[test]
fn edge_excursions_are_checked_in_both_directions() {
    let mut brep = square();
    brep.edges[0].curve = NurbsCurve::try_clamped_uniform(
        1,
        vec![
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
        ],
    )
    .unwrap();
    // Every lifted trim point belongs to this edge, but not conversely.
    assert!(rebuild(brep).is_err());
}

#[test]
fn independent_nonlinear_trim_parameterization_is_valid() {
    let mut brep = square();
    let trim = &mut brep.faces[0].loops[0].trims[0];
    trim.curve = NurbsCurve2::try_new(
        2,
        vec![
            Point2::try_new(0.0, 0.0).unwrap(),
            Point2::try_new(0.0, 0.0).unwrap(),
            Point2::try_new(1.0, 0.0).unwrap(),
        ],
        vec![-7.0, -7.0, -7.0, 13.0, 13.0, 13.0],
    )
    .unwrap();
    assert!(rebuild(brep.clone()).is_ok());
    // Reverse the shared edge without changing the face-local traversal.
    brep.edges[0].curve = brep.edges[0].curve.reversed().unwrap();
    brep.edges[0].vertices.reverse();
    brep.faces[0].loops[0].trims[0].reversed_3d = true;
    assert!(rebuild(brep).is_ok());
}

#[test]
fn interior_edge_jumps_are_checked_at_exact_knot_limits() {
    let mut brep = square();
    brep.edges[0].curve = NurbsCurve::try_new(
        1,
        vec![
            p(0.0, 0.0, 0.0),
            p(0.4, 0.0, 0.0),
            p(0.42, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.4, 0.4, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        rebuild(brep),
        Err(GeometryError::InvalidBrepTopology {
            context: "a B-rep boundary curve contains a positional jump"
        })
    ));
}

#[test]
fn edge_jump_tolerance_uses_euclidean_distance_not_a_coordinate_box() {
    let mut brep = square();
    brep.edges[0].curve = NurbsCurve::try_new(
        1,
        vec![
            p(0.0, 0.0, 0.0),
            p(0.4, 0.0, 0.0),
            p(0.4 + 8e-10, 8e-10, 8e-10),
            p(1.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.4, 0.4, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        rebuild(brep),
        Err(GeometryError::InvalidBrepTopology {
            context: "a B-rep boundary curve contains a positional jump"
        })
    ));
}

#[test]
fn geometric_not_structural_continuity_is_required_at_full_order_knots() {
    let mut brep = square();
    brep.edges[0].curve = NurbsCurve::try_new(
        1,
        vec![
            p(0.0, 0.0, 0.0),
            p(0.4, 0.0, 0.0),
            p(0.4, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.4, 0.4, 1.0, 1.0],
    )
    .unwrap();
    assert!(rebuild(brep).is_ok());
}

#[test]
fn singular_trim_interiors_must_stay_at_the_collapsed_vertex() {
    let frame = Frame3::try_from_normal(
        p(0.0, 0.0, 0.0),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut brep = Brep::try_surface_face(
        NurbsSurface::try_sphere(frame, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let trim = &mut brep.faces[0].loops[0].trims[0];
    assert_eq!(trim.trim_type, BrepTrimType::Singular);
    let start = trim.curve.start_point().unwrap();
    let end = trim.curve.end_point().unwrap();
    trim.curve = NurbsCurve2::try_new(
        2,
        vec![
            start,
            Point2::try_new((start.x() + end.x()) * 0.5, start.y() + 0.25).unwrap(),
            end,
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    trim.iso = SurfaceIso::NotIso;
    assert!(matches!(
        rebuild(brep),
        Err(GeometryError::InvalidBrepTopology {
            context: "a singular trim interior leaves its model-space vertex"
        })
    ));
}

#[test]
fn edge_tolerance_is_model_space_but_trim_tolerance_is_parameter_space() {
    let mut brep = square();
    brep.edges[0].curve = NurbsCurve::try_clamped_uniform(
        2,
        vec![p(0.0, 0.0, 0.0), p(0.5, 0.0, 1e-4), p(1.0, 0.0, 0.0)],
    )
    .unwrap();
    brep.faces[0].loops[0].trims[0].tolerance = [1.0; 2];
    assert!(rebuild(brep.clone()).is_err());
    brep.edges[0].tolerance = 1e-4;
    assert!(rebuild(brep).is_ok());
}

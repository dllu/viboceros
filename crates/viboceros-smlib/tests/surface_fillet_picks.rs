#![cfg(feature = "native")]
use viboceros_geometry::{
    AffineTransform3, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::{SurfaceFilletOptions as Options, SurfaceFilletSide as Side};

fn tol() -> Tolerance {
    Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap()
}
fn plane(points: [[f64; 3]; 4]) -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        points.map(|p| Point3::try_from(p).unwrap()).to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn parents() -> [NurbsSurface; 2] {
    [
        plane([
            [-10., -10., 0.],
            [10., -10., 0.],
            [-10., 10., 0.],
            [10., 10., 0.],
        ]),
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            [-5., 5.]
                .into_iter()
                .flat_map(|z| {
                    (0..3).map(move |i| {
                        WeightedPoint3::try_new(
                            Point3::try_new(
                                if i < 2 { 5. } else { 0. },
                                if i > 0 { 5. } else { 0. },
                                z,
                            )
                            .unwrap(),
                            if i == 1 {
                                std::f64::consts::FRAC_1_SQRT_2
                            } else {
                                1.
                            },
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
    ]
}
fn side(positive: bool) -> Side {
    if positive {
        Side::Positive
    } else {
        Side::Negative
    }
}

#[test]
fn four_plane_quadrants_choose_independent_normal_sides() {
    let p = [
        plane([[0., 0., 0.], [10., 0., 0.], [0., 10., 0.], [10., 10., 0.]]),
        plane([[5., 0., -5.], [5., 0., 5.], [5., 10., -5.], [5., 10., 5.]]),
    ];
    let before = p.clone();
    for x in [0.2, 0.8] {
        for z in [0.2, 0.8] {
            let o = Options::try_from_picks(&p[0], &p[1], [[x, 0.5], [z, 0.5]], 0.75, false, tol())
                .unwrap();
            assert_eq!(
                o,
                Options {
                    radius: 0.75,
                    extend: false,
                    sides: [side(z > 0.5), side(x < 0.5)]
                }
            );
            let reversed =
                Options::try_from_picks(&p[1], &p[0], [[z, 0.5], [x, 0.5]], 0.75, false, tol())
                    .unwrap();
            assert_eq!(reversed.sides, [o.sides[1], o.sides[0]]);
        }
    }
    assert_eq!(p, before);
}

#[test]
fn curved_surface_picks_follow_bounded_projection_under_uv_edits() {
    for sense in 0..4 {
        let mut p = parents();
        p[1] = match sense {
            1 => p[1].try_reversed_u().unwrap(),
            2 => p[1].try_reversed_v().unwrap(),
            3 => p[1].try_swapped_uv().unwrap(),
            _ => p[1].clone(),
        };
        p[1] = p[1].try_reparameterized(0. ..=1., 0. ..=1.).unwrap();
        let before = p.clone();
        for (xy, outside) in [([2., 2.], false), ([6., 0.], true), ([-6., 0.], false)] {
            for z in [0.2, 0.8] {
                let mut uv = [0.5, z];
                if sense == 2 {
                    uv[1] = 1. - uv[1];
                }
                if sense == 3 {
                    uv.reverse();
                }
                let o = Options::try_from_picks(
                    &p[0],
                    &p[1],
                    [[(xy[0] + 10.) / 20., (xy[1] + 10.) / 20.], uv],
                    0.75,
                    false,
                    tol(),
                )
                .unwrap();
                assert_eq!(
                    o.sides,
                    [side(z > 0.5), side(outside ^ (sense != 0))],
                    "sense {sense}, {xy:?}, z {z}"
                );
            }
        }
        assert_eq!(p, before);
    }
}

#[test]
fn reflected_sources_flip_both_normal_offsets_and_keep_the_physical_fillet() {
    let transform = AffineTransform3::try_new(
        [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let p = parents()
        .each_ref()
        .map(|s| s.transformed(transform).unwrap());
    let before = p.clone();
    let options =
        Options::try_from_picks(&p[0], &p[1], [[0.9, 0.6], [0.5, 0.8]], 0.75, false, tol())
            .unwrap();
    assert_eq!(options.sides, [Side::Negative; 2]);
    let fillet = viboceros_smlib::fillet_surfaces(&p[0], &p[1], options, tol())
        .unwrap()
        .unwrap();
    for face in fillet.faces() {
        let s = face.surface();
        for i in 0..=8 {
            for j in 0..=8 {
                let q = s
                    .evaluate(
                        s.parameter_at_u(i as f64 / 8.).unwrap(),
                        s.parameter_at_v(j as f64 / 8.).unwrap(),
                    )
                    .unwrap();
                let error = ((q.x().hypot(q.y()) - 5.75).hypot(q.z() - 0.75) - 0.75).abs();
                assert!(error <= 1e-8, "{q:?}: {error}");
            }
        }
    }
    assert_eq!(p, before);
}

#[test]
fn pick_sides_reuse_conditioned_spatial_frames_original_domains_and_signed_weights() {
    for scale in [1e-40, 1., 1e40] {
        for k in [0.5_f64, 1., 2.] {
            let mut p = parents();
            let c = &p[1];
            let controls = c
                .control_points()
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    WeightedPoint3::try_new(p.point(), p.weight() * k.powi((i % 3) as i32) * -1e280)
                        .unwrap()
                })
                .collect();
            p[1] = NurbsSurface::try_new_rational(
                2,
                1,
                3,
                2,
                controls,
                c.knots_u().to_vec(),
                c.knots_v().to_vec(),
            )
            .unwrap();
            let transform = AffineTransform3::try_new(
                [[0., 0., scale], [scale, 0., 0.], [0., scale, 0.]],
                Vector3::try_new(1000. * scale, -2000. * scale, 3000. * scale).unwrap(),
            )
            .unwrap();
            p = p.each_ref().map(|s| {
                s.transformed(transform)
                    .unwrap()
                    .try_reparameterized(-2. ..=7., 1e12..=1e12 + 10.)
                    .unwrap()
            });
            let before = p.clone();
            let picks = [[6.1, 1e12 + 6.], [-2. + 9. / (k + 1.), 1e12 + 8.]];
            let tolerance = Tolerance::try_new(1e-8 * scale, 1e-12, 1e-8).unwrap();
            let o = Options::try_from_picks(&p[0], &p[1], picks, 0.75 * scale, false, tolerance)
                .unwrap();
            assert_eq!(o.sides, [Side::Positive; 2], "scale {scale}, k {k}");
            assert_eq!(p, before);
        }
    }
}

#[test]
fn invalid_and_near_contact_picks_preserve_sources() {
    let p = parents();
    let before = p.clone();
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(
            Options::try_from_picks(&p[0], &p[1], [[value, 0.5], [0.5, 0.8]], 0.75, false, tol())
                .is_err()
        );
    }
    for radius in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(
            Options::try_from_picks(&p[0], &p[1], [[0.2, 0.5], [0.5, 0.8]], radius, false, tol())
                .is_err()
        );
    }
    assert!(
        Options::try_from_picks(&p[0], &p[1], [[0.75, 0.5], [0.5, 0.5]], 0.75, false, tol())
            .is_err()
    );
    assert_eq!(p, before);

    let below = plane([
        [-1., -1., -1.],
        [1., -1., -1.],
        [-1., 1., -1.],
        [1., 1., -1.],
    ]);
    let cone = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        p[1].control_points()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let q = if i < 3 {
                    Point3::try_new(0., 0., 0.).unwrap()
                } else {
                    c.point()
                };
                WeightedPoint3::try_new(q, c.weight()).unwrap()
            })
            .collect(),
        p[1].knots_u().to_vec(),
        p[1].knots_v().to_vec(),
    )
    .unwrap();
    let saved = [below.clone(), cone.clone()];
    assert!(Options::try_from_picks(&below, &cone, [[0.5; 2]; 2], 1., false, tol()).is_err());
    assert_eq!([below, cone], saved);
}

#[test]
fn general_surface_uses_regular_interior_and_boundary_projection_normals() {
    let mut controls = Vec::new();
    for (j, y) in [-1., 0., 1.].into_iter().enumerate() {
        for (i, x) in [-1., 0., 1.].into_iter().enumerate() {
            let z = [1., -1., 1.][i] + [1., -1., 1.][j];
            controls.push(Point3::try_new(x, y, z).unwrap());
        }
    }
    let graph = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        controls,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let before = graph.clone();
    for x in [0., 3.] {
        let wall = plane([[x, -1., -1.], [x, -1., 1.], [x, 1., -1.], [x, 1., 1.]]);
        let picked =
            Options::try_from_picks(&graph, &wall, [[0.75, 0.5], [0.6, 0.5]], 0.1, false, tol());
        if x == 0. {
            assert_eq!(picked.unwrap().sides, [Side::Positive, Side::Negative]);
        } else {
            assert_eq!(picked.unwrap().sides, [Side::Negative, Side::Positive]);
        }
    }
    assert_eq!(graph, before);
}

#[test]
fn fitted_fillet_normals_keep_off_surface_closest_points_within_model_accuracy() {
    let p = parents();
    let before = p.clone();
    for (pick_z, sign) in [(0.2, -1.), (0.8, 1.)] {
        let options = Options::try_from_picks(
            &p[0],
            &p[1],
            [[0.2, 0.5], [0.5, pick_z]],
            0.75,
            false,
            tol(),
        )
        .unwrap();
        let b = viboceros_smlib::fillet_surfaces(&p[0], &p[1], options, tol())
            .unwrap()
            .unwrap();
        for theta in [
            0.2 * std::f64::consts::FRAC_PI_2,
            0.7 * std::f64::consts::FRAC_PI_2,
        ] {
            for alpha in [0.3_f64, 0.8] {
                let radius = 5.75 - 0.75 * alpha.sin();
                let z = sign * (0.75 - 0.75 * alpha.cos());
                let query = Point3::try_new(radius * theta.cos(), radius * theta.sin(), z).unwrap();
                // Independent metric projection onto the exact torus cross circle.
                // All projected points are inside the quarter-sheet bounds.
                let length = (radius - 4.25).hypot(z - sign * 0.75);
                let expected_radius = 4.25 + 0.75 * (radius - 4.25) / length;
                let expected = Point3::try_new(
                    expected_radius * theta.cos(),
                    expected_radius * theta.sin(),
                    sign * 0.75 + 0.75 * (z - sign * 0.75) / length,
                )
                .unwrap();
                let (face, u, v) = b.closest_face_parameters(query, tol()).unwrap().unwrap();
                let actual = b.faces()[face].surface().evaluate(u, v).unwrap();
                let error = actual.distance_to(expected).unwrap();
                assert!(error <= 1e-8, "{actual:?} versus {expected:?}: {error}");
            }
        }
    }
    assert_eq!(p, before);
}

#[test]
fn construction_reserve_preserves_the_existing_native_accuracy_floor() {
    let p = [
        parents()[0].clone(),
        plane([
            [5., -10., -5.],
            [5., -10., 5.],
            [5., 10., -5.],
            [5., 10., 5.],
        ]),
    ];
    let before = p.clone();
    let options = Options {
        radius: 0.75,
        extend: false,
        ..Options::default()
    };
    // Shared frame scale is sqrt(800), so the first request is barely above
    // the native 1e-12 floor. The second remains outside its supported range.
    let tolerance = Tolerance::try_new(3e-11, 1e-12, 1e-8).unwrap();
    let b = viboceros_smlib::fillet_surfaces(&p[0], &p[1], options, tolerance)
        .unwrap()
        .unwrap();
    for vertex in b.vertices() {
        let q = vertex.point();
        assert!(((q.x() - 4.25).hypot(q.z() - 0.75) - 0.75).abs() <= tolerance.absolute());
    }
    let too_tight = Tolerance::try_new(2e-11, 1e-12, 1e-8).unwrap();
    assert!(viboceros_smlib::fillet_surfaces(&p[0], &p[1], options, too_tight).is_err());
    assert_eq!(p, before);
}

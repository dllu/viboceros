use super::*;

#[test]
fn approximate_surface_closure_cannot_pair_distinct_topological_endpoints() {
    let origin = 1e9;
    for transpose in [false, true] {
        let mut points = [
            [origin, 0., 0.],
            [origin + 1e-6, 0., 0.],
            [origin, 3., 0.],
            [origin + 1e-6, 3., 0.],
        ]
        .map(|p| Point3::try_new(p[0], p[1], p[2]).unwrap())
        .to_vec();
        if transpose {
            points.swap(1, 2);
        }
        let surface = NurbsSurface::try_clamped_uniform(1, 1, 2, 2, points).unwrap();
        // Surface closure currently has a coordinate-scale rounding allowance.
        // It must not override the independently constructed vertex topology.
        let result = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
        assert_eq!((result.vertices.len(), result.edges.len()), (4, 4));
        assert_eq!(result.faces[0].surface, surface);
        assert!(result.edges.iter().all(|e| e.vertices[0] != e.vertices[1]));
        assert!(
            result.faces[0].loops[0]
                .trims
                .iter()
                .all(|t| t.trim_type == BrepTrimType::Boundary)
        );
    }
}

#[test]
fn natural_surface_construction_preserves_seams_poles_and_meshes_after_uv_translation() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (name, surface) in [
        (
            "cylinder",
            NurbsSurface::try_cylinder(frame, 2., 0., 5.).unwrap(),
        ),
        ("cone", NurbsSurface::try_cone(frame, 2., 5.).unwrap()),
        ("sphere", NurbsSurface::try_sphere(frame, 2.).unwrap()),
        ("torus", NurbsSurface::try_torus(frame, 4., 1.).unwrap()),
    ] {
        let surface = surface.try_reparameterized(0.0..=1.0, 0.0..=1.0).unwrap();
        let expected = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
        for offset in [[1e12, -2e12], [-1e12, 2e12]] {
            let shifted = surface
                .try_reparameterized(offset[0]..=offset[0] + 1., offset[1]..=offset[1] + 1.)
                .unwrap();
            // Input construction itself must not have rounded any knot data.
            for (axis, (a, b)) in [surface.knots_u(), surface.knots_v()]
                .into_iter()
                .zip([shifted.knots_u(), shifted.knots_v()])
                .enumerate()
            {
                for (a, b) in a.iter().zip(b) {
                    assert_eq!(*a, *b - offset[axis]);
                }
            }
            let actual = Brep::try_surface_face(shifted.clone(), Tolerance::DEFAULT)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(actual.faces[0].surface(), &shifted);
            assert_eq!(actual.vertices.len(), expected.vertices.len());
            assert_eq!(actual.edges.len(), expected.edges.len());
            assert_eq!(actual.is_solid(), expected.is_solid());
            for (a, b) in actual.faces[0].loops[0]
                .trims
                .iter()
                .zip(&expected.faces[0].loops[0].trims)
            {
                assert_eq!(
                    (a.vertices, a.edge, a.reversed_3d, a.trim_type, a.iso),
                    (b.vertices, b.edge, b.reversed_3d, b.trim_type, b.iso)
                );
                for (a, b) in a
                    .curve
                    .control_points()
                    .iter()
                    .zip(b.curve.control_points())
                {
                    assert_eq!(
                        [a.point().x() - offset[0], a.point().y() - offset[1]],
                        b.point().to_array()
                    );
                }
            }
            let a = actual.tessellate(3, Tolerance::DEFAULT).unwrap();
            let b = expected.tessellate(3, Tolerance::DEFAULT).unwrap();
            assert_eq!(a.faces(), b.faces());
            assert_eq!(a.vertices().len(), b.vertices().len());
            for (a, b) in a.vertices().iter().zip(b.vertices()) {
                assert!(a.distance_to(*b).unwrap() < 2e-12, "{name}: {a:?} != {b:?}");
            }
            for density in [-1, 1, 3] {
                let a = shifted.wireframe_curves(density).unwrap();
                let b = surface.wireframe_curves(density).unwrap();
                let borders_a = shifted.natural_boundary_curve_loops().unwrap();
                let borders_b = surface.natural_boundary_curve_loops().unwrap();
                assert_eq!(
                    borders_a.iter().map(Vec::len).collect::<Vec<_>>(),
                    borders_b.iter().map(Vec::len).collect::<Vec<_>>()
                );
                assert_eq!(a.len(), b.len());
                for (a, b) in a
                    .iter()
                    .chain(borders_a.iter().flatten())
                    .zip(b.iter().chain(borders_b.iter().flatten()))
                {
                    for t in [0., 0.1, 0.3, 0.5, 0.9, 1.] {
                        let a = a.evaluate(a.parameter_at(t).unwrap()).unwrap();
                        let b = b.evaluate(b.parameter_at(t).unwrap()).unwrap();
                        assert!(
                            a.distance_to(b).unwrap() < 2e-12,
                            "{name}, density={density}: {a:?} != {b:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn rectangular_surface_edges_survive_large_native_uv_offsets() {
    let controls = [(0., 0., 0.), (1., 0., 0.), (0., 1., 0.), (1., 1., 1.)];
    for weights in [[1., 1., 1., 1.], [1., 2., 4., 8.], [-1., -2., -4., -8.]] {
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            controls
                .into_iter()
                .zip(weights)
                .map(|((x, y, z), w)| {
                    WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), w).unwrap()
                })
                .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        for offset in [[0., 0.], [1e12, -2e12], [-1e12, 2e12]] {
            let shifted = surface
                .try_reparameterized(offset[0]..=offset[0] + 1., offset[1]..=offset[1] + 1.)
                .unwrap();
            for (u, v) in [(0.0..=1.0, 0.0..=1.0), (0.125..=0.875, 0.25..=0.75)] {
                for reversed in [false, true] {
                    let expected = Brep::try_rectangular_surface_face_with_orientation(
                        surface.clone(),
                        u.clone(),
                        v.clone(),
                        reversed,
                        Tolerance::DEFAULT,
                    )
                    .unwrap();
                    let actual = Brep::try_rectangular_surface_face_with_orientation(
                        shifted.clone(),
                        offset[0] + u.start()..=offset[0] + u.end(),
                        offset[1] + v.start()..=offset[1] + v.end(),
                        reversed,
                        Tolerance::DEFAULT,
                    )
                    .unwrap_or_else(|e| {
                        panic!("offset={offset:?}, weights={weights:?}, error={e:?}")
                    });
                    assert_eq!(actual.faces[0].surface(), &shifted);
                    assert_eq!(actual.faces[0].is_reversed(), reversed);
                    assert_eq!(actual.edges.len(), expected.edges.len());
                    for (a, b) in actual.edges.iter().zip(&expected.edges) {
                        assert_eq!(a.vertices(), b.vertices());
                        for t in [0., 0.1, 0.3, 0.5, 0.9, 1.] {
                            let a = a
                                .curve()
                                .evaluate(a.curve().parameter_at(t).unwrap())
                                .unwrap();
                            let b = b
                                .curve()
                                .evaluate(b.curve().parameter_at(t).unwrap())
                                .unwrap();
                            assert!(a.distance_to(b).unwrap() < 2e-12, "{a:?} != {b:?}");
                        }
                    }
                }
            }
        }
    }
}

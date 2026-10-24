use viboceros_geometry::{AffineTransform3, Brep, Frame3, NurbsCurve, Point3, Tolerance, Vector3};
fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_directions(
        point([0., 0., 0.]),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn profile() -> NurbsCurve {
    viboceros_geometry::Polyline3::try_new(
        vec![
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [0., 0., 0.],
        ]
        .into_iter()
        .map(point)
        .collect(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}
fn rectangle(cap: bool, height: f64) -> Brep {
    Brep::try_planar_profile_extrusion(&profile(), frame(), height, cap, Tolerance::DEFAULT)
        .unwrap()
}
#[test]
fn extrusion_canonical_views_preserve_caps_negative_height_volume_and_archive() {
    for cap in [false, true] {
        for height in [-2., 2.] {
            let source = rectangle(cap, height);
            assert!(source.is_extrusion());
            assert_eq!(source.is_solid(), cap);
            assert_eq!(source.faces().len(), if cap { 6 } else { 4 });
            if cap {
                assert!((source.signed_volume(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-6);
            }
            let archive = source.extrusion().unwrap().archive();
            let restored = Brep::try_from_extrusion_archive(archive, Tolerance::DEFAULT).unwrap();
            assert_eq!(source, restored);
            assert_eq!(source.explicit_brep(), restored.explicit_brep());
            assert!(!source.explicit_brep().is_extrusion());
        }
    }
}
#[test]
fn extrusion_affine_maps_preserve_representation_shape_and_outward_orientation() {
    let source = rectangle(true, 2.);
    for rows in [
        [[2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        [[1., 0.4, 0.7], [0., 1., 0.3], [0., 0., 1.]],
    ] {
        let transform =
            AffineTransform3::try_new(rows, Vector3::try_new(10., -4., 3.).unwrap()).unwrap();
        let expected = source
            .explicit_brep()
            .transformed(transform, Tolerance::DEFAULT)
            .unwrap();
        let transformed = source.transformed(transform, Tolerance::DEFAULT).unwrap();
        assert!(transformed.is_extrusion());
        // A regenerated trimmed cap can have a different conservative control
        // hull. Compare the visible geometry, not its support-plane rectangle.
        let actual_bounds = transformed.tight_bounds(Tolerance::DEFAULT).unwrap();
        let expected_bounds = expected.tight_bounds(Tolerance::DEFAULT).unwrap();
        assert!(
            actual_bounds
                .min()
                .distance_to(expected_bounds.min())
                .unwrap()
                < 1e-6,
            "{rows:?}: {actual_bounds:?} vs {expected_bounds:?}"
        );
        assert!(
            actual_bounds
                .max()
                .distance_to(expected_bounds.max())
                .unwrap()
                < 1e-6
        );
        for vertex in transformed.vertices() {
            assert!(
                expected
                    .vertices()
                    .iter()
                    .any(|p| p.point().distance_to(vertex.point()).unwrap() < 1e-6)
            );
        }
        assert!(
            (transformed.signed_volume(Tolerance::DEFAULT).unwrap()
                - expected.signed_volume(Tolerance::DEFAULT).unwrap())
            .abs()
                < 1e-6
        );
    }
    assert_eq!(source, rectangle(true, 2.));
}
#[test]
fn extrusion_geometry_edits_drop_the_native_form_and_cannot_save_stale_geometry() {
    let source = rectangle(true, 2.);
    let mut swapped = source.clone();
    swapped.try_swap_face_uv(&[0]).unwrap();
    assert!(!swapped.is_extrusion());
    assert_eq!(source.is_solid(), swapped.is_solid());
    let reversed = source.reversed();
    assert!(!reversed.is_extrusion());
    assert!((reversed.signed_volume(Tolerance::DEFAULT).unwrap() + 24.).abs() < 1e-6);
    assert!(source.is_extrusion());
}
#[test]
fn extrusion_rational_circle_profiles_keep_exact_cylindrical_geometry() {
    let circle = viboceros_geometry::Circle3::try_new(
        point([0., 0., 0.]),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let brep = Brep::try_planar_profile_extrusion(
        &circle.to_nurbs().unwrap(),
        frame(),
        3.,
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(brep.is_extrusion());
    assert!(brep.is_solid());
    assert!(
        (brep.signed_volume(Tolerance::DEFAULT).unwrap() - 12. * std::f64::consts::PI).abs() < 1e-6
    );
}
#[test]
fn malformed_extrusion_archives_and_degenerate_construction_leave_sources_unchanged() {
    let source = rectangle(true, 2.);
    let bytes = source.extrusion().unwrap().archive();
    for n in [0, 8, 16, 17, bytes.len() / 2, bytes.len() - 1] {
        assert!(Brep::try_from_extrusion_archive(&bytes[..n], Tolerance::DEFAULT).is_err());
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(Brep::try_from_extrusion_archive(&trailing, Tolerance::DEFAULT).is_err());
    let mut version = bytes.to_vec();
    version[8] = 0;
    assert!(Brep::try_from_extrusion_archive(&version, Tolerance::DEFAULT).is_err());
    for h in [0., f64::NAN, f64::INFINITY] {
        assert!(
            Brep::try_planar_profile_extrusion(&profile(), frame(), h, true, Tolerance::DEFAULT)
                .is_err()
        );
    }
    assert_eq!(source, rectangle(true, 2.));
}

#[test]
fn one_face_extrusion_refinement_becomes_an_explicit_brep_without_losing_knots() {
    let profile = NurbsCurve::try_new(
        1,
        vec![point([0., 0., 0.]), point([4., 0., 0.])],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source =
        Brep::try_planar_profile_extrusion(&profile, frame(), 2., false, Tolerance::DEFAULT)
            .unwrap();
    assert_eq!(source.faces().len(), 1);
    let refined = source
        .try_insert_single_surface_knot(viboceros_geometry::SurfaceKnotDirection::V, 0.5, 1)
        .unwrap();
    assert!(!refined.is_extrusion());
    assert!(source.is_extrusion());
    assert!(
        refined.faces()[0].surface().control_point_count_v()
            > source.faces()[0].surface().control_point_count_v()
    );
}

#[test]
fn extrusion_surface_transfer_retains_metadata_and_rejects_malformed_envelopes() {
    let source = rectangle(true, 2.);
    let data =
        viboceros_opennurbs::extrusion_surface(source.extrusion().unwrap().archive()).unwrap();
    assert_eq!(data.caps, [true, true]);
    assert_eq!(data.path_axis, 1);
    let surface = viboceros_geometry::interop::decode_surface(&data.surface).unwrap();
    assert_eq!(
        [
            surface.control_point_count_u(),
            surface.control_point_count_v()
        ],
        [5, 2]
    );
    for length in [0, 8, 11, 12, data.surface.len() - 1] {
        assert!(viboceros_geometry::interop::decode_surface(&data.surface[..length]).is_err());
    }
    let mut trailing = data.surface.clone();
    trailing.push(0);
    assert!(viboceros_geometry::interop::decode_surface(&trailing).is_err());
    let mut version = data.surface;
    version[8] = 2;
    assert!(viboceros_geometry::interop::decode_surface(&version).is_err());
    assert_eq!(source, rectangle(true, 2.));
}

#[test]
fn large_tilted_cylinder_caps_bound_roundoff_without_changing_geometry() {
    for exponent in [0, 40, 80] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(scale * 1e-7, 1e-12, 1e-9).unwrap();
        let frame = Frame3::try_from_normal(
            point([11. * scale, -4. * scale, 7. * scale]),
            Vector3::try_new(2., 3., 6.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        for cap in [false, true] {
            let source =
                Brep::try_cylinder_extrusion(frame, 2. * scale, [0., 3. * scale], cap, tolerance)
                    .unwrap();
            assert!(source.is_extrusion());
            assert_eq!(source.is_solid(), cap);
            let restored =
                Brep::try_from_extrusion_archive(source.extrusion().unwrap().archive(), tolerance)
                    .unwrap();
            assert_eq!(source, restored);
            let transform = AffineTransform3::from_translation(
                Vector3::try_new(5. * scale, 3. * scale, -2. * scale).unwrap(),
            );
            let moved = source.transformed(transform, tolerance).unwrap();
            assert!(moved.is_extrusion());
            assert_eq!(moved.is_solid(), cap);
            for vertex in moved.vertices() {
                let distance = source
                    .vertices()
                    .iter()
                    .map(|v| {
                        transform
                            .transform_point(v.point())
                            .unwrap()
                            .distance_to(vertex.point())
                            .unwrap()
                    })
                    .fold(f64::INFINITY, f64::min);
                assert!(distance / scale < 1e-12, "scale={exponent} gap={distance}");
            }
        }
    }
}

#[test]
fn rational_extrusion_control_net_caps_survive_weight_scaling_shear_and_close_path_ends() {
    use viboceros_geometry::{Circle3, SubD, SubDFromSurfaceMethod, WeightedPoint3};
    let construction_tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-6).unwrap();
    let profile = Circle3::try_new(
        point([0., 0., 0.]),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        construction_tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let shear = AffineTransform3::try_new(
        [[1., 0.3, 0.2], [0., 1., 0.4], [0., 0., 1.]],
        Vector3::try_new(7., -4., 3.).unwrap(),
    )
    .unwrap();
    for (height, transform) in [
        (2., None),
        (-2., None),
        (1e-8, None),
        (2., Some(shear)),
        (-2., Some(shear)),
    ] {
        let mut reference: Option<SubD> = None;
        for scale in [1., 0.3] {
            let controls = profile
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * scale).unwrap())
                .collect();
            let scaled =
                NurbsCurve::try_new_rational(profile.degree(), controls, profile.knots().to_vec())
                    .unwrap();
            let source = Brep::try_planar_profile_extrusion(
                &scaled,
                frame(),
                height,
                true,
                construction_tolerance,
            )
            .unwrap_or_else(|e| panic!("construct height={height}, scale={scale}: {e}"));
            let source = if let Some(transform) = transform {
                source
                    .transformed(transform, construction_tolerance)
                    .unwrap()
            } else {
                source
            };
            let original = source.clone();
            // The thin case has ends closer than this tolerance. Cap
            // membership must still retain the two distinct boundaries.
            let subd = SubD::try_from_extrusion(
                source.extrusion().unwrap(),
                SubDFromSurfaceMethod::ControlPoints,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap_or_else(|e| {
                panic!("height={height}, transform={transform:?}, scale={scale}: {e}")
            });
            assert_eq!(source, original);
            assert_eq!(subd.control_points().len(), 16);
            assert_eq!(subd.edges().len(), 24);
            assert_eq!(subd.faces().len(), 10);
            assert_eq!(subd.faces().iter().filter(|f| f.len() == 8).count(), 2);
            let mut incidence = std::collections::BTreeMap::<[u32; 2], usize>::new();
            for face in subd.faces() {
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    *incidence.entry([a.min(b), a.max(b)]).or_default() += 1;
                }
            }
            assert!(incidence.values().all(|&n| n == 2));
            if let Some(reference) = &reference {
                assert_eq!(subd.faces(), reference.faces());
                for (actual, expected) in
                    subd.control_points().iter().zip(reference.control_points())
                {
                    assert!(actual.distance_to(*expected).unwrap() < 1e-12);
                }
            } else {
                reference = Some(subd);
            }
        }
    }
}

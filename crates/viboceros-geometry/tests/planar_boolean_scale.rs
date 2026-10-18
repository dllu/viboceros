use viboceros_geometry::{
    Brep, BrepBooleanOperation, Circle3, Frame3, NurbsSurface, Point3, Tolerance, Vector3,
    boolean_planar_breps,
};

#[test]
fn circular_and_mixed_planar_booleans_preserve_area_across_model_scales() {
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut maximum_area_error = 0_f64;
    for exponent in [-20, 0, 20, 40, 80] {
        let scale = 2_f64.powi(exponent);
        let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        for tilted in [false, true] {
            let (origin, normal) = if tilted {
                ([5. * scale, -7. * scale, 11. * scale], [2., 3., 6.])
            } else {
                ([0.; 3], [0., 0., 1.])
            };
            let frame = Frame3::try_from_normal(
                Point3::try_from(origin).unwrap(),
                Vector3::try_from(normal).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            let point = |x, y| frame.point_at([x * scale, y * scale, 0.]).unwrap();
            let disk = |x, radius| {
                let circle =
                    Circle3::try_new(point(x, 0.), radius * scale, frame.z_axis(), tolerance)
                        .unwrap();
                Brep::try_planar_face(&circle.to_nurbs().unwrap(), tolerance).unwrap()
            };
            let strip = Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    point(-0.5, -3.),
                    point(0.5, -3.),
                    point(0.5, 3.),
                    point(-0.5, 3.),
                ])
                .unwrap(),
                tolerance,
            )
            .unwrap();
            let small = disk(3., 2.);
            let large = disk(0., 3.);
            let central = disk(0., 2.);
            // Independent analytic circle-lens and disk-strip areas in unit scale.
            let lens = 4. * (1. / 3_f64).acos() + 9. * (7. / 9_f64).acos() - 4. * 2_f64.sqrt();
            let strip_intersection = 3.75_f64.sqrt() + 8. * 0.25_f64.asin();
            let pi = std::f64::consts::PI;
            for (name, a, b, area_a, area_b, intersection, difference_pieces) in [
                ("circles", &small, &large, 4. * pi, 9. * pi, lens, 1),
                ("reverse-circles", &large, &small, 9. * pi, 4. * pi, lens, 1),
                (
                    "strip",
                    &central,
                    &strip,
                    4. * pi,
                    6.,
                    strip_intersection,
                    2,
                ),
                (
                    "reverse-strip",
                    &strip,
                    &central,
                    6.,
                    4. * pi,
                    strip_intersection,
                    2,
                ),
            ] {
                let before = [a.clone(), b.clone()];
                for (operation, expected, count) in [
                    (
                        BrepBooleanOperation::Union,
                        area_a + area_b - intersection,
                        1,
                    ),
                    (
                        BrepBooleanOperation::Difference,
                        area_a - intersection,
                        difference_pieces,
                    ),
                    (BrepBooleanOperation::Intersection, intersection, 1),
                ] {
                    checked += 1;
                    match boolean_planar_breps(&[a, b], operation, tolerance) {
                        Ok(pieces) => {
                            let actual = pieces
                                .iter()
                                .map(|b| b.area(tolerance).unwrap() / scale / scale)
                                .sum::<f64>();
                            let error = (actual - expected).abs();
                            maximum_area_error = maximum_area_error.max(error);
                            assert!(
                                error < 1e-6,
                                "{name} {exponent} tilted={tilted} {operation:?}: {actual} != {expected}"
                            );
                            assert_eq!(pieces.len(), count, "{name} {exponent} {operation:?}");
                            assert!(pieces.iter().all(|b| b.is_manifold() && !b.is_solid()));
                            for vertex in pieces.iter().flat_map(|b| b.vertices()) {
                                assert!(
                                    frame.coordinates_of(vertex.point()).unwrap()[2].abs()
                                        < tolerance.absolute()
                                );
                            }
                        }
                        Err(error) => failures.push(format!(
                            "{name} {exponent} tilted={tilted} {operation:?}: {error:?}"
                        )),
                    }
                    assert_eq!([a, b], [&before[0], &before[1]]);
                }
            }
        }
    }
    eprintln!(
        "PLANAR_SCALE cases={checked} failures={} max_normalized_area_error={maximum_area_error:e}",
        failures.len()
    );
    assert_eq!(checked, 120);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

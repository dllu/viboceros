use viboceros_geometry::{
    Brep, Circle3, Frame3, NurbsSurface, Point3, SurfaceSurfaceIntersectionEvent, Tolerance,
    Vector3, surface_surface_intersection_events,
};

fn setup(exponent: i32) -> (f64, Tolerance, Frame3) {
    let scale = 2_f64.powi(exponent);
    let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(5. * scale, -7. * scale, 11. * scale).unwrap(),
        Vector3::try_new(2., 3., 6.).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    (scale, tolerance, frame)
}

#[test]
fn typed_normal_frames_retain_directions_and_raw_normal_size_admission() {
    for normal in [
        [0., 0., 1.],
        [0., 1., 0.],
        [1., 0., 0.],
        [2., 3., 6.],
        [-2., 3., -6.],
    ] {
        let direction = Vector3::try_from(normal)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let origin = Point3::try_new(1e24, -2e24, 3e24).unwrap();
        let frame = Frame3::try_from_unit_normal(origin, direction).unwrap();
        assert_eq!(frame.origin(), origin);
        assert_eq!(frame.z_axis(), direction);
        assert!(
            frame
                .x_axis()
                .as_vector()
                .dot(frame.z_axis().as_vector())
                .unwrap()
                .abs()
                < 4. * f64::EPSILON
        );
        assert!(
            frame
                .y_axis()
                .as_vector()
                .dot(frame.z_axis().as_vector())
                .unwrap()
                .abs()
                < 4. * f64::EPSILON
        );
        assert!(
            frame
                .x_axis()
                .as_vector()
                .cross(frame.y_axis().as_vector())
                .unwrap()
                .dot(direction.as_vector())
                .unwrap()
                > 1. - 8. * f64::EPSILON
        );
        let raw = Frame3::try_from_normal(
            origin,
            Vector3::try_from(normal).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert_eq!(frame, raw);
    }
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    assert!(
        Frame3::try_from_normal(
            origin,
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::try_new(2., 1e-12, 1e-9).unwrap()
        )
        .is_err()
    );
    assert!(
        Frame3::try_from_normal(
            origin,
            Vector3::try_new(0., 0., 1e-12).unwrap(),
            Tolerance::DEFAULT
        )
        .is_err()
    );
}

#[test]
fn circle_unit_axes_obey_angular_admission_at_every_distance_tolerance() {
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    for absolute in [1e-13, 1e-7, 0.1, 1., 1e5, 1e17] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-9).unwrap();
        let radius = 10. * absolute.max(1.);
        for (projection, accepted) in [(0., false), (0.5e-9, false), (2e-9, true), (1., true)] {
            let x = Vector3::try_new(projection, 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap();
            let result = Circle3::try_from_frame(origin, radius, x, normal, tolerance);
            assert_eq!(
                result.is_ok(),
                accepted,
                "absolute={absolute} projection={projection}"
            );
        }
    }
}

#[test]
fn circle_direction_constructors_keep_model_size_and_angular_policies_separate() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let point = |x, y| frame.point_at([x * scale, y * scale, 0.]).unwrap();
        let center = frame.origin();
        for (name, result, expected_center, expected_normal) in [
            (
                "normal",
                Circle3::try_new(center, 2. * scale, frame.z_axis(), tolerance),
                center,
                frame.z_axis(),
            ),
            (
                "frame",
                Circle3::try_from_frame(
                    center,
                    2. * scale,
                    frame.x_axis(),
                    frame.z_axis(),
                    tolerance,
                ),
                center,
                frame.z_axis(),
            ),
            (
                "diameter",
                Circle3::try_from_diameter_on_plane(
                    point(-2., 0.),
                    point(2., 0.),
                    frame,
                    tolerance,
                ),
                center,
                frame.z_axis(),
            ),
            (
                "vertical",
                Circle3::try_from_vertical_direction(
                    center,
                    2. * scale,
                    point(2., 0.),
                    frame,
                    tolerance,
                ),
                center,
                frame.y_axis(),
            ),
            (
                "two-points-radius",
                Circle3::try_from_two_points_radius_direction(
                    point(-1., 0.),
                    point(1., 0.),
                    2. * scale,
                    point(0., 2.),
                    tolerance,
                ),
                point(0., 3_f64.sqrt()),
                frame.z_axis(),
            ),
        ] {
            checked += 1;
            match result {
                Ok(circle) => {
                    assert!(
                        (circle.radius() / scale - 2.).abs() < 1e-12,
                        "{name} {exponent}"
                    );
                    assert!(
                        circle.center().distance_to(expected_center).unwrap() / scale < 1e-12,
                        "{name} {exponent}"
                    );
                    assert!(
                        circle
                            .normal()
                            .unwrap()
                            .as_vector()
                            .dot(expected_normal.as_vector())
                            .unwrap()
                            .abs()
                            > 1. - 1e-12
                    );
                    let curve = circle.to_nurbs().unwrap();
                    for fraction in [0., 0.13, 0.49, 0.81, 1.] {
                        let t = *curve.domain().start()
                            + (*curve.domain().end() - *curve.domain().start()) * fraction;
                        assert!(
                            (curve
                                .evaluate(t)
                                .unwrap()
                                .distance_to(expected_center)
                                .unwrap()
                                / scale
                                - 2.)
                                .abs()
                                < 1e-12
                        );
                    }
                }
                Err(error) => failures.push(format!("{name} exponent={exponent}: {error:?}")),
            }
        }
        assert!(
            Circle3::try_new(center, tolerance.absolute() / 2., frame.z_axis(), tolerance).is_err()
        );
        assert!(
            Circle3::try_from_frame(
                center,
                2. * scale,
                frame.z_axis(),
                frame.z_axis(),
                tolerance
            )
            .is_err()
        );
    }
    eprintln!(
        "UNIT_DIRECTION_CIRCLES cases={checked} failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn sphere_intersection_unit_axis_does_not_depend_on_model_distance_tolerance() {
    let mut failures = Vec::new();
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let other_center = frame.point_at([2. * scale, 0., 0.]).unwrap();
        let a = NurbsSurface::try_sphere(frame, 2. * scale).unwrap();
        let b = NurbsSurface::try_sphere(frame.with_origin(other_center), 2. * scale).unwrap();
        let before = [a.clone(), b.clone()];
        match surface_surface_intersection_events(&a, &b, tolerance) {
            Ok(events) => {
                assert_eq!(events.len(), 2);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("intersection circle")
                    };
                    for fraction in [0., 0.13, 0.49, 0.81, 1.] {
                        let t = *curve.domain().start()
                            + (*curve.domain().end() - *curve.domain().start()) * fraction;
                        let point = curve.evaluate(t).unwrap();
                        for center in [frame.origin(), other_center] {
                            assert!(
                                (point.distance_to(center).unwrap() / scale - 2.).abs() < 1e-12
                            );
                        }
                    }
                }
            }
            Err(error) => failures.push(format!("exponent={exponent}: {error:?}")),
        }
        assert_eq!([a, b], before);
    }
    eprintln!("UNIT_DIRECTION_SPHERES cases=5 failures={}", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn planar_caps_keep_source_topology_at_large_model_tolerances() {
    let mut failures = Vec::new();
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let cube = Brep::try_box(
            frame,
            [[0., 2. * scale], [0., 3. * scale], [0., 5. * scale]],
            tolerance,
        )
        .unwrap();
        for omitted in 0..6 {
            let source = cube
                .sub_brep(
                    &(0..6).filter(|i| *i != omitted).collect::<Vec<_>>(),
                    tolerance,
                )
                .unwrap();
            let before = source.clone();
            match source.try_cap_planar_holes(tolerance) {
                Ok(Some(capped)) => {
                    assert!(capped.is_solid());
                    assert_eq!(capped.faces().len(), 6);
                    assert_eq!(capped.vertices(), source.vertices());
                    assert_eq!(capped.edges(), source.edges());
                    let volume =
                        capped.signed_volume(Tolerance::DEFAULT).unwrap() / scale / scale / scale;
                    assert!(
                        (volume - 30.).abs() < 1e-8,
                        "{exponent} {omitted}: {volume}"
                    );
                }
                result => {
                    failures.push(format!("exponent={exponent} omitted={omitted}: {result:?}"))
                }
            }
            assert_eq!(source, before);
        }
    }
    eprintln!("UNIT_DIRECTION_CAPS cases=30 failures={}", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn crossed_torus_section_frames_use_directions_at_every_model_scale() {
    let mut failures = Vec::new();
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let second = Frame3::try_from_normal(
            frame.origin(),
            frame.x_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let a = NurbsSurface::try_torus(frame, 4. * scale, scale).unwrap();
        let b = NurbsSurface::try_torus(second, 4. * scale, scale).unwrap();
        let before = [a.clone(), b.clone()];
        match surface_surface_intersection_events(&a, &b, tolerance) {
            Ok(events) => {
                assert_eq!(events.len(), 4);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("closed sections")
                    };
                    assert!(curve.is_closed().unwrap());
                    for sample in 0..=16 {
                        let t = *curve.domain().start()
                            + (*curve.domain().end() - *curve.domain().start()) * sample as f64
                                / 16.;
                        let point = curve.evaluate(t).unwrap();
                        for frame in [frame, second] {
                            let [x, y, z] = frame.coordinates_of(point).unwrap().map(|x| x / scale);
                            assert!(((x.hypot(y) - 4.).hypot(z) - 1.).abs() < 5e-7);
                        }
                    }
                }
            }
            Err(error) => failures.push(format!("exponent={exponent}: {error:?}")),
        }
        assert_eq!([a, b], before);
    }
    eprintln!("UNIT_DIRECTION_TORI cases=5 failures={}", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

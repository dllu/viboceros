use viboceros_command::{CommandRegistry, construction_plane, orient_curve};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{
    Circle3, Curve3, CurveRef, Frame3, LineSegment, MeshFace, NurbsCurve, NurbsSurface, Point3,
    Tolerance, TriangleMesh, Vector3,
};

#[test]
fn point_defined_axes_and_projected_picks_keep_distance_admission_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, _) = setup(exponent);
        let origin = Point3::try_new(0., 0., 0.).unwrap();
        let short = tolerance.absolute() * 0.5;
        let radial = format!("{short},0,{}", 3. * scale);
        for command in [
            format!("Cylinder 0,0,0 {radial} {} Axis=0,0,1", 3. * scale),
            format!("Cone 0,0,0 {radial} {} Axis=0,0,1", 3. * scale),
            format!("MeshSphere 0,0,0 {radial} Axis=0,0,1"),
            format!("Helix 0,0,0 0,0,{short} {} Turns=2", 2. * scale),
            format!(
                "Spiral 0,0,0 0,0,{short} {} {} Turns=2",
                2. * scale,
                3. * scale
            ),
            format!(
                "Spiral Flat 0,0,0 {radial} {} Axis=0,0,1 Turns=2",
                3. * scale
            ),
            format!("Cylinder 0,0,0 {} {} Axis=0,0,0", 2. * scale, 3. * scale),
            format!(
                "Spiral Flat 0,0,0 {} {} Axis=0,0,0 Turns=2",
                2. * scale,
                3. * scale
            ),
        ] {
            let mut document = Document::default();
            document.set_tolerance(tolerance);
            document.add_geometry(Geometry::Point(origin)).unwrap();
            document
                .add_geometry(Geometry::Point(Point3::try_new(scale, 0., 0.).unwrap()))
                .unwrap();
            document.undo().unwrap();
            document.select_all();
            let before = document.objects().cloned().collect::<Vec<_>>();
            let selection = document.selected_object_ids().collect::<Vec<_>>();
            assert!(document.can_undo() && document.can_redo());
            assert!(
                registry.execute(&mut document, &command).is_err(),
                "accepted {command}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(
                document.selected_object_ids().collect::<Vec<_>>(),
                selection
            );
            assert!(document.can_undo() && document.can_redo());
            document.redo().unwrap();
            assert_eq!(document.objects().len(), 2);
            document.undo().unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            checked += 1;
        }
        let plane = construction_plane::WorldPlane::Top.frame();
        let along_z = Point3::try_new(0., 0., 3. * scale).unwrap();
        let short_axis = Point3::try_new(0., 0., short).unwrap();
        let projected_short = Point3::try_new(short, 0., 3. * scale).unwrap();
        let valid = Point3::try_new(2. * scale, 0., 0.).unwrap();
        for result in [
            construction_plane::vertical(plane, origin, projected_short, tolerance),
            construction_plane::three_point_z_axis(origin, short_axis, tolerance),
            construction_plane::rotated(plane, origin, short_axis, 90., tolerance),
            construction_plane::rotated_by_reference_points(
                plane,
                origin,
                along_z,
                projected_short,
                valid,
                tolerance,
            ),
            construction_plane::rotated_by_reference_points(
                plane,
                origin,
                along_z,
                valid,
                projected_short,
                tolerance,
            ),
        ] {
            assert!(result.is_err(), "scale=2^{exponent}");
            checked += 1;
        }
        assert!(
            construction_plane::surface_frame_with_x(plane, projected_short, tolerance).is_err()
        );
        checked += 1;
    }
    assert_eq!(checked, 70);
    eprintln!("DIRECTION_ADAPTERS rejected_physical_cues={checked}");
}

#[test]
fn helix_and_spiral_adapters_keep_axial_and_radial_geometry() {
    let registry = CommandRegistry::with_builtins();
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let end = token(frame.point_at([0., 0., 3. * scale]).unwrap());
        let mut flat_reference = None;
        for (kind, command) in [
            ("helix", format!("Helix 0,0,0 {end} {} Turns=2", 2. * scale)),
            (
                "spiral",
                format!("Spiral 0,0,0 {end} {} {} Turns=2", 2. * scale, 3. * scale),
            ),
        ]
        .into_iter()
        .chain([-200, 0, 200].map(|e| {
            let factor = 2_f64.powi(e);
            (
                "flat",
                format!(
                    "Spiral Flat 0,0,0 {} {} Turns=2 Axis={},{},{}",
                    2. * scale,
                    3. * scale,
                    2. * factor,
                    3. * factor,
                    6. * factor
                ),
            )
        })) {
            let mut document = Document::default();
            document.set_tolerance(tolerance);
            document.clear_history().unwrap();
            registry.execute(&mut document, &command).unwrap();
            let before = document.objects().cloned().collect::<Vec<_>>();
            let Geometry::NurbsCurve(curve) = before[0].geometry() else {
                panic!("expected NURBS curve");
            };
            // Axial spirals are cubic approximations. Their interpolation
            // stations lie on the analytic spiral; points between stations
            // have the existing approximation error.
            for i in 0..=24 {
                let fraction = i as f64 / 24.;
                let angle = 2. * std::f64::consts::TAU * fraction;
                let radius = if kind == "helix" { 2. } else { 2. + fraction };
                let expected = frame
                    .point_at([
                        scale * radius * angle.cos(),
                        scale * radius * angle.sin(),
                        if kind == "flat" {
                            0.
                        } else {
                            3. * scale * fraction
                        },
                    ])
                    .unwrap();
                let actual = curve
                    .evaluate(curve.parameter_at(fraction).unwrap())
                    .unwrap();
                assert!(
                    actual.distance_to(expected).unwrap() / scale < 1e-12,
                    "{kind} station={i}"
                );
            }
            for i in 0..=32 {
                let [x, y, z] = frame
                    .coordinates_of(
                        curve
                            .evaluate(curve.parameter_at(i as f64 / 32.).unwrap())
                            .unwrap(),
                    )
                    .unwrap()
                    .map(|x| x / scale);
                let radius = x.hypot(y);
                if kind == "helix" {
                    assert!((radius - 2.).abs() < 1e-6);
                } else {
                    assert!(
                        (2. - 1e-10..=3. + 1e-10).contains(&radius),
                        "{kind} radius={radius}"
                    );
                }
                assert!((-1e-10..=3. + 1e-10).contains(&z));
                if kind == "flat" {
                    assert!(z.abs() < 1e-12);
                }
                if i == 0 {
                    assert!((radius - 2.).abs() < 1e-12 && z.abs() < 1e-12);
                }
                if i == 32 {
                    assert!((radius - if kind == "helix" { 2. } else { 3. }).abs() < 1e-12);
                    assert!((z - if kind == "flat" { 0. } else { 3. }).abs() < 1e-12);
                }
            }
            if kind == "flat" {
                if let Some(reference) = &flat_reference {
                    assert_eq!(curve, reference);
                } else {
                    flat_reference = Some(curve.clone());
                }
            }
            document.undo().unwrap();
            assert_eq!(document.objects().len(), 0);
            document.redo().unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            checked += 1;
        }
    }
    assert_eq!(checked, 25);
    eprintln!("DIRECTION_ADAPTERS helix_spiral={checked}");
}

fn setup(exponent: i32) -> (f64, Tolerance, Frame3) {
    let scale = 2_f64.powi(exponent);
    let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(2., 3., 6.).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    (scale, tolerance, frame)
}

fn token(point: Point3) -> String {
    let [x, y, z] = point.to_array();
    format!("{x},{y},{z}")
}

fn frame_agrees(actual: Frame3, expected: Frame3, scale: f64) {
    assert!(actual.origin().distance_to(expected.origin()).unwrap() / scale < 1e-12);
    for (a, b) in actual.axes().into_iter().zip(expected.axes()) {
        assert!(a.as_vector().dot(b.as_vector()).unwrap() > 1. - 1e-12);
    }
}

fn check_primitive(
    geometry: &Geometry,
    kind: &str,
    frame: Frame3,
    scale: f64,
    tolerance: Tolerance,
) {
    let point = |point: Point3, mesh: bool| {
        let [x, y, z] = frame.coordinates_of(point).unwrap().map(|x| x / scale);
        let radius = x.hypot(y);
        let epsilon = 1e-10;
        match kind {
            "cylinder" | "cone" | "frustum" => {
                assert!(z >= -epsilon && z <= 3. + epsilon, "{kind}: z={z}");
                let expected = match kind {
                    "cone" => 2. * (1. - z / 3.),
                    "frustum" => 2. - z / 3.,
                    _ => 2.,
                };
                let cap = mesh && (z.abs() < epsilon || (z - 3.).abs() < epsilon);
                assert!(
                    if cap {
                        radius <= expected + epsilon
                    } else {
                        (radius - expected).abs() < epsilon
                    },
                    "{kind}: r={radius} expected={expected}, z={z}"
                );
            }
            "sphere" => assert!((radius.hypot(z) - 2.).abs() < epsilon),
            "torus" => assert!(((radius - 3.).hypot(z) - 1.).abs() < epsilon),
            _ => unreachable!(),
        }
    };
    match geometry {
        Geometry::NurbsSurface(surface) => {
            for u in [0., 0.17, 0.49, 0.81, 1.] {
                for v in [0., 0.17, 0.49, 0.81, 1.] {
                    point(
                        surface
                            .evaluate(
                                surface.parameter_at_u(u).unwrap(),
                                surface.parameter_at_v(v).unwrap(),
                            )
                            .unwrap(),
                        false,
                    );
                }
            }
        }
        Geometry::Mesh(mesh) => {
            assert!(mesh.topology().is_closed());
            for &vertex in mesh.vertices() {
                point(vertex, true);
            }
        }
        Geometry::Brep(brep) => {
            assert!(brep.is_solid());
            let expected = match kind {
                "cylinder" => 12. * std::f64::consts::PI,
                "tube" => 9. * std::f64::consts::PI,
                "pyramid" => 8.,
                "truncated-pyramid" => 14.,
                _ => panic!("unexpected solid {kind}"),
            };
            let volume = brep.signed_volume(tolerance).unwrap() / scale / scale / scale;
            assert!(
                (volume - expected).abs() < 1e-6,
                "{kind}: volume={volume} expected={expected}"
            );
        }
        _ => panic!("primitive geometry"),
    }
}

#[test]
fn explicit_primitive_directions_preserve_geometry_across_axis_magnitudes_and_model_units() {
    let registry = CommandRegistry::with_builtins();
    let mut failures = Vec::new();
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        for picked_radius in [false, true] {
            let radius = |value: f64| {
                if picked_radius {
                    token(frame.point_at([value * scale, 0., 0.]).unwrap())
                } else {
                    (value * scale).to_string()
                }
            };
            let r = radius(2.);
            let major = radius(3.);
            for (kind, recipe) in [
                (
                    "cylinder",
                    format!("Cylinder 0,0,0 {r} {} Solid=Yes", 3. * scale),
                ),
                ("cone", format!("Cone 0,0,0 {r} {}", 3. * scale)),
                (
                    "frustum",
                    format!("TruncatedCone 0,0,0 {r} {} {}", 3. * scale, scale),
                ),
                ("tube", format!("Tube 0,0,0 {r} {} {}", scale, 3. * scale)),
                ("torus", format!("Torus 0,0,0 {major} {scale}")),
                (
                    "cylinder",
                    format!("MeshCylinder 0,0,0 {r} {} Solid=Yes", 3. * scale),
                ),
                (
                    "cone",
                    format!("MeshCone 0,0,0 {r} {} Solid=Yes", 3. * scale),
                ),
                (
                    "frustum",
                    format!(
                        "MeshTruncatedCone 0,0,0 {r} {} {} Solid=Yes",
                        3. * scale,
                        scale
                    ),
                ),
                ("sphere", format!("MeshSphere 0,0,0 {r}")),
                ("torus", format!("MeshTorus 0,0,0 {major} {scale}")),
                (
                    "pyramid",
                    format!("Pyramid 4 0,0,0 {r} {} Solid=Yes", 3. * scale),
                ),
                (
                    "truncated-pyramid",
                    format!(
                        "TruncatedPyramid 4 0,0,0 {r} {} {} Solid=Yes",
                        3. * scale,
                        scale
                    ),
                ),
            ] {
                let mut reference = None;
                for direction_exponent in [-200, 0, 200] {
                    checked += 1;
                    let factor = 2_f64.powi(direction_exponent);
                    let command = format!(
                        "{recipe} Axis={},{},{}",
                        2. * factor,
                        3. * factor,
                        6. * factor
                    );
                    let mut document = Document::default();
                    document.set_tolerance(tolerance);
                    document.clear_history().unwrap();
                    match registry.execute(&mut document, &command) {
                        Ok(_) => {
                            assert_eq!(document.objects().len(), 1);
                            let before = document.objects().cloned().collect::<Vec<_>>();
                            let geometry = before[0].geometry();
                            check_primitive(geometry, kind, frame, scale, tolerance);
                            if let Some(reference) = &reference {
                                assert!(geometry == reference, "axis magnitude changed {recipe} at scale 2^{exponent}");
                            } else { reference = Some(geometry.clone()); }
                            document.undo().unwrap();
                            assert_eq!(document.objects().len(), 0);
                            document.redo().unwrap();
                            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                        }
                        Err(error) => failures.push(format!("scale=2^{exponent} axis=2^{direction_exponent} picked={picked_radius} {recipe}: {error:?}")),
                    }
                }
            }
        }
    }
    eprintln!(
        "DIRECTION_ADAPTERS primitives={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 360);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn construction_planes_keep_unit_axes_and_ignore_surface_parameter_units() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let point = |x, y, z| frame.point_at([x * scale, y * scale, z * scale]).unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(2., 3., 0.),
            point(0., 3., 0.),
        ])
        .unwrap();
        let circle = Circle3::try_from_frame(
            frame.origin(),
            2. * scale,
            frame.x_axis(),
            frame.z_axis(),
            tolerance,
        )
        .unwrap();
        let line = LineSegment::try_new(point(0., 0., 0.), point(0., 0., 3.), tolerance).unwrap();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 3., 0.),
                point(0., 3., 0.),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            tolerance,
        )
        .unwrap();
        for (name, result, expected) in [
            (
                "vertical",
                construction_plane::vertical(frame, frame.origin(), point(2., 0., 5.), tolerance)
                    .map_err(|e| e.to_string()),
                None,
            ),
            (
                "circle",
                construction_plane::frame_from_object(&Geometry::Circle(circle), tolerance)
                    .map_err(|e| e.to_string()),
                Some(frame),
            ),
            (
                "line",
                construction_plane::frame_from_object(&Geometry::Line(line), tolerance)
                    .map_err(|e| e.to_string()),
                None,
            ),
            (
                "perpendicular",
                construction_plane::curve_perpendicular_frame(
                    CurveRef::Line(&line),
                    None,
                    tolerance,
                )
                .map_err(|e| e.to_string()),
                None,
            ),
            (
                "mesh",
                construction_plane::frame_from_mesh_face(&mesh, 0, tolerance)
                    .map_err(|e| e.to_string()),
                None,
            ),
            (
                "rotate",
                construction_plane::rotated(
                    frame,
                    frame.origin(),
                    point(0., 0., 3.),
                    90.,
                    tolerance,
                )
                .map_err(|e| e.to_string()),
                None,
            ),
            (
                "rotate-reference",
                construction_plane::rotated_by_reference_points(
                    frame,
                    frame.origin(),
                    point(0., 0., 3.),
                    point(2., 0., 0.),
                    point(0., 2., 0.),
                    tolerance,
                )
                .map_err(|e| e.to_string()),
                None,
            ),
        ] {
            checked += 1;
            match result {
                Ok(actual) => {
                    if let Some(expected) = expected {
                        frame_agrees(actual, expected, scale);
                    }
                    let axes = actual.axes().map(|x| x.as_vector());
                    assert!(
                        axes[0].cross(axes[1]).unwrap().dot(axes[2]).unwrap() > 1. - 1e-12,
                        "{name}"
                    );
                    if name.starts_with("rotate") {
                        assert!(axes[0].dot(frame.y_axis().as_vector()).unwrap() > 1. - 1e-12);
                        assert!(
                            axes[1].dot(frame.x_axis().opposite().as_vector()).unwrap()
                                > 1. - 1e-12
                        );
                    }
                    if name == "perpendicular" {
                        assert!(axes[2].dot(frame.z_axis().as_vector()).unwrap() > 1. - 1e-12);
                    }
                }
                Err(error) => failures.push(format!("scale=2^{exponent} {name}: {error}")),
            }
        }
        for parameter_exponent in [-200, 0, 200] {
            let width = 2_f64.powi(parameter_exponent);
            let source = surface
                .try_reparameterized(0. ..=width, 0. ..=width)
                .unwrap();
            let before = source.clone();
            for (name, result) in [
                (
                    "surface-object",
                    construction_plane::frame_from_object(
                        &Geometry::NurbsSurface(source.clone()),
                        tolerance,
                    ),
                ),
                (
                    "surface-default",
                    construction_plane::surface_frame(&source, false, None, None, tolerance),
                ),
            ] {
                checked += 1;
                match result {
                    Ok(actual) => {
                        frame_agrees(actual, frame.with_origin(point(1., 1.5, 0.)), scale)
                    }
                    Err(error) => failures.push(format!(
                        "scale=2^{exponent} parameters=2^{parameter_exponent} {name}: {error:?}"
                    )),
                }
            }
            assert_eq!(source, before);
        }
    }
    eprintln!(
        "DIRECTION_ADAPTERS construction_planes={checked} failures={}",
        failures.len()
    );
    assert_eq!(checked, 65);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn curve_guided_placement_retains_tangents_and_rigid_distances_at_each_model_scale() {
    let mut checked = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (scale, tolerance, frame) = setup(exponent);
        let point = |x, y, z| frame.point_at([x * scale, y * scale, z * scale]).unwrap();
        let line = Curve3::Line(
            LineSegment::try_new(point(0., 0., 0.), point(0., 0., 3.), tolerance).unwrap(),
        );
        let circle = Curve3::Circle(
            Circle3::try_from_frame(
                frame.origin(),
                2. * scale,
                frame.x_axis(),
                frame.z_axis(),
                tolerance,
            )
            .unwrap(),
        );
        let spatial = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                vec![
                    point(0., 0., 0.),
                    point(1., 0., 0.),
                    point(1., 1., 0.),
                    point(1., 1., 1.),
                ],
                vec![0., 0., 0., 0., 1., 1., 1., 1.],
            )
            .unwrap(),
        );
        for guide in [line, circle, spatial] {
            let sample = guide
                .as_ref()
                .evaluate_with_tangent(*guide.as_ref().domain().start())
                .unwrap();
            for options in [
                orient_curve::Options::default(),
                orient_curve::Options {
                    perpendicular: true,
                    ..Default::default()
                },
                orient_curve::Options {
                    perpendicular: true,
                    rotation: 90.,
                    x_flip: true,
                    y_flip: true,
                    ..Default::default()
                },
            ] {
                let placement = orient_curve::Placement::try_new(
                    guide.clone(),
                    frame.origin(),
                    frame,
                    options,
                    tolerance,
                )
                .unwrap();
                let target = placement.target_frame(sample.point()).unwrap();
                assert!(target.origin().distance_to(sample.point()).unwrap() / scale < 1e-12);
                let tangent_axis = if options.perpendicular {
                    target.z_axis()
                } else {
                    target.x_axis()
                };
                assert!(
                    tangent_axis
                        .as_vector()
                        .dot(sample.tangent().as_vector())
                        .unwrap()
                        > 1. - 1e-12
                );
                let transform = placement.transform_at(sample.point()).unwrap();
                for (i, axis) in target.axes().into_iter().enumerate() {
                    let mut coordinates = [0.; 3];
                    coordinates[i] = 2. * scale;
                    let actual = transform
                        .transform_point(frame.point_at(coordinates).unwrap())
                        .unwrap();
                    let expected = target
                        .origin()
                        .translated(axis.as_vector().scaled(2. * scale).unwrap())
                        .unwrap();
                    assert!(actual.distance_to(expected).unwrap() / scale < 1e-12);
                    assert!(
                        (actual.distance_to(target.origin()).unwrap() / scale - 2.).abs() < 1e-12
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 45);
    eprintln!("DIRECTION_ADAPTERS curve_placement={checked}");
}

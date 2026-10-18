use viboceros_command::{CommandRegistry, arc_extension_input};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{LineSegment, Point3, Tolerance};

fn document(exponent: i32) -> (Document, f64) {
    let scale = 2_f64.powi(exponent);
    let mut document = Document::default();
    document.set_tolerance(Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap());
    document.clear_history().unwrap();
    (document, scale)
}

#[test]
fn primitive_commands_keep_world_directions_at_large_document_tolerances() {
    let registry = CommandRegistry::with_builtins();
    let mut cases = 0;
    for exponent in [-20, 0, 20, 40, 80] {
        let (_, scale) = document(exponent);
        for (command, radii) in [
            (format!("Circle 0,0,0 {}", 2. * scale), [2., 2., 0.]),
            (format!("Sphere 0,0,0 {}", 2. * scale), [2.; 3]),
            (
                format!(
                    "Ellipsoid 0,0,0 {} {} {}",
                    2. * scale,
                    3. * scale,
                    4. * scale
                ),
                [2., 3., 4.],
            ),
            (
                format!(
                    "MeshEllipsoid 0,0,0 {} {} {}",
                    2. * scale,
                    3. * scale,
                    4. * scale
                ),
                [2., 3., 4.],
            ),
        ] {
            let (mut document, _) = document(exponent);
            registry
                .execute(&mut document, &command)
                .unwrap_or_else(|error| panic!("exponent={exponent} {command}: {error:?}"));
            assert_eq!(document.objects().len(), 1);
            let before = document.objects().cloned().collect::<Vec<_>>();
            let bounds = before[0].geometry().bounds();
            let meshed = if let Geometry::Mesh(mesh) = before[0].geometry() {
                // A finite latitude grid need not include every axis extremum.
                // Check its actual vertices against the analytic ellipsoid.
                assert!(mesh.topology().is_closed());
                for point in mesh.vertices() {
                    let implicit = point
                        .to_array()
                        .into_iter()
                        .zip(radii)
                        .map(|(x, radius)| (x / scale / radius).powi(2))
                        .sum::<f64>();
                    assert!((implicit - 1.).abs() < 1e-12, "{command} vertex {implicit}");
                }
                true
            } else {
                false
            };
            for (axis, radius) in radii.into_iter().enumerate() {
                let lo = bounds.min().to_array()[axis] / scale;
                let hi = bounds.max().to_array()[axis] / scale;
                if meshed {
                    assert!(lo >= -radius - 1e-12 && lo < -0.5 * radius, "{command} min");
                    assert!(hi <= radius + 1e-12 && hi > 0.5 * radius, "{command} max");
                } else {
                    assert!((lo + radius).abs() < 1e-12, "{command} min");
                    assert!((hi - radius).abs() < 1e-12, "{command} max");
                }
            }
            document.undo().unwrap();
            assert_eq!(document.objects().len(), 0);
            document.redo().unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            cases += 1;
        }
    }
    eprintln!("UNIT_DIRECTION_COMMANDS primitive_cases={cases}");
    assert_eq!(cases, 20);
}

#[test]
fn infinite_plane_and_arc_extension_reuse_valid_unit_directions_and_preserve_sources() {
    let registry = CommandRegistry::with_builtins();
    for exponent in [-20, 0, 20, 40, 80] {
        let (mut document, scale) = document(exponent);
        let start = Point3::try_new(10. * scale, 20. * scale, -3. * scale).unwrap();
        let end = Point3::try_new(10. * scale, 20. * scale, 5. * scale).unwrap();
        let id = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(start, end, document.tolerance()).unwrap(),
            ))
            .unwrap();
        document.clear_history().unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let mut input = arc_extension_input::Input::new(document.tolerance());
        input.curve(&document, id, None, end).unwrap();
        let frame = input.center_plane().expect("validated endpoint direction");
        assert_eq!(frame.origin(), end);
        assert_eq!(frame.z_axis().as_vector().to_array(), [0., 0., 1.]);
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        registry
            .execute(&mut document, &format!("IPlane Objects={id} Plane=WorldXY"))
            .unwrap();
        assert_eq!(document.objects().len(), 2);
        let result = document.selected_objects().next().unwrap();
        let Geometry::Point(point) = result.geometry() else {
            panic!("plane/line intersection")
        };
        assert_eq!(point.to_array().map(|x| x / scale), [10., 20., 0.]);
        let after = document.objects().cloned().collect::<Vec<_>>();
        assert_eq!(document.object(id), Some(&before[0]));
        document.undo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        document.redo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
    eprintln!("UNIT_DIRECTION_COMMANDS plane_cases=5 extension_frame_cases=5");
}

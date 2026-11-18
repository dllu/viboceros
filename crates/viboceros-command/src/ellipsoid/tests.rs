use super::*;

#[test]
fn solid_ellipsoid_control_nets_match_native_getters() {
    // Successful geometry snapshots from private Rhino 8 command-file and
    // live getter captures, as 45 Euclidean control points and weights.
    let captures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("rhino_control_nets.json")).unwrap();
    let registry = CommandRegistry::with_builtins();
    for capture in captures {
        let mut doc = Document::default();
        let mut command = capture["command"].as_str().unwrap().to_owned();
        let points: Vec<[f64; 4]> = serde_json::from_value(capture["points"].clone()).unwrap();
        let expected_foci: Vec<[f64; 3]> = serde_json::from_value(capture["foci"].clone()).unwrap();
        if command.contains("{guide}") {
            registry.execute(&mut doc, "Line 0,0,0 0,10,10").unwrap();
            let guide = doc.objects().next().unwrap().id();
            command = command.replace("{guide}", &guide.to_string());
        }
        let existing = doc.objects().count();
        registry
            .execute(&mut doc, &command)
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        let mut created = doc.objects().skip(existing);
        let Geometry::NurbsSurface(surface) = created.next().unwrap().geometry() else {
            panic!("{command} did not create an ellipsoid");
        };
        assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
        assert_eq!(
            surface.domain_v(),
            -std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2
        );
        assert_eq!(points.len(), 45);
        for (index, [x, y, z, w]) in points.into_iter().enumerate() {
            let control = surface.control_point(index % 9, index / 9).unwrap();
            let expected = Point3::try_new(x, y, z).unwrap();
            assert!(
                control.point().distance_to(expected).unwrap() < 1e-10,
                "{command} control {index}: {control:?} != {expected:?}"
            );
            assert!(
                (control.weight() - w).abs() < 1e-14,
                "{command} control weight {index}"
            );
        }
        let foci = created
            .map(|obj| match obj.geometry() {
                Geometry::Point(point) => point.to_array(),
                other => panic!("{command}: expected a focal marker, got {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(foci, expected_foci, "{command}");
        assert_eq!(doc.undo_label(), Some("Ellipsoid"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), existing, "{command}");
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.objects().count(),
            existing + 1 + foci.len(),
            "{command}"
        );
    }
}

#[test]
fn invalid_solid_ellipsoid_options_and_axes_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Ellipsoid 0,0 2 3 4").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    for command in [
        "Ellipsoid Deformable 0,0 4,0 0,3 2",
        "Ellipsoid Vertical 0,0 4,0 0,3 2",
        "Ellipsoid Corner 0,0 4,3 0",
        "Ellipsoid FromFoci -4,0 4,0 MarkFoci=Yes 0,3 2",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(doc.objects().count(), 0);
        assert_eq!(doc.redo_label(), Some("Ellipsoid"));
        assert!(!registry.ellipsoid_mark_foci());
    }
}

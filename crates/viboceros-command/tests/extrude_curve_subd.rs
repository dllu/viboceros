use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
use viboceros_geometry::{LineSegment, Point3, Tolerance};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn line(a: Point3, b: Point3) -> Geometry {
    Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}

#[test]
fn extrude_curve_subd_retains_source_metadata_and_rolls_back_mixed_profile_failure() {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    let current = doc.current_layer_id();
    let layer = doc
        .add_layer("Profiles", ColorRgb::new(20, 40, 60))
        .unwrap();
    let source = doc
        .add_geometry_with_attributes(
            line(p(3., 0., 0.), p(3., 0., 5.)),
            ObjectAttributes::on_layer(layer)
                .with_name("Profile")
                .with_object_color(ColorRgb::new(1, 2, 3)),
        )
        .unwrap();
    let anchor = doc.add_geometry(Geometry::Point(p(10., 0., 0.))).unwrap();
    let bad = doc
        .add_geometry(Geometry::NurbsCurve(
            viboceros_geometry::NurbsCurve::try_new(
                1,
                vec![p(0., 0., 0.); 2],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    let group = doc
        .add_group(Some("Profile group".into()), [source, anchor])
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let original = doc.object(source).unwrap().clone();
    commands
        .execute(
            &mut doc,
            "ExtrudeCrv 0,0,0 0,2,0 Output=SubD DeleteInput=No",
        )
        .unwrap();
    assert_eq!(doc.object(source).unwrap(), &original);
    let output = doc
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::SubD(_)))
        .unwrap();
    assert_eq!(output.attributes().layer_id(), current);
    assert_eq!(output.attributes().name(), None);
    assert!(output.group_ids().is_empty());
    assert!(!doc.is_selected(output.id()));
    assert_eq!(
        doc.group(group)
            .unwrap()
            .members()
            .collect::<std::collections::BTreeSet<_>>(),
        [source, anchor].into_iter().collect()
    );
    commands.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([source, bad], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    assert!(
        commands
            .execute(
                &mut doc,
                "ExtrudeCrv 0,0,0 0,2,0 Output=SubD DeleteInput=Yes"
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
    assert_eq!(doc.redo_label(), Some("ExtrudeCrv"));
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    commands
        .execute(
            &mut doc,
            "ExtrudeCrv 0,0,0 0,2,0 Output=SubD DeleteInput=Yes",
        )
        .unwrap();
    assert!(doc.object(source).is_none());
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        [anchor]
    );
}

#[test]
fn extrude_curve_subd_rejects_malformed_options_and_keeps_surface_default() {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    let source = doc
        .add_geometry(line(p(0., 0., 0.), p(3., 0., 0.)))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    commands.execute(&mut doc, "Point 30,20,10").unwrap();
    commands.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for option in [
        "Output=Mesh",
        "Output=Surface Output=SubD",
        "Output=SubD BothSides=Maybe",
        "Output=SubD BothSides=Yes BothSides=No",
        "Output=SubD Solid=Maybe",
        "Output=SubD DeleteInput=Yes DeleteInput=No",
    ] {
        assert!(
            commands
                .execute(&mut doc, &format!("ExtrudeCrv 2 {option}"))
                .is_err(),
            "{option}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.is_selected(source));
        assert_eq!(doc.redo_label(), Some("Point"));
    }
    commands
        .execute(
            &mut doc,
            "ExtrudeCrv -2 _Output=_SubD BothSides=Yes Solid=Yes",
        )
        .unwrap();
    let Geometry::SubD(s) = doc.objects().nth(1).unwrap().geometry() else {
        panic!("SubD required")
    };
    assert_eq!(s.faces().len(), 1);
    assert_eq!(s.vertex_tags(), [3; 4]);
    assert_eq!(
        s.control_points()
            .iter()
            .map(|p| p.z())
            .fold(f64::INFINITY, f64::min),
        -2.
    );
    assert_eq!(
        s.control_points()
            .iter()
            .map(|p| p.z())
            .fold(f64::NEG_INFINITY, f64::max),
        2.
    );
    commands.execute(&mut doc, "Undo").unwrap();
    commands.execute(&mut doc, "ExtrudeCrv 2").unwrap();
    assert!(matches!(
        doc.objects().nth(1).unwrap().geometry(),
        Geometry::Brep(_)
    ));
}

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
fn revolve_subd_retains_source_metadata_and_rolls_back_mixed_profile_failure() {
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
    let axis = doc
        .add_geometry(line(p(0., 0., 0.), p(0., 0., 5.)))
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
            "Revolve 0,0,0 0,0,1 120 Output=SubD SegmentCount=7 DeleteInput=No",
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
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        [source, anchor]
    );
    commands.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([source, axis], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    assert!(
        commands
            .execute(
                &mut doc,
                "Revolve 0,0,0 0,0,1 360 Output=SubD DeleteInput=Yes"
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
    assert_eq!(doc.redo_label(), Some("Revolve"));
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    commands
        .execute(
            &mut doc,
            "Revolve 0,0,0 0,0,1 360 Output=SubD DeleteInput=Yes",
        )
        .unwrap();
    assert!(doc.object(source).is_none());
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        [anchor]
    );
}

#[test]
fn revolve_subd_option_admission_and_large_counts_keep_independent_surface_defaults() {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    let source = doc
        .add_geometry(line(p(3., 0., 0.), p(3., 0., 5.)))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    commands.execute(&mut doc, "Point 10,20,30").unwrap();
    commands.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for option in [
        "SegmentCount=-1",
        "SegmentCount=100001",
        "SegmentCount=4.5",
        "SegmentCount=NaN",
        "SegmentCount=4 SegmentCount=8",
        "Output=Mesh",
        "SegmentCount=8 PointCount=0",
    ] {
        let output = if option.starts_with("Output=") {
            ""
        } else {
            "Output=SubD"
        };
        assert!(
            commands
                .execute(
                    &mut doc,
                    &format!("Revolve 0,0,0 0,0,1 360 {output} {option}")
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.is_selected(source));
        assert_eq!(doc.redo_label(), Some("Point"));
    }
    commands.execute(&mut doc,"Revolve 0,0,0 0,0,1 360 Output=SubD SegmentCount=1001 Deformable=Yes PointCount=16 SplitAtTangents=Yes").unwrap();
    let Geometry::SubD(s) = doc.objects().nth(1).unwrap().geometry() else {
        panic!("SubD required")
    };
    assert_eq!((s.faces().len(), s.control_points().len()), (1001, 2002));
    commands.execute(&mut doc, "Undo").unwrap();
    commands
        .execute(&mut doc, "Revolve 0,0,0 0,0,1 360")
        .unwrap();
    assert!(matches!(
        doc.objects().nth(1).unwrap().geometry(),
        Geometry::NurbsSurface(_)
    ));
}

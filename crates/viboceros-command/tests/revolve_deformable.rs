use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{LineSegment, Point3, Tolerance};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn invalid_deformable_revolutions_preserve_model_attributes_selection_and_redo() {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    let source = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(3., 0., 0.), point(3., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let axis = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(0., 0., 0.), point(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    commands.execute(&mut doc, "Point 10,20,30").unwrap();
    commands.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    for options in [
        "PointCount=3",
        "PointCount=1001",
        "PointCount=-1",
        "PointCount=4.5",
        "PointCount=8 PointCount=9",
        "Deformable=Yes Deformable=No",
        "Deformable=Maybe",
        "StartAngle=NaN",
        "Output=Mesh",
        "FullCircle=Yes",
        "Angle=90",
        "extra",
    ] {
        let input = format!("Revolve 0,0,0 0,0,1 120 Deformable=Yes {options}");
        assert!(commands.execute(&mut doc, &input).is_err(), "{input}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(doc.undo_label(), history.as_deref());
        assert_eq!(doc.redo_label(), Some("Point"));
    }
    doc.select_objects_direct([source, axis], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        commands
            .execute(
                &mut doc,
                "Revolve 0,0,0 0,0,1 120 Deformable=Yes DeleteInput=Yes"
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        doc.selected_object_ids().collect::<Vec<_>>(),
        [source, axis]
    );
    assert_eq!(doc.redo_label(), Some("Point"));
}

#[test]
fn deformable_revolve_uses_native_default_and_accepts_the_resource_limit() {
    let commands = CommandRegistry::with_builtins();
    for count in [None, Some(4), Some(1000)] {
        let mut doc = Document::default();
        commands.execute(&mut doc, "Line 3,0,0 3,0,5").unwrap();
        commands.execute(&mut doc, "SelAll").unwrap();
        let suffix = count
            .map(|n| format!(" PointCount={n}"))
            .unwrap_or_default();
        let full_circle = if count.is_none() {
            "FullCircle"
        } else {
            "FullCircle=Yes"
        };
        commands
            .execute(
                &mut doc,
                &format!("Revolve 0,0,0 0,0,1 {full_circle} Deformable=Yes{suffix}"),
            )
            .unwrap();
        let Geometry::NurbsSurface(s) = doc.objects().nth(1).unwrap().geometry() else {
            panic!("surface required")
        };
        assert_eq!(s.control_point_count_u(), count.unwrap_or(10) + 3);
        assert_eq!(s.degree_u(), 3);
        assert_eq!(s.domain_v(), 0.0..=5.0);
        assert!(s.control_points().iter().all(|p| p.weight() == 1.));
        commands.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 1);
        commands.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), 2);
    }
}

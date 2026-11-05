use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{NurbsCurve, Point3};
fn setup() -> (Document, CommandRegistry) {
    let mut doc = Document::default();
    let curve = NurbsCurve::try_new(
        2,
        [
            [3., 0., 0.],
            [3.5, 0., 1.],
            [4.25, 0., 2.],
            [5., 0., 3.],
            [6.5, 0., 5.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let source = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, CommandRegistry::with_builtins())
}
fn faces(doc: &Document) -> usize {
    let Geometry::Brep(b) = doc.objects().nth(1).unwrap().geometry() else {
        panic!("B-rep required")
    };
    b.faces().len()
}
#[test]
fn straight_split_preference_survives_undo_and_drives_hidden_oblique_output() {
    let (mut doc, commands) = setup();
    commands
        .execute(&mut doc, "UseExtrusions Polysurface")
        .unwrap();
    commands
        .execute(&mut doc, "CreaseSplitting Disable")
        .unwrap();
    commands
        .execute(&mut doc, "ExtrudeCrv 0,0,0 0,3,0 SplitAtTangents=No")
        .unwrap();
    assert_eq!(faces(&doc), 1);
    assert!(!commands.curve_extrusion_split_at_tangents());
    commands.execute(&mut doc, "Undo").unwrap();
    commands
        .execute(&mut doc, "UseExtrusions Extrusion")
        .unwrap();
    commands
        .execute(&mut doc, "ExtrudeCrv 0,0,0 1,3,2")
        .unwrap();
    assert_eq!(faces(&doc), 1);
    commands.execute(&mut doc, "Undo").unwrap();
    commands
        .execute(&mut doc, "UseExtrusions Polysurface")
        .unwrap();
    commands
        .execute(&mut doc, "ExtrudeCrv 0,0,0 1,3,2 SplitAtTangents=Yes")
        .unwrap();
    assert_eq!(faces(&doc), 2);
    let after = doc.objects().cloned().collect::<Vec<_>>();
    commands.execute(&mut doc, "Undo").unwrap();
    assert!(commands.curve_extrusion_split_at_tangents());
    commands.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn malformed_or_unavailable_split_flags_preserve_geometry_redo_and_preference() {
    let (mut doc, commands) = setup();
    commands.execute(&mut doc, "Point 30,20,10").unwrap();
    commands.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    for input in [
        "ExtrudeCrv 3 SplitAtTangents=No",
        "ExtrudeCrv 3 SplitAtTangents=Maybe",
        "ExtrudeCrv 3 SplitAtTangents=No SplitAtTangents=Yes",
    ] {
        assert!(commands.execute(&mut doc, input).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(doc.redo_label(), Some("Point"));
        assert!(commands.curve_extrusion_split_at_tangents());
    }
}

use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{NurbsCurve, Point3};

fn profile() -> Document {
    let mut doc = Document::default();
    let curve = NurbsCurve::try_new(
        1,
        [[3., 0., 0.], [3., 0., 1.], [4., 0., 1.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let source = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    doc
}

#[test]
fn revolve_splitting_uses_shared_preference_outside_document_undo() {
    let commands = CommandRegistry::with_builtins();
    let independent = CommandRegistry::with_builtins();
    let mut settings_doc = Document::default();
    commands
        .execute(&mut settings_doc, "CreaseSplitting Disable")
        .unwrap();
    for (registry, split, expected, preference) in [
        (&commands, false, 1, false),
        (&commands, true, 2, false),
        (&independent, false, 2, true),
    ] {
        for deformable in ["Yes", "No"] {
            let mut doc = profile();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "Revolve 0,0,0 0,0,1 120 Deformable={deformable} SplitAtTangents={}",
                        if split { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            let faces = match doc.objects().nth(1).unwrap().geometry() {
                Geometry::NurbsSurface(_) => 1,
                Geometry::Brep(b) => b.faces().len(),
                _ => panic!("surface required"),
            };
            assert_eq!(faces, expected);
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().len(), 1);
            assert_eq!(registry.crease_splitting(), preference);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().len(), 2);
        }
    }
}

#[test]
fn invalid_revolve_split_options_preserve_source_selection_and_pending_redo() {
    let commands = CommandRegistry::with_builtins();
    let mut doc = profile();
    commands.execute(&mut doc, "Point 10,20,30").unwrap();
    commands.execute(&mut doc, "Undo").unwrap();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    for options in [
        "SplitAtTangents=Maybe",
        "SplitAtTangents=Yes SplitAtTangents=No",
        "SplitAtTangents=Yes PointCount=0",
    ] {
        assert!(
            commands
                .execute(
                    &mut doc,
                    &format!("Revolve 0,0,0 0,0,1 120 DeleteInput=Yes {options}")
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(doc.redo_label(), Some("Point"));
        assert!(doc.undo_label().is_none());
    }
}

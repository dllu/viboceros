use viboceros_command::{
    CommandRegistry, ComponentPick, ComponentSelectionKind as Kind,
    component_selection::ComponentAction,
    edge_pattern_selection::{Pattern, Session},
};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{MeshFace, Point3, Tolerance, TriangleMesh};
fn fixture() -> (Document, viboceros_document::ObjectId) {
    let mut doc = Document::new(Tolerance::DEFAULT);
    let points = (0..4)
        .flat_map(|y| {
            (0..5).map(move |x| Point3::try_new(x as f64 * 3., y as f64 * 3., 0.).unwrap())
        })
        .collect();
    let faces = (0..3)
        .flat_map(|y| {
            (0..4).map(move |x| {
                let a = y * 5 + x;
                MeshFace::Quad([a, a + 1, a + 6, a + 5])
            })
        })
        .collect();
    let id = doc
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new_faces(points, faces, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
#[test]
fn headless_selection_feeds_nurbs_and_preserves_redo_until_geometry_changes() {
    for (command, expected) in [
        ("SelEdgeLoop", [15, 13, 11, 9]),
        ("SelEdgeRing", [28, 20, 11, 2]),
    ] {
        let (mut doc, id) = fixture();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 40,50,60").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.can_redo());
        let seed = ComponentPick {
            object: id,
            kind: Kind::MeshEdge,
            index: 11,
        };
        registry
            .component_selection()
            .replace(&doc, vec![seed])
            .unwrap();
        registry.execute(&mut doc, command).unwrap();
        let selected = registry.component_selection().checked_picks(&doc).unwrap();
        assert_eq!(
            selected.iter().map(|p| p.index).collect::<Vec<_>>(),
            expected
        );
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            registry.component_selection().checked_picks(&doc).unwrap(),
            selected
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.can_redo());
        registry.execute(&mut doc, "SelNone").unwrap();
        assert!(registry.component_selection().is_empty());
        assert!(doc.can_redo());
        assert!(
            registry
                .execute(&mut doc, &format!("{command} Edges=11,999 Object={id}"))
                .is_err()
        );
        assert!(registry.component_selection().is_empty());
        assert!(doc.can_redo());
        registry
            .execute(&mut doc, &format!("{command} Edges=11 Object={id}"))
            .unwrap();
        let selected = registry.component_selection().checked_picks(&doc).unwrap();
        assert!(
            registry
                .execute(&mut doc, &format!("{command} Edges=12,999 Object={id}"))
                .is_err()
        );
        assert_eq!(
            registry.component_selection().checked_picks(&doc).unwrap(),
            selected
        );
        assert!(
            registry
                .component_selection()
                .replace(&doc, vec![ComponentPick { index: 999, ..seed }])
                .is_err()
        );
        assert_eq!(
            registry.component_selection().checked_picks(&doc).unwrap(),
            selected
        );
        registry
            .execute(&mut doc, "ToNURBS DeleteInput=No")
            .unwrap();
        assert_eq!(doc.objects().len(), 5);
        assert!(registry.component_selection().is_empty());
        assert!(!doc.can_redo());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 1);
        assert!(
            CommandRegistry::with_builtins()
                .component_selection()
                .is_empty()
        );
    }
}

#[test]
fn session_validates_atomically_and_owns_source_witnesses() {
    for pattern in [Pattern::Loop, Pattern::Ring, Pattern::FaceLoop] {
        let same = if pattern == Pattern::Loop { 13 } else { 20 };
        let (mut doc, id) = fixture();
        let edge = |index| ComponentPick {
            object: id,
            kind: Kind::MeshEdge,
            index,
        };
        let mut session = Session::new(pattern);
        session
            .pick(&doc, &[edge(11)], ComponentAction::Add)
            .unwrap();
        let before = session.preview().to_vec();
        assert!(
            session
                .pick(&doc, &[edge(12), edge(999)], ComponentAction::Add)
                .is_err()
        );
        assert_eq!(session.seeds(), [edge(11)]);
        assert_eq!(session.preview(), before);
        session
            .pick(&doc, &[edge(same)], ComponentAction::Remove)
            .unwrap();
        assert_eq!(session.preview(), before);
        session
            .pick(&doc, &[edge(same)], ComponentAction::Add)
            .unwrap();
        session
            .pick(&doc, &[edge(11)], ComponentAction::Remove)
            .unwrap();
        assert_eq!(session.preview(), before);
        doc.delete_object(id).unwrap();
        assert!(!session.current(&doc));
        assert!(session.finish(&doc).is_err());
        assert!(session.pick(&doc, &[], ComponentAction::Add).is_err());
    }
}

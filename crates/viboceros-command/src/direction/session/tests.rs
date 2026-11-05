use super::*;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 3., 0.),
            point(0., 3., 0.),
        ])
        .unwrap(),
    )
}

#[test]
fn natural_surface_involutions_restore_original_storage_and_redo() {
    for edit in [
        DirectionEdit::ReverseU,
        DirectionEdit::ReverseV,
        DirectionEdit::SwapUv,
        DirectionEdit::Flip,
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(plane()).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let storage = doc.object(id).unwrap().geometry_snapshot().clone();
        doc.clear_history().unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 99,99,99").unwrap();
        doc.undo().unwrap();
        let before = format!("{doc:?}");
        let mut session = DirectionSession::new(&doc, false).unwrap();
        session.apply(&doc, edit, None).unwrap();
        session.apply(&doc, edit, None).unwrap();
        assert_eq!(session.accept(&mut doc).unwrap(), 0);
        assert_eq!(format!("{doc:?}"), before);
        assert!(storage.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    }
}

#[test]
fn rejected_batches_and_foreign_clicks_preserve_the_whole_preview() {
    let mut doc = Document::default();
    let surface = doc.add_geometry(plane()).unwrap();
    let curve = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(0., 0., 2.), point(1., 0., 2.), doc.tolerance()).unwrap(),
        ))
        .unwrap();
    let foreign = doc.add_geometry(plane()).unwrap();
    doc.select_objects_direct([surface, curve], SelectionMode::Replace)
        .unwrap();
    let mut session = DirectionSession::new(&doc, false).unwrap();
    session.apply(&doc, DirectionEdit::Flip, None).unwrap();
    let staged = session
        .geometries()
        .map(|(id, g)| (id, g.clone()))
        .collect::<Vec<_>>();
    let before = format!("{doc:?}");
    assert!(session.apply(&doc, DirectionEdit::ReverseU, None).is_err());
    assert!(
        session
            .apply(&doc, DirectionEdit::Flip, Some(foreign))
            .is_err()
    );
    assert_eq!(
        session
            .geometries()
            .map(|(id, g)| (id, g.clone()))
            .collect::<Vec<_>>(),
        staged
    );
    assert_eq!(format!("{doc:?}"), before);
    session.cancel(&mut doc).unwrap();
    assert!(!doc.is_selected(surface));
    assert!(!doc.is_selected(curve));
    doc.set_transient_selection([surface, curve]).unwrap();
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn stale_sources_cannot_overwrite_an_intervening_edit() {
    let mut doc = Document::default();
    let id = doc.add_geometry(plane()).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let mut session = DirectionSession::new(&doc, false).unwrap();
    session.apply(&doc, DirectionEdit::ReverseU, None).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "Move 0,0,0 0,0,5")
        .unwrap();
    let before = format!("{doc:?}");
    assert!(!session.is_current(&doc));
    assert!(session.accept(&mut doc).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn subd_reversal_pairs_restore_storage_and_pending_redo_without_archive_equality() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "SubDPlane XCount=2 YCount=2 0,0,0 4,4,0")
        .unwrap();
    let id = doc.objects().last().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let storage = doc.object(id).unwrap().geometry_snapshot().clone();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let mut session = DirectionSession::new(&doc, false).unwrap();
    for _ in 0..3 {
        assert_eq!(session.apply(&doc, DirectionEdit::Flip, None).unwrap(), 1);
        assert_eq!(session.apply(&doc, DirectionEdit::Flip, None).unwrap(), 1);
    }
    assert_eq!(session.accept(&mut doc).unwrap(), 0);
    assert_eq!(format!("{doc:?}"), before);
    assert!(storage.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    assert_eq!(doc.redo().unwrap().as_deref(), Some("Point"));
    assert_eq!(doc.objects().len(), 2);
}

#[test]
fn scripted_subd_dir_uses_the_registry_transaction_and_native_selection_history() {
    for closed in [false, true] {
        for postselected in [false, true] {
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(
                    &mut doc,
                    if closed {
                        "SubDBox XCount=1 YCount=1 ZCount=1 0,0,0 4,4,0 4"
                    } else {
                        "SubDPlane XCount=2 YCount=2 0,0,0 4,4,0"
                    },
                )
                .unwrap();
            let id = doc.objects().last().unwrap().id();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let original = doc.object(id).unwrap().geometry_snapshot().clone();
            doc.clear_history().unwrap();
            registry.execute(&mut doc, "Point 99,99,99").unwrap();
            doc.undo().unwrap();
            let result = if postselected {
                registry.execute_postselected(&mut doc, "Dir Flip", Default::default())
            } else {
                registry.execute(&mut doc, "Dir Flip")
            }
            .unwrap();
            assert!(result.starts_with(if closed { "Edited 0" } else { "Edited 1" }));
            assert_eq!(doc.is_selected(id), !postselected);
            if closed {
                assert!(!doc.can_undo());
                assert_eq!(doc.redo().unwrap().as_deref(), Some("Point"));
            } else {
                assert_eq!(doc.undo_label(), Some("Dir"));
                assert_eq!(doc.undo().unwrap().as_deref(), Some("Dir"));
                assert_eq!(doc.is_selected(id), !postselected);
                assert!(original.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
                assert_eq!(doc.redo().unwrap().as_deref(), Some("Dir"));
                assert_eq!(doc.is_selected(id), !postselected);
            }
        }
    }
}

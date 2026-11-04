use viboceros_command::{
    CommandRegistry,
    chain_selection::{Session, Target, defaults},
};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{LineSegment, Point3};
fn line(x: f64, y: f64) -> Geometry {
    Geometry::Line(
        LineSegment::try_new(
            Point3::try_new(x, y, 0.).unwrap(),
            Point3::try_new(x + 1., y, 0.).unwrap(),
            Default::default(),
        )
        .unwrap(),
    )
}
#[test]
fn long_seeded_chains_ignore_unrelated_dense_endpoints_and_preserve_history() {
    let mut doc = Document::default();
    let mut ids = Vec::new();
    for i in 0..20_000 {
        ids.push(doc.add_geometry(line(i as f64, 0.)).unwrap());
    }
    for i in 0..1000 {
        doc.add_geometry(line(-10., 5. + i as f64 * 1e-10)).unwrap();
    }
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 40,50,60").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let source = doc.object(ids[0]).unwrap().clone();
    let mut session = Session::new(&doc, registry.component_selection(), defaults(&doc)).unwrap();
    session.pick(&doc, Target::Curve(ids[0])).unwrap();
    assert_eq!(session.targets().len(), 20_000);
    session
        .apply(&mut doc, registry.component_selection())
        .unwrap();
    assert_eq!(doc.selected_object_count(), 20_000);
    assert_eq!(doc.object(ids[0]), Some(&source));
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    session.undo(&doc).unwrap();
    session
        .apply(&mut doc, registry.component_selection())
        .unwrap();
    assert_eq!(doc.selected_object_count(), 19_999);
    assert!(doc.can_redo());
}
#[test]
fn scripted_selector_errors_preserve_selection_preferences_and_redo() {
    let mut doc = Document::default();
    let id = doc.add_geometry(line(0., 0.)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut doc,
            &format!("SelChain Seed={id} Direction=Forward AngleTolerance=1"),
        )
        .unwrap();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    let options = registry.chain_selection_options(&doc);
    for args in [
        "Seed=bad",
        "Seed=00000000-0000-0000-0000-000000000000 Direction=Backward",
        "GapTolerance=-1",
        "AngleTolerance=nan",
        "ChainContinuity=unknown",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("SelChain {args}"))
                .is_err()
        );
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(registry.chain_selection_options(&doc), options);
    }
    assert_ne!(
        CommandRegistry::with_builtins().chain_selection_options(&doc),
        options
    );
}
#[test]
fn cancellation_restores_available_baseline_and_drops_hidden_component_owners() {
    use viboceros_command::component_selection::{ComponentPick, ComponentSelectionKind};
    use viboceros_document::SelectionMode;
    use viboceros_geometry::NurbsSurface;
    let mut doc = Document::default();
    let seed = doc.add_geometry(line(0., 0.)).unwrap();
    let baseline = doc.add_geometry(line(10., 0.)).unwrap();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0., 3., 0.).unwrap(),
        Point3::try_new(1., 3., 0.).unwrap(),
        Point3::try_new(1., 4., 0.).unwrap(),
        Point3::try_new(0., 4., 0.).unwrap(),
    ])
    .unwrap();
    let owner = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.clear_history().unwrap();
    let registry = CommandRegistry::with_builtins();
    doc.select_objects_direct([baseline], SelectionMode::Replace)
        .unwrap();
    registry
        .component_selection()
        .replace(
            &doc,
            vec![ComponentPick {
                object: owner,
                index: 0,
                kind: ComponentSelectionKind::BrepEdge,
            }],
        )
        .unwrap();
    let mut session = Session::new(&doc, registry.component_selection(), defaults(&doc)).unwrap();
    session.pick(&doc, Target::Curve(seed)).unwrap();
    session
        .apply(&mut doc, registry.component_selection())
        .unwrap();
    assert_eq!(registry.component_selection().len(), 1);
    doc.select_objects_direct([owner], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Hide").unwrap();
    assert!(!session.current(&doc));
    let history = doc.undo_label().map(str::to_owned);
    session
        .cancel(&mut doc, registry.component_selection())
        .unwrap();
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [baseline]);
    assert!(registry.component_selection().is_empty());
    assert_eq!(doc.undo_label(), history.as_deref());
}

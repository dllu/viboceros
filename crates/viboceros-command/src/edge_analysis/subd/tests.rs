use super::*;

#[test]
fn large_curved_grid_uses_bounded_crease_curves_and_tracks_appearance_without_model_edits() {
    let mut doc = Document::default();
    let commands = CommandRegistry::with_builtins();
    commands
        .execute(&mut doc, "SubDPlane XCount=80 YCount=60 0,0,0 8,6,0")
        .unwrap();
    let id = doc.objects().last().unwrap().id();
    let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
        unreachable!()
    };
    let points = s
        .control_points()
        .iter()
        .map(|p| Point3::try_new(p.x(), p.y(), 0.1 * p.x() * p.x() + 0.2 * p.y() * p.y()).unwrap())
        .collect::<Vec<_>>();
    let geometry = Geometry::SubD(s.try_with_control_points(&points).unwrap());
    doc.replace_object_geometries([(id, geometry)]).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert!(ObjectSelectionFilter::DiagnosticEdges.accepts_object(doc.object(id).unwrap()));
    assert!(!ObjectSelectionFilter::EdgeAnalysis.accepts_object(doc.object(id).unwrap()));
    let source = doc.object(id).unwrap().clone();
    doc.clear_history().unwrap();
    commands.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    commands.execute(&mut doc, "ShowEdges Show=All").unwrap();
    let smooth = commands.edge_analysis_view(&doc).unwrap().unwrap();
    assert_eq!(smooth.displayed().count(), 280);
    assert!(
        smooth
            .edges
            .iter()
            .all(|e| e.naked && e.curve.degree() == 3)
    );
    assert!(!doc.is_selected(id));
    commands
        .execute(&mut doc, "ShowEdges Color=12,34,56")
        .unwrap();
    assert!(Arc::ptr_eq(
        &smooth.edges,
        &commands.edge_analysis_view(&doc).unwrap().unwrap().edges
    ));
    commands.execute(&mut doc, "SubDDisplayToggle").unwrap();
    let flat = commands.edge_analysis_view(&doc).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&smooth.edges, &flat.edges));
    assert_eq!(flat.edges.len(), smooth.edges.len());
    assert!(flat.edges.iter().all(|e| e.curve.degree() == 1));
    assert!(smooth.edges.iter().zip(flat.edges.iter()).any(|(a, b)| {
        a.curve
            .evaluate(0.5)
            .unwrap()
            .distance_to(
                b.curve
                    .evaluate((b.curve.domain().start() + b.curve.domain().end()) / 2.)
                    .unwrap(),
            )
            .unwrap()
            > 1e-6
    }));
    assert_eq!(doc.object(id), Some(&source));
    assert!(!doc.can_undo());
    assert_eq!(doc.redo().unwrap().as_deref(), Some("Point"));
}

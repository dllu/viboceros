use super::*;
use viboceros_geometry::SubD;
fn subd(x: f64) -> Geometry {
    Geometry::SubD(
        SubD::try_from_polygons(
            [[x, 0., 0.], [x + 4., 0., 0.], [x + 4., 3., 0.], [x, 3., 0.]]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
            vec![vec![0, 1, 2, 3]],
        )
        .unwrap(),
    )
}
#[test]
fn subd_join_copy_keeps_peers_sources_redo_and_remembered_options_consistent() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let a = doc.add_geometry(subd(0.)).unwrap();
    let b = doc.add_geometry(subd(4.)).unwrap();
    let peer = doc
        .add_geometry(Geometry::Point(Point3::try_new(40., 20., 0.).unwrap()))
        .unwrap();
    let group = doc
        .add_group(Some("Assembly".into()), [a, b, peer])
        .unwrap();
    registry.execute(&mut doc, "Point 30,40,50").unwrap();
    doc.undo().unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    let original = doc.objects().cloned().collect::<Vec<_>>();
    doc.select_objects_direct([a], SelectionMode::Replace)
        .unwrap();
    assert!(
        registry
            .execute(&mut doc, "JoinCopy SubDJoinedEdges=Crease")
            .is_err()
    );
    assert_eq!(doc.redo_label(), redo.as_deref());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
    doc.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, "JoinCopy SubDJoinedEdges=Crease")
        .unwrap();
    let output = doc
        .objects()
        .find(|o| !original.iter().any(|source| source.id() == o.id()))
        .unwrap()
        .id();
    assert!(!doc.is_selected(peer));
    assert!(doc.is_selected(a) && doc.is_selected(b) && doc.is_selected(output));
    assert_eq!(doc.object(a).unwrap(), &original[0]);
    assert_eq!(doc.object(b).unwrap(), &original[1]);
    assert_eq!(doc.object(output).unwrap().group_ids(), [group]);
    assert_eq!(doc.group(group).unwrap().members().count(), 4);
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
    doc.redo().unwrap();
    assert_eq!(doc.group(group).unwrap().members().count(), 4);
    for bad in [
        "SubDJoinedEdges=Maybe",
        "SubDJoinedEdges=Crease SubDJoinedEdges=Smooth",
        "SubDJoinedEdges=Crease JoinDisjointMeshes=Maybe",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("JoinCopy {bad}"))
                .is_err()
        );
        let prompt = registry
            .object_selection_prompt("JoinCopy")
            .unwrap()
            .unwrap();
        assert_eq!(prompt.choices[0].value, "Crease");
    }
}

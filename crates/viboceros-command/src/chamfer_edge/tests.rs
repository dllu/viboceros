use super::*;
#[test]
fn chamfer_options_are_atomic_and_metadata_requires_edges() {
    let mut o = ChamferOptions::default();
    o.update(&["Distance=2", "RailType=RollingBall"]).unwrap();
    let original = o;
    assert!(
        o.update(&["RailType=DistBetweenRails", "Distance=0"])
            .is_err()
    );
    assert_eq!(o, original);
    assert!(parse(&["Distance=1", "distance=2"]).is_err());
    assert!(parse(&["All=Yes", "1", "0"]).is_err());
    let r = CommandRegistry::with_builtins();
    let p = r
        .component_selection_prompt("ChamferEdge 2 RailType=RollingBall")
        .unwrap()
        .unwrap();
    assert_eq!(p.kind, ComponentSelectionKind::BrepEdge);
    assert_eq!(p.numbers[0].value, 2.);
}
fn source(doc: &mut Document, size: f64) -> ObjectId {
    doc.add_geometry(Geometry::Brep(
        Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., size]; 3],
            doc.tolerance(),
        )
        .unwrap(),
    ))
    .unwrap()
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn chamfer_portable_decline_preserves_document() {
    let mut doc = Document::default();
    let id = source(&mut doc, 10.);
    let before = format!("{doc:?}");
    assert!(matches!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, &format!("ChamferEdge Distance=1 {id} 0")),
        Err(CommandError::ChamferEdgeRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn chamfer_preserves_identity_attributes_groups_and_history() {
    let mut doc = Document::default();
    let id = source(&mut doc, 10.);
    doc.set_object_names([(id, Some("Bevelled".into()))])
        .unwrap();
    let group = doc.add_group(None, [id]).unwrap();
    let original = doc.object(id).unwrap().clone();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut doc, &format!("ChamferEdge Distance=1 {id} 0,0"))
        .unwrap();
    let after = doc.object(id).unwrap().clone();
    assert_eq!(after.attributes(), original.attributes());
    assert_eq!(
        doc.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![id]
    );
    let Geometry::Brep(b) = after.geometry() else {
        panic!()
    };
    assert!((b.signed_volume(doc.tolerance()).unwrap() - 997.5).abs() < 1e-6);
    assert_eq!(doc.undo_label(), Some("ChamferEdge"));
    r.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&original));
    r.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&after));
}
#[cfg(feature = "native-smlib")]
#[test]
fn chamfer_later_kernel_failure_and_stale_sources_preserve_history() {
    let mut doc = Document::default();
    let a = source(&mut doc, 10.);
    let b = source(&mut doc, 0.1);
    let r = CommandRegistry::with_builtins();
    r.execute(&mut doc, &format!("ChamferEdge Distance=0.1 {a} 0"))
        .unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        r.execute(&mut doc, &format!("ChamferEdge Distance=1 {a} 0 {b} 0"))
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
    let prepared =
        EdgeBlendSelection::prepare_chamfer(&doc, [(a, 0)], 1., ChamferRailType::RollingBall)
            .unwrap();
    doc.replace_object_geometries([(a, Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))])
        .unwrap();
    let before = format!("{doc:?}");
    assert!(prepared.commit(&mut doc).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

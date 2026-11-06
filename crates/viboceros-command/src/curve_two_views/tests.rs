use super::*;
#[test]
fn reconstructs_profiles_with_fresh_unselected_output_and_one_undo() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    // This experimental solver's exact fast path requires a shared chart.
    // SDK polylines use vertex indices; command polylines use independent lengths.
    for points in [
        [[0., 0., 0.], [5., 4., 0.], [10., 1., 0.]],
        [[0., 0., 2.], [5., 0., 3.], [10., 0., -1.]],
    ] {
        doc.add_geometry(Geometry::Polyline(
            Polyline3::try_new(
                points
                    .into_iter()
                    .map(|p| Point3::try_from(p).unwrap())
                    .collect(),
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.clear_selection();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let input = parse(&[
        &format!("Sources={},{}", ids[0], ids[1]),
        "Direction1=0,0,1",
        "Direction2=0,1,0",
    ])
    .unwrap();
    prepare(
        &doc,
        &input.profiles(&doc).unwrap(),
        CommandContext::default().construction_plane,
    )
    .unwrap()
    .commit(&mut doc)
    .unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert_eq!(doc.selected_object_count(), 0);
    for object in &before {
        assert_eq!(doc.object(object.id()), Some(object));
    }
    r.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    r.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 3);
}
#[test]
fn preparation_rejects_changed_parents_and_invalid_directions_without_history() {
    let r = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for command in ["Line 0,0,0 10,5,0", "Line 0,0,2 10,0,4"] {
        r.execute(&mut doc, command).unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.clear_selection();
    doc.clear_history().unwrap();
    let input = parse(&[
        &format!("Sources={},{}", ids[0], ids[1]),
        "Direction1=0,0,1",
        "Direction2=0,1,0",
    ])
    .unwrap();
    let staged = prepare(
        &doc,
        &input.profiles(&doc).unwrap(),
        CommandContext::default().construction_plane,
    )
    .unwrap();
    doc.set_object_geometry_user_text([ids[0]], "changed", Some("yes"))
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(staged.commit(&mut doc).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    for argument in [
        "Direction1=0,0,0",
        "Direction2=nan,1,0",
        "Edge1=-1",
        "SubCrv1=0,1,2",
    ] {
        assert!(parse(&[argument]).is_err());
    }
}

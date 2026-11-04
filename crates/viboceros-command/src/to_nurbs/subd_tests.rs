use super::*;
use viboceros_geometry::SubD;

fn source() -> SubD {
    SubD::try_from_polygons(
        vec![
            [0., 0., 0.],
            [2., 0., 1.],
            [4., 0., 0.],
            [0., 3., 0.],
            [2., 3., 2.],
            [4., 3., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
    .try_with_vertex_tags(&[(0, 3), (2, 3), (3, 3), (5, 3)])
    .unwrap()
}

#[test]
fn uniform_soft_edge_conversion_preserves_attributes_groups_identity_and_history() {
    for packed in [false, true] {
        for delete in [false, true] {
            let registry = CommandRegistry::with_builtins();
            let mut doc = Document::default();
            let subd = source();
            let edge = subd
                .edges()
                .iter()
                .position(|e| *e == [1, 4] || *e == [4, 1])
                .unwrap();
            let (soft, _) = subd
                .try_with_soft_crease_weights(&[(edge, [100.; 2])])
                .unwrap();
            let id = doc.add_geometry(Geometry::SubD(soft)).unwrap();
            let group = doc.add_group(Some("Soft crease".into()), [id]).unwrap();
            let layer = doc
                .add_layer("Other current layer", ColorRgb::BLACK)
                .unwrap();
            doc.set_current_layer(layer).unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let original = doc.object(id).unwrap().clone();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "ToNURBS Faces={} ExtraordinaryVertex=G2 DeleteInputObjects={}",
                        if packed { "Packed" } else { "Unpacked" },
                        if delete { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            assert_eq!(doc.objects().len(), if delete { 1 } else { 2 });
            let output = if delete {
                doc.object(id).unwrap()
            } else {
                doc.objects().find(|o| o.id() != id).unwrap()
            };
            let Geometry::Brep(brep) = output.geometry() else {
                panic!()
            };
            assert_eq!(brep.faces().len(), if packed { 1 } else { 2 });
            assert_eq!(output.attributes(), original.attributes());
            assert_eq!(output.group_ids(), [group]);
            assert_eq!(doc.is_selected(output.id()), delete);
            let after = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), [original]);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn subd_failure_rolls_back_the_whole_mixed_conversion_and_preserves_redo_and_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let line = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(4., 2., 0.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    let subd = source().subdivided(5).unwrap();
    let edge = subd.edge_tags().iter().position(|&tag| tag == 1).unwrap();
    let (soft, changed) = subd
        .try_with_soft_crease_weights(&[(edge, [100.; 2])])
        .unwrap();
    assert!(changed);
    let id = doc.add_geometry(Geometry::SubD(soft)).unwrap();
    doc.add_group(Some("both".into()), [line, id]).unwrap();
    doc.select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.set_transient_selection([line, id]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let groups = doc.groups().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    assert!(doc.can_redo());
    let error = registry
        .execute(
            &mut doc,
            "ToNURBS DeleteInputObjects=Yes Faces=Unpacked ExtraordinaryVertex=G2",
        )
        .unwrap_err();
    assert!(error.to_string().contains("budget"), "{error}");
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.groups().cloned().collect::<Vec<_>>(), groups);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [line, id]);
    assert!(doc.can_redo());
    assert_eq!(doc.undo_label(), history.as_deref());
    let mut next = Document::default();
    let id = next.add_geometry(Geometry::SubD(source())).unwrap();
    next.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut next, "ToNURBS").unwrap();
    assert_eq!(next.objects().len(), 2);
    let Geometry::Brep(b) = next.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 1);
}

#[test]
fn subd_options_require_a_selected_subd_and_invalid_edits_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::SubD(source())).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let base = registry
        .object_selection_prompt("ToNURBS")
        .unwrap()
        .unwrap();
    let mut prompt = registry
        .object_selection_confirmation(&doc, &base)
        .unwrap()
        .unwrap();
    for invalid in [
        "Faces=Invalid",
        "ExtraordinaryVertex=G3",
        "Faces=Packed Faces=Unpacked",
        "ExtraordinaryVertex=G0 ExtraordinaryVertex=G1",
        "Faces=Unpacked DeleteInputObjects=Yes",
    ] {
        let before = prompt.clone();
        assert!(prompt.update_menu_options(0, invalid).is_err());
        assert_eq!(prompt, before);
    }
    prompt
        .update_menu_options(0, "Faces=Unpacked ExtraordinaryVertex=G1xx")
        .unwrap();
    assert!(
        prompt
            .command_line()
            .contains("SubDOptions Faces=Unpacked ExtraordinaryVertex=G1xx")
    );
    doc.clear_selection();
    let line = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(4., 0., 0.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    for invalid in [
        "ToNURBS SubDOptions",
        "ToNURBS Faces=Packed",
        "ToNURBS ExtraordinaryVertex=G2",
    ] {
        assert!(registry.execute(&mut doc, invalid).is_err());
    }
}

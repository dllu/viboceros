use super::*;
fn setup() -> (CommandRegistry, Document, Vec<ObjectId>) {
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for input in [
        "Polyline 0,0,0 6,0,0 6,6,0 0,6,0 0,0,0",
        "Polyline 4,2,0 10,2,0 10,8,0 4,8,0 4,2,0",
    ] {
        commands.execute(&mut doc, input).unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (commands, doc, ids)
}
fn areas(doc: &Document) -> Vec<Real> {
    let mut result = doc
        .selected_objects()
        .map(|o| {
            if let Geometry::Brep(b) = o.geometry() {
                b.area(doc.tolerance()).unwrap()
            } else {
                Brep::try_planar_face(
                    &o.geometry().curve_ref().unwrap().to_nurbs().unwrap(),
                    doc.tolerance(),
                )
                .unwrap()
                .area(doc.tolerance())
                .unwrap()
            }
        })
        .collect::<Vec<_>>();
    result.sort_by(Real::total_cmp);
    result
}
#[test]
fn selected_regions_options_empty_selection_and_one_undo_redo() {
    for (args, expected) in [
        ("AllRegions CombineRegions=No", vec![8., 28., 28.]),
        ("AllRegions", vec![64.]),
        ("1,1,0 5,3,0", vec![36.]),
        ("1,1,0 1,1,0 DeleteInput=All", vec![]),
    ] {
        let (commands, mut doc, ids) = setup();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        commands
            .execute(&mut doc, &format!("CurveBoolean Output=Surfaces {args}"))
            .unwrap();
        let actual = areas(&doc);
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(&expected) {
            assert!((a - b).abs() < 1e-8, "{args}: {actual:?}");
        }
        for id in ids {
            assert!(doc.object(id).is_some());
        }
        if !expected.is_empty() {
            let after = doc.objects().cloned().collect::<Vec<_>>();
            commands.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
            commands.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        } else {
            assert!(!doc.can_undo());
        }
    }
}
#[test]
fn used_deletes_only_output_boundary_restrictions_and_preserves_metadata() {
    for (picks, combine, expected_pieces) in [
        ("1,1,0", false, 2),
        ("5,3,0", false, 3),
        ("AllRegions", false, 0),
        ("AllRegions", true, 3),
    ] {
        let (commands, mut doc, ids) = setup();
        for (i, id) in ids.iter().enumerate() {
            doc.set_object_geometry_user_text([*id], "origin", Some(&format!("{i}")))
                .unwrap();
            doc.set_object_user_text([*id], "attribute", Some(&format!("{i}")))
                .unwrap();
        }
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        commands
            .execute(
                &mut doc,
                &format!(
                    "CurveBoolean {picks} DeleteInput=Used CombineRegions={} SimplifyCurves=No",
                    if combine { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        assert!(ids.iter().all(|id| doc.object(*id).is_none()));
        let remnants = doc
            .objects()
            .filter(|o| !doc.is_selected(o.id()))
            .collect::<Vec<_>>();
        assert_eq!(remnants.len(), expected_pieces, "{picks} {combine}");
        for remnant in remnants {
            assert!(!remnant.geometry_user_text().is_empty());
            assert!(!remnant.attributes().user_text().is_empty());
            assert!(!remnant.geometry().curve_ref().unwrap().is_closed().unwrap());
        }
        commands.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            ids.into_iter().collect()
        );
    }
}
#[test]
fn stage_is_atomic_and_rejects_stale_inputs() {
    let (_, mut doc, ids) = setup();
    let mut session = prepare(&doc, &ids, CommandContext::default().construction_plane).unwrap();
    session.all_regions();
    let prepared = session
        .stage(Options {
            delete_input: DeleteInput::All,
            ..Options::default()
        })
        .unwrap();
    doc.set_object_geometry_user_text([ids[0]], "changed", Some("yes"))
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(prepared.commit(&mut doc).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
}
#[test]
fn options_are_isolated_from_history_and_invalid_inputs_do_not_change_preferences() {
    let (commands, mut doc, _) = setup();
    commands.execute(&mut doc,"CurveBoolean AllRegions CombineRegions=No Output=Surfaces DeleteInput=All SimplifyCurves=No").unwrap();
    let options = commands.curve_boolean_options();
    commands.execute(&mut doc, "Undo").unwrap();
    assert_eq!(commands.curve_boolean_options(), options);
    assert_eq!(
        CommandRegistry::with_builtins().curve_boolean_options(),
        Options::default()
    );
    for arg in [
        "Output=Mesh",
        "DeleteInput=Yes",
        "CombineRegions=Maybe",
        "SimplifyCurves=Yes SimplifyCurves=No",
        "Sources=1,1",
        "0,nan,0",
    ] {
        assert!(
            commands
                .execute(&mut doc, &format!("CurveBoolean {arg}"))
                .is_err(),
            "{arg}"
        );
        assert_eq!(commands.curve_boolean_options(), options);
    }
}

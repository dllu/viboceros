use super::*;

fn fixture(document: &mut Document) -> ObjectId {
    document
        .add_geometry(Geometry::Brep(
            Brep::try_tube(
                CommandContext::default().construction_plane,
                [2., 5.],
                8.,
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}
fn objects(document: &Document) -> Vec<viboceros_document::Object> {
    document.objects().cloned().collect()
}

#[test]
fn preparation_is_readonly_and_stale_source_failures_preserve_history_and_redo() {
    let mut document = Document::default();
    let object = fixture(&mut document);
    document.clear_history().unwrap();
    let before = objects(&document);
    let plan = UntrimSelection::prepare(
        &document,
        object,
        3,
        UntrimOptions {
            keep_trim_objects: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(plan.changes_geometry());
    assert_eq!(objects(&document), before);
    assert!(!document.can_undo());
    let result = plan.commit(&mut document).unwrap();
    assert!(result.changed);
    assert_eq!(result.removed_faces, 1);
    assert_eq!(result.restored_boundaries, 2);
    assert_eq!(result.retained.len(), 1);
    let after = objects(&document);
    assert!(matches!(
        plan.commit(&mut document),
        Err(CommandError::UntrimStale)
    ));
    assert_eq!(objects(&document), after);
    document.undo().unwrap();
    assert_eq!(objects(&document), before);
    assert!(document.can_redo());
    for edge in [6, usize::MAX] {
        assert!(
            UntrimSelection::prepare(&document, object, edge, UntrimOptions::default()).is_err()
        );
        assert_eq!(objects(&document), before);
        assert!(document.can_redo());
    }
    document.redo().unwrap();
    assert_eq!(objects(&document), after);
}

#[test]
fn option_parse_failures_do_not_change_registry_memory_or_document() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let object = fixture(&mut document);
    let before = objects(&document);
    registry
        .accept_object_selection_input("Untrim AllSimilar=Yes KeepTrimObjects=Yes")
        .unwrap();
    for tail in [
        "AllSimilar=No KeepTrimObjects=Maybe",
        "All=Yes",
        "MaximumEdgeLength=1",
        "KeepTrimObjects=No extra",
    ] {
        assert!(
            registry
                .execute(&mut document, &format!("Untrim {object} 3 {tail}"))
                .is_err()
        );
        assert_eq!(objects(&document), before);
        assert_eq!(
            registry
                .component_selection_prompt("Untrim")
                .unwrap()
                .unwrap()
                .command_line(),
            "Untrim AllSimilar=Yes KeepTrimObjects=Yes"
        );
    }
    assert!(
        !registry
            .component_selection_prompt("UntrimHoles")
            .unwrap()
            .unwrap()
            .options
            .iter()
            .find(|o| o.name == "KeepTrimObjects")
            .unwrap()
            .value
    );
}

#[test]
fn ignored_multi_face_exterior_pick_preserves_objects_history_and_redo() {
    let mut document = Document::default();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(10., 0., 0.).unwrap(),
        Point3::try_new(10., 10., 0.).unwrap(),
        Point3::try_new(0., 10., 0.).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(0.0..=10., 0.0..=10.)
    .unwrap();
    let band =
        Brep::try_rectangular_surface_face(surface, 2.0..=8., 0.0..=10., document.tolerance())
            .unwrap();
    let object = document
        .add_geometry(Geometry::Brep(
            Brep::try_combine(vec![band.clone(), band], document.tolerance()).unwrap(),
        ))
        .unwrap();
    document.clear_history().unwrap();
    document.begin_transaction("redo control").unwrap();
    fixture(&mut document);
    document.commit_transaction().unwrap();
    document.undo().unwrap();
    let before = objects(&document);
    assert!(document.can_redo());
    for all_similar in [false, true] {
        for keep_trim_objects in [false, true] {
            let plan = UntrimSelection::prepare(
                &document,
                object,
                1,
                UntrimOptions {
                    all_similar,
                    keep_trim_objects,
                },
            )
            .unwrap();
            assert!(!plan.changes_geometry());
            assert_eq!(plan.commit(&mut document).unwrap(), UntrimResult::default());
            assert_eq!(objects(&document), before);
            assert!(!document.can_undo());
            assert!(document.can_redo());
        }
    }
    document.redo().unwrap();
    assert_eq!(document.objects().count(), 2);
}

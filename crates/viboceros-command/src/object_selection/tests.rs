use super::*;

#[test]
fn confirmation_menus_are_selection_dependent_atomic_and_readonly_until_conversion() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(4., 0., 0.).unwrap(),
                    Point3::try_new(0., 3., 0.).unwrap(),
                ],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([mesh], SelectionMode::Replace)
        .unwrap();
    let base = registry
        .object_selection_prompt("ToNURBS")
        .unwrap()
        .unwrap();
    assert_eq!(
        base.workflow,
        ObjectSelectionWorkflow::ConfirmAfterSelection
    );
    assert!(base.menus.is_empty());
    let mut prompt = registry
        .object_selection_confirmation(&document, &base)
        .unwrap()
        .unwrap();
    assert_eq!(prompt.menus[0].name, "MeshOptions");
    prompt
        .update_options("DeleteInput Yes MeshOptions TrimTriangularFaces No")
        .unwrap();
    let staged = prompt.clone();
    for input in [
        "DeleteInput=No DeleteInputObjects=Yes",
        "MeshOptions TrimTriangularFaces=Yes MeshOptions",
        "TrimTriangularFaces=Yes Unknown=No",
    ] {
        assert!(prompt.update_options(input).is_err());
        assert_eq!(prompt, staged);
    }
    assert!(
        prompt
            .update_menu_options(99, "TrimTriangularFaces=Yes")
            .is_err()
    );
    assert!(
        prompt
            .update_menu_options(0, "DeleteInputObjects=No")
            .is_err()
    );
    assert_eq!(prompt, staged);
    registry.accept_object_selection_options(&prompt).unwrap();
    assert_eq!(
        registry
            .object_selection_prompt("ToNURBS")
            .unwrap()
            .unwrap(),
        base
    );
    prompt
        .update_menu_options(0, "TrimTriangularFaces Yes")
        .unwrap();
    assert_eq!(
        prompt.command_line(),
        "ToNURBS DeleteInputObjects=Yes MeshOptions TrimTriangularFaces=Yes"
    );
}

#[test]
fn prompt_queries_and_staged_updates_are_readonly_until_explicit_acceptance() {
    let r = CommandRegistry::with_builtins();
    assert!(r.object_selection_prompt("Delete").unwrap().is_none());
    let mut p = r
        .object_selection_prompt("_-mEsHtOnUrB TrimTriangularFaces=No")
        .unwrap()
        .unwrap();
    assert_eq!(p.filter, ObjectSelectionFilter::Mesh);
    assert_eq!(
        p.command_line(),
        "MeshToNURB TrimTriangularFaces=No UseNgons=Yes"
    );
    assert!(
        r.object_selection_prompt("MeshToNURB")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    p.update_options("TrimTriangularFaces Yes UseNgons=No")
        .unwrap();
    let before = p.clone();
    for invalid in [
        "TrimTriangularFaces=No Unknown=Yes",
        "UseNgons=Yes UseNgons=No",
        "TrimTriangularFaces=Maybe",
        "",
    ] {
        assert!(p.update_options(invalid).is_err());
        assert_eq!(p, before);
    }
    r.accept_object_selection_options(&p).unwrap();
    assert!(
        !r.object_selection_prompt("MeshToNURB")
            .unwrap()
            .unwrap()
            .options[1]
            .value
    );
    assert!(
        CommandRegistry::with_builtins()
            .object_selection_prompt("MeshToNURB")
            .unwrap()
            .unwrap()
            .options[1]
            .value
    );
}

#[test]
fn postselection_clears_picks_without_adding_an_extra_history_entry() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(0., 3., 0.).unwrap(),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let id = d.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute_postselected(
        &mut d,
        "MeshToNURB TrimTriangularFaces=No",
        CommandContext::default(),
    )
    .unwrap();
    assert_eq!(d.selected_object_count(), 0);
    assert_eq!(d.undo_label(), Some("MeshToNURB"));
    let after = d.objects().cloned().collect::<Vec<_>>();
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn postselection_failure_keeps_accepted_prompt_choices_but_rolls_back_document_state() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    d.select_objects_direct([point], SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    assert!(
        r.execute_postselected(
            &mut d,
            "MeshToNURB TrimTriangularFaces=No",
            CommandContext::default()
        )
        .is_err()
    );
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(d.undo_label(), history.as_deref());
    assert!(d.is_selected(point));
    assert!(
        !r.object_selection_prompt("MeshToNURB")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert!(
        r.execute(&mut d, "MeshToNURB TrimTriangularFaces=Yes")
            .is_err()
    );
    assert!(
        !r.object_selection_prompt("MeshToNURB")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
}

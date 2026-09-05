use super::*;

fn sources(document: &mut Document) -> Vec<ObjectId> {
    let registry = CommandRegistry::with_builtins();
    for command in ["Line 0,0,0 1,0,0", "Line 0,0,3 2,0,3", "Line 0,0,5 1,0,5"] {
        registry.execute(document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    ids
}

#[test]
fn loft_uses_selection_order_and_fresh_attributes_with_undo_redo() {
    let registry = CommandRegistry::with_builtins();
    for closed in [false, true] {
        let mut document = Document::default();
        let ids = sources(&mut document);
        registry.execute(&mut document, "Group Profiles").unwrap();
        registry
            .execute(&mut document, "SetObjectName Input")
            .unwrap();
        registry.execute(&mut document, "Layer New Output").unwrap();
        registry
            .execute(&mut document, "Layer Current Output")
            .unwrap();
        document.clear_selection();
        for id in [ids[2], ids[0], ids[1]] {
            document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(
                &mut document,
                if closed {
                    "Loft Type=Tight Closed=Yes"
                } else {
                    "Loft Type=Tight"
                },
            )
            .unwrap();
        let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("loft BRep");
        };
        let surface = brep.faces()[0].surface();
        assert_eq!(
            output.attributes(),
            &ObjectAttributes::on_layer(document.current_layer_id())
        );
        assert!(
            document
                .groups()
                .all(|g| !g.members().any(|id| id == output.id()))
        );
        let first = before
            .iter()
            .find(|o| o.id() == ids[2])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap();
        assert!(
            surface
                .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
                .unwrap()
                .distance_to(first)
                .unwrap()
                < 1e-12
        );
        assert_eq!(document.objects().count(), 4);
        assert_eq!(document.selected_object_count(), 3);
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![ids[2], ids[0], ids[1]]
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn invalid_lofts_are_atomic_and_do_not_add_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = sources(&mut document);
    for command in [
        "Loft Type=Wrong",
        "Loft Closed=Maybe",
        "Loft Type=Loose Type=Normal",
        "Loft DeleteInput=Yes Closed=Yes Closed=No",
        "Loft 1,2,3",
        "Loft Rebuild=10",
    ] {
        let before = document.objects().cloned().collect::<Vec<_>>();
        let selection = document.selected_object_ids().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }
    document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    assert!(
        registry
            .execute(&mut document, "Loft DeleteInput=Yes")
            .is_err()
    );
    assert_eq!(document.objects().count(), 3);
    let p = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([ids[0], ids[1], p], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Loft"),
        Err(CommandError::LoftRequiresCurves)
    ));
    assert_eq!(document.objects().count(), 4);
}

use super::*;

fn sources(document: &mut Document, n: usize) -> Vec<ObjectId> {
    let corners = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 1.0],
        [4.0, 3.0, 2.0],
        [0.0, 3.0, -1.0],
    ];
    let middle = [
        [2.0, -1.0, 2.0],
        [5.0, 1.5, 3.0],
        [2.0, 4.0, -1.0],
        [-1.0, 1.5, 2.0],
    ];
    (0..n)
        .map(|i| {
            document
                .add_geometry(Geometry::NurbsCurve(
                    NurbsCurve::try_new(
                        2,
                        [corners[i], middle[i], corners[(i + 1) % 4]]
                            .into_iter()
                            .map(|p| Point3::try_from(p).unwrap())
                            .collect(),
                        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    )
                    .unwrap(),
                ))
                .unwrap()
        })
        .collect()
}

#[test]
fn edge_surface_preserves_sources_and_selection_with_fresh_output_and_atomic_history() {
    let registry = CommandRegistry::with_builtins();
    for count in 2..=4 {
        let mut document = Document::default();
        let ids = sources(&mut document, count);
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Group Boundaries").unwrap();
        registry
            .execute(&mut document, "SetObjectName Source")
            .unwrap();
        registry.execute(&mut document, "Layer New Output").unwrap();
        registry
            .execute(&mut document, "Layer Current Output")
            .unwrap();
        if count == 4 {
            document.clear_selection();
            for i in [2, 0, 3, 1] {
                document
                    .select_objects_direct([ids[i]], SelectionMode::Add)
                    .unwrap();
            }
        }
        let before = document.objects().cloned().collect::<Vec<_>>();
        let selection = document.selected_object_ids().collect::<Vec<_>>();
        registry.execute(&mut document, "EdgeSrf").unwrap();
        let output = document.objects().find(|o| !ids.contains(&o.id())).unwrap();
        assert_eq!(
            output.attributes(),
            &ObjectAttributes::on_layer(document.current_layer_id())
        );
        assert!(!document.is_selected(output.id()));
        assert!(
            document
                .groups()
                .all(|g| !g.members().any(|id| id == output.id()))
        );
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("EdgeSrf output must have BRep topology");
        };
        assert_eq!(brep.faces().len(), 1);
        if count == 4 {
            let s = brep.faces()[0].surface();
            assert_eq!(
                s.evaluate(*s.domain_u().start(), *s.domain_v().start())
                    .unwrap(),
                Point3::try_new(4.0, 3.0, 2.0).unwrap()
            );
        }
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        for original in &before {
            assert_eq!(document.object(original.id()).unwrap(), original);
        }
        let after = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn invalid_edge_surface_requests_leave_geometry_selection_and_history_untouched() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = sources(&mut document, 4);
    registry
        .execute(&mut document, "Rectangle 0,0,0 1,1,0")
        .unwrap();
    let closed = document.objects().last().unwrap().id();
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    let point = document.objects().last().unwrap().id();
    for (selection, command) in [
        (vec![ids[0]], "EdgeSrf"),
        (vec![closed, ids[0]], "EdgeSrf"),
        (vec![point, ids[0]], "EdgeSrf"),
        (vec![ids[0], ids[1]], "EdgeSrf DeleteInput=Yes"),
        (vec![ids[0], ids[1]], "EdgeSrf 1,2,3"),
        (vec![], "EdgeSrf"),
    ] {
        document
            .select_objects_direct(selection, SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let selection = document.selected_object_ids().collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

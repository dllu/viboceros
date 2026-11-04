use super::*;
use viboceros_geometry::MeshFace;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_from([x, y, z]).unwrap()
}
fn fixture() -> (Document, ObjectId) {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 3., 0.)],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}

#[test]
fn scripted_edge_selectors_use_explicit_or_unique_selected_owner_and_keep_order() {
    for explicit in [false, true] {
        for selector in ["Edges=2,0,2", "Edge 2,0,2", "Edges=All"] {
            let (mut doc, id) = fixture();
            let r = CommandRegistry::with_builtins();
            if !explicit {
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            let selection = doc.selected_object_ids().collect::<Vec<_>>();
            let source = doc.object(id).unwrap().clone();
            let text = format!(
                "_ToNURBS {selector} {} DeleteInputObjects=Yes",
                if explicit {
                    format!("Object={id}")
                } else {
                    String::new()
                }
            );
            assert!(r.object_selection_prompt(&text).unwrap().is_none());
            r.execute(&mut doc, &text).unwrap();
            assert_eq!(doc.object(id), Some(&source));
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selection);
            let Geometry::Mesh(m) = source.geometry() else {
                panic!()
            };
            let points = m.topology_edge_points();
            let indices = if selector.ends_with("All") {
                vec![0, 1, 2]
            } else {
                vec![2, 0]
            };
            assert_eq!(doc.objects().len(), 1 + indices.len());
            for (o, i) in doc.objects().skip(1).zip(indices) {
                let Geometry::NurbsCurve(c) = o.geometry() else {
                    panic!()
                };
                assert_eq!(c.degree(), 1);
                assert_eq!(
                    c.control_points()
                        .iter()
                        .map(|p| p.point())
                        .collect::<Vec<_>>(),
                    points[i]
                );
                assert_eq!(
                    c.domain(),
                    0. ..=points[i][0].distance_to(points[i][1]).unwrap()
                );
                assert!(!doc.is_selected(o.id()));
            }
        }
    }
}

#[test]
fn invalid_scripted_or_typed_targets_preserve_redo_selection_and_preferences() {
    let (mut doc, id) = fixture();
    let r = CommandRegistry::with_builtins();
    let empty = CommandRegistry::default();
    assert!(
        empty
            .to_nurbs_target_confirmation(&doc, "ToNURBS", &[ToNurbsTarget::Edge(id, 0)])
            .is_err()
    );
    assert!(
        empty
            .execute_to_nurbs_targets(&mut doc, "ToNURBS", &[ToNurbsTarget::Edge(id, 0)], false)
            .is_err()
    );
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    for args in [
        "Edges=3",
        "Edges=-1",
        "Edges=0,",
        "Edges=",
        "Edges=0 Edge=1",
        "Edges=0 Object=missing",
        "Object=missing",
        "Edges=0 MeshOptions",
        "Edges=0 SubDOptions",
        "Edges=0 Faces=Unpacked",
    ] {
        assert!(
            r.execute(&mut doc, &format!("ToNURBS {args} DeleteInputObjects=Yes"))
                .is_err(),
            "{args}"
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(doc.undo_label(), history.as_deref());
        assert!(doc.can_redo());
    }
    for targets in [
        vec![],
        vec![ToNurbsTarget::Edge(id, 3)],
        vec![ToNurbsTarget::Edge(
            "00000000-0000-0000-0000-000000000000".parse().unwrap(),
            0,
        )],
        vec![ToNurbsTarget::Edge(id, 0); 100_001],
    ] {
        assert!(
            r.execute_to_nurbs_targets(&mut doc, "ToNURBS DeleteInputObjects=Yes", &targets, false)
                .is_err()
        );
        assert!(doc.can_redo());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
    assert!(
        !r.object_selection_prompt("ToNURBS")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    let other = doc.add_geometry(Geometry::Point(p(10., 0., 0.))).unwrap();
    doc.select_objects_direct([other], SelectionMode::Add)
        .unwrap();
    assert!(r.execute(&mut doc, "ToNURBS Edges=0").is_err());
    assert!(
        r.execute_to_nurbs_targets(&mut doc, "ToNURBS", &[ToNurbsTarget::Edge(other, 0)], false)
            .is_err()
    );
}

#[test]
fn mixed_edge_and_oversized_whole_conversion_is_atomic_after_edge_evaluation() {
    let (mut doc, id) = fixture();
    let r = CommandRegistry::with_builtins();
    let huge = TriangleMesh::try_new_faces(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 3., 0.)],
        vec![MeshFace::Triangle([0, 1, 2]); MAX_OUTPUT_CONTROLS / 4 + 1],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let other = doc.add_geometry(Geometry::Mesh(huge)).unwrap();
    let group = doc.add_group(Some("sources".into()), [id, other]).unwrap();
    doc.select_objects_direct([other], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    r.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        r.execute_to_nurbs_targets(
            &mut doc,
            "ToNURBS DeleteInputObjects=Yes",
            &[ToNurbsTarget::Edge(id, 0), ToNurbsTarget::Whole(other)],
            true
        ),
        Err(CommandError::TooManyNurbsConversionControls { .. })
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [other]);
    assert_eq!(doc.group(group).unwrap().members().count(), 2);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    assert!(
        !r.object_selection_prompt("ToNURBS")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
}

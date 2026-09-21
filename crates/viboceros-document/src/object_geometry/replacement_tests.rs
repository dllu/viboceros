use super::*;

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}

fn document() -> (Document, [ObjectId; 2]) {
    let mut doc = Document::default();
    let ids = [0., 1.].map(|x| doc.add_geometry(point(x)).unwrap());
    doc.add_group(Some("Both".into()), ids).unwrap();
    doc.set_objects_color(ids, Some(ColorRgb::BLACK)).unwrap();
    doc.select_objects_direct([ids[1], ids[0]], SelectionMode::Replace)
        .unwrap();
    doc.add_geometry(point(2.)).unwrap();
    doc.undo().unwrap();
    (doc, ids)
}

#[test]
fn explicit_equal_replacements_preserve_objects_and_exchange_history_selection() {
    for active in [false, true] {
        let (mut doc, ids) = document();
        let original = doc.clone();
        if active {
            doc.begin_transaction("caller").unwrap();
        }
        // Last duplicate wins, including when it makes a replacement equal.
        assert_eq!(
            doc.replace_object_geometries_with_history(
                [
                    (ids[1], point(8.)),
                    (ids[0], point(0.)),
                    (ids[1], point(1.))
                ],
                ReplacementHistory::EveryReplacement,
            )
            .unwrap(),
            2
        );
        assert_eq!(doc.objects, original.objects);
        assert_eq!(doc.groups, original.groups);
        assert_eq!(doc.selection_order, original.selection_order);
        if active {
            assert_eq!(doc.redo_label(), original.redo_label());
            doc.commit_transaction().unwrap();
        }
        assert_eq!(
            doc.undo_label(),
            Some(if active {
                "caller"
            } else {
                "Replace object geometry"
            })
        );
        assert_eq!(doc.redo_label(), None);
        doc.clear_selection();
        assert_eq!(doc.select_last_changed(true), 2);
        assert_eq!(doc.selection, ids.into_iter().collect());
        doc.clear_selection();
        doc.undo().unwrap();
        assert_eq!(doc.objects, original.objects);
        assert_eq!(doc.groups, original.groups);
        assert_eq!(doc.selection, original.selection);
        doc.redo().unwrap();
        assert_eq!(doc.objects, original.objects);
        assert_eq!(doc.groups, original.groups);
        assert!(doc.selection.is_empty());
    }
}

#[test]
fn equal_replacement_rollback_and_empty_batches_preserve_complete_state() {
    for policy in [
        ReplacementHistory::ChangesOnly,
        ReplacementHistory::EveryReplacement,
    ] {
        let (mut doc, ids) = document();
        let before = format!("{doc:?}");
        assert_eq!(
            doc.replace_object_geometries_with_history([], policy)
                .unwrap(),
            0
        );
        assert_eq!(format!("{doc:?}"), before);
        doc.begin_transaction("rollback").unwrap();
        doc.replace_object_geometries_with_history([(ids[0], point(0.))], policy)
            .unwrap();
        doc.clear_selection();
        doc.rollback_transaction().unwrap();
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn replacement_validation_precedes_every_mutation_even_for_equal_geometry() {
    for policy in [
        ReplacementHistory::ChangesOnly,
        ReplacementHistory::EveryReplacement,
    ] {
        for active in [false, true] {
            for unknown in [false, true] {
                let (mut doc, ids) = document();
                let bad = if unknown {
                    ObjectId::new()
                } else {
                    doc.set_objects_locked([ids[1]], true).unwrap();
                    ids[1]
                };
                if active {
                    doc.begin_transaction("caller").unwrap();
                }
                let before = format!("{doc:?}");
                assert!(
                    doc.replace_object_geometries_with_history(
                        [(ids[0], point(9.)), (bad, point(1.))],
                        policy
                    )
                    .is_err()
                );
                assert_eq!(format!("{doc:?}"), before);
            }
        }
    }
}

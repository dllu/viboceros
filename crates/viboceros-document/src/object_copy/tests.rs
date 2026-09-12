use super::*;

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}

#[test]
fn repeated_source_pieces_keep_input_order_metadata_and_exact_history() {
    let mut document = Document::default();
    let ids = (0..20)
        .map(|x| document.add_geometry(point(f64::from(x))).unwrap())
        .collect::<Vec<_>>();
    let sources = [ids[3], ids[17]];
    let group = document.add_group(None, sources).unwrap();
    document.add_group(None, [sources[1]]).unwrap();
    // Keep the shared group last so picking source 0 selects the restricted peer.
    let memberships = document
        .object(sources[1])
        .unwrap()
        .group_ids()
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>();
    document
        .set_object_group_memberships(sources[1], memberships)
        .unwrap();
    let layer = document
        .add_layer("Locked source", ColorRgb::BLACK)
        .unwrap();
    document.set_objects_layer([sources[1]], layer).unwrap();
    document.set_layer_locked(layer, true).unwrap();
    document
        .select_object(sources[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(document.selected_object_count(), 2);
    let before_objects = document.objects.clone();
    let before_groups = document.groups.clone();
    let before_selection = document.selection.clone();
    let pieces = (0..128)
        .map(|index| {
            (
                sources[usize::from(index % 3 != 1)],
                point(f64::from(index) + 100.),
            )
        })
        .collect::<Vec<_>>();
    let copied = document
        .copy_object_pieces_into_source_groups(pieces.clone())
        .unwrap();
    assert_eq!(copied.len(), pieces.len());
    for (id, (source, geometry)) in copied.iter().zip(&pieces) {
        let object = document.object(*id).unwrap();
        let source = document.object(*source).unwrap();
        assert_eq!(object.geometry(), geometry);
        assert_eq!(object.attributes(), source.attributes());
        assert_eq!(object.group_ids(), source.group_ids());
        assert!(!document.is_selected(*id));
        assert!(object.group_ids().contains(&group));
    }
    assert_eq!(document.selection, before_selection);
    let after_objects = document.objects.clone();
    let after_groups = document.groups.clone();
    document.undo().unwrap();
    assert_eq!(document.objects, before_objects);
    assert_eq!(document.groups, before_groups);
    assert_eq!(document.selection, before_selection);
    document.redo().unwrap();
    assert_eq!(document.objects, after_objects);
    assert_eq!(document.groups, after_groups);
    assert_eq!(document.selection, before_selection);
}

#[test]
fn piece_copy_validates_all_sources_before_mutation_and_preserves_empty_noops() {
    for active in [false, true] {
        for failure in 0..3 {
            let mut document = Document::default();
            let first = document.add_geometry(point(0.)).unwrap();
            let last = document.add_geometry(point(1.)).unwrap();
            let group = document.add_group(None, [first, last]).unwrap();
            if failure == 1 {
                document.set_objects_locked([last], true).unwrap();
            }
            document.add_geometry(point(99.)).unwrap();
            document.undo().unwrap();
            if failure == 2 {
                document.objects[1].group_ids.push(group);
            }
            if active {
                document.begin_transaction("Caller").unwrap();
            }
            let before = format!("{document:?}");
            assert!(
                document
                    .copy_object_pieces_into_source_groups([])
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(format!("{document:?}"), before);
            let target = if failure == 0 { ObjectId::new() } else { last };
            let result = document.copy_object_pieces_into_source_groups([
                (first, point(10.)),
                (first, point(11.)),
                (target, point(12.)),
            ]);
            assert!(result.is_err());
            assert_eq!(format!("{document:?}"), before);
        }
    }
}

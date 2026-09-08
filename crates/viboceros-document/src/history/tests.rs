//! Exercise rollback against complete document/history state, not just counts.
use crate::*;
use viboceros_geometry::Vector3;

#[test]
fn mixed_edit_prefixes_roll_back_exactly_including_selection_and_redo() {
    let mut base = Document::default();
    let ids = (0..4)
        .map(|i| {
            base.add_geometry(Geometry::Point(
                Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    base.add_group(None, [ids[0], ids[1]]).unwrap();
    base.add_group(None, [ids[1], ids[2]]).unwrap();
    base.set_objects_locked([ids[1]], true).unwrap();
    base.select_object(ids[3], SelectionMode::Replace).unwrap();
    base.select_object(ids[0], SelectionMode::Replace).unwrap();
    base.add_geometry(Geometry::Point(Point3::try_new(99.0, 0.0, 0.0).unwrap()))
        .unwrap();
    base.undo().unwrap(); // Failed edits must not destroy an existing redo branch.
    let before = format!("{base:?}");
    let mut covered = 0u32;
    let mut succeeded = 0u32;
    let mut rejected = 0;
    for seed in 0..256u64 {
        let mut document = base.clone();
        let mut random = seed + 1;
        document.begin_transaction("Aborted mixed edit").unwrap();
        for step in 0..32 {
            random = random
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let choice = (random >> 32) as usize;
            let objects = document.objects().map(Object::id).collect::<Vec<_>>();
            let layers = document.layers().map(Layer::id).collect::<Vec<_>>();
            let kind = if objects.is_empty() { 0 } else { choice % 16 };
            covered |= 1 << kind;
            let id = objects.get((choice / 16) % objects.len().max(1)).copied();
            let layer = layers[(choice / 256) % layers.len()];
            let flag = choice & 256 != 0;
            let result = match kind {
                0 => document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(step as f64, 1.0, 0.0).unwrap(),
                    ))
                    .map(|_| ()),
                1 => document.delete_object(id.unwrap()),
                2 => document
                    .transform_objects(
                        [id.unwrap()],
                        AffineTransform3::from_translation(
                            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
                        ),
                    )
                    .map(|_| ()),
                3 => document
                    .set_objects_visibility([id.unwrap()], flag)
                    .map(|_| ()),
                4 => document.set_objects_locked([id.unwrap()], flag).map(|_| ()),
                5 => document
                    .set_objects_color([id.unwrap()], Some(ColorRgb::new(12, 34, 56)))
                    .map(|_| ()),
                6 => document
                    .add_layer(format!("Temporary {step}"), ColorRgb::BLACK)
                    .map(|_| ()),
                7 => document.set_objects_layer([id.unwrap()], layer).map(|_| ()),
                8 => document.set_layer_visibility(layer, flag).map(|_| ()),
                9 => document.add_empty_group(None).map(|_| ()),
                10 => {
                    let groups = document
                        .groups()
                        .take(choice % 3)
                        .map(Group::id)
                        .collect::<Vec<_>>();
                    document
                        .set_object_group_memberships(id.unwrap(), groups)
                        .map(|_| ())
                }
                11 => {
                    document.clear_objects();
                    Ok(())
                }
                12 => document
                    .select_objects_direct([id.unwrap()], SelectionMode::Replace)
                    .map(|_| ()),
                13 => {
                    document.select_previous(flag);
                    Ok(())
                }
                14 => {
                    document.select_last_changed(flag);
                    Ok(())
                }
                15 => document.set_current_layer(layer),
                _ => unreachable!(),
            };
            if result.is_err() {
                if !document.history.active.as_ref().unwrap().edits.is_empty() {
                    rejected += 1;
                }
                break;
            }
            succeeded |= 1 << kind;
        }
        document.rollback_transaction().unwrap();
        // Debug includes geometry, ordered memberships, all selection memories,
        // pending state, and both history stacks, without pointer addresses.
        assert_eq!(format!("{document:?}"), before, "rollback seed {seed}");
    }
    assert_eq!(covered, 0xffff);
    assert_eq!(succeeded, 0xffff);
    assert!(
        rejected > 0,
        "the matrix must include rejected partial edits"
    );
}

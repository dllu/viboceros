//! Temporary object-table lookup for atomic bulk edits.
use super::*;

impl Document {
    /// Resolve unique IDs in object-table order before staging any edits.
    /// Report the lowest missing ID before editability or geometry errors.
    /// Indices are temporary: callers must not reorder/remove objects while
    /// using them. No persistent document index or geometry clones are needed.
    pub(super) fn resolve_object_indices(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<Vec<usize>, DocumentError> {
        let mut remaining = ids.into_iter().collect::<BTreeSet<_>>();
        let mut indices = Vec::with_capacity(remaining.len());
        if remaining.is_empty() {
            return Ok(indices);
        }
        for (index, object) in self.objects.iter().enumerate() {
            if remaining.remove(&object.id) {
                indices.push(index);
                if remaining.is_empty() {
                    return Ok(indices);
                }
            }
        }
        Err(DocumentError::ObjectNotFound(*remaining.first().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_preflight_late_membership_corruption() {
        for corruption in 0..3 {
            for active in [false, true] {
                for operation in 0..5 {
                    let mut document = Document::default();
                    let geometry = Geometry::Point(Point3::try_new(1., 2., 3.).unwrap());
                    let ids = [0, 1].map(|_| document.add_geometry(geometry.clone()).unwrap());
                    let group = document.add_group(None, ids).unwrap();
                    document.add_geometry(geometry.clone()).unwrap();
                    document.undo().unwrap();
                    if active {
                        document.begin_transaction("caller").unwrap();
                        document.add_geometry(geometry.clone()).unwrap();
                    }
                    match corruption {
                        0 => document.objects[1].group_ids.push(GroupId::new()),
                        1 => document.objects[1].group_ids.push(group),
                        _ => {
                            document.groups[0].members.remove(&ids[1]);
                        }
                    }
                    let before = format!("{document:?}");
                    let copies = ids.map(|id| (id, geometry.clone()));
                    let result = match operation {
                        0 => document.copy_object_geometries_into_source_groups(copies),
                        1 => document.copy_object_geometries_into_source_groups_in_order(copies),
                        2 | 3 => document.copy_objects_with_transforms_and_groups(
                            ids,
                            &[AffineTransform3::identity(); 2],
                            if operation == 2 {
                                CopyGroupPolicy::Preserve
                            } else {
                                CopyGroupPolicy::DefinitionsOnly
                            },
                        ),
                        _ => document.copy_objects_morphed(ids, &IdentityMorph),
                    };
                    assert!(result.is_err());
                    assert_eq!(format!("{document:?}"), before);
                }
            }
        }
    }

    #[test]
    fn replacement_copy_order_duplicates_and_memberships_replay_exactly() {
        for ordered in [false, true] {
            let mut document = Document::default();
            let point = |x| Geometry::Point(Point3::try_new(x, 0., 0.).unwrap());
            let ids = [0., 1., 2.].map(|x| document.add_geometry(point(x)).unwrap());
            document.add_group(None, [ids[0], ids[2]]).unwrap();
            document.add_group(None, [ids[2]]).unwrap();
            document
                .select_objects_direct([ids[2]], SelectionMode::Replace)
                .unwrap();
            let originals = document.objects.clone();
            let groups = document.groups.clone();
            let input = [
                (ids[0], point(9.)),
                (ids[2], point(12.)),
                (ids[1], point(11.)),
                (ids[0], point(10.)),
            ];
            let copies = if ordered {
                document.copy_object_geometries_into_source_groups_in_order(input)
            } else {
                document.copy_object_geometries_into_source_groups(input)
            }
            .unwrap();
            let order = if ordered { [2, 1, 0] } else { [0, 1, 2] };
            for (copy, source) in copies.iter().zip(order) {
                let object = document.object(*copy).unwrap();
                assert_eq!(object.geometry, point(10. + source as f64));
                assert_eq!(object.group_ids, originals[source].group_ids);
                assert_eq!(object.attributes, originals[source].attributes);
            }
            assert_eq!(document.selection, BTreeSet::from([ids[2]]));
            let after_objects = document.objects.clone();
            let after_groups = document.groups.clone();
            document.undo().unwrap();
            assert_eq!(document.objects, originals);
            assert_eq!(document.groups, groups);
            document.redo().unwrap();
            assert_eq!(document.objects, after_objects);
            assert_eq!(document.groups, after_groups);
        }
    }

    #[test]
    fn late_geometry_overflow_preserves_groups_redo_and_caller_transactions() {
        for copy in [false, true] {
            for active in [false, true] {
                let mut document = Document::default();
                let ids = [1., f64::MAX].map(|x| {
                    document
                        .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                        .unwrap()
                });
                document.add_group(Some("Both".into()), ids).unwrap();
                document.add_group(Some("Last".into()), [ids[1]]).unwrap();
                document
                    .select_objects_direct(ids, SelectionMode::Replace)
                    .unwrap();
                document
                    .add_geometry(Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()))
                    .unwrap();
                document.undo().unwrap();
                if active {
                    document.begin_transaction("caller").unwrap();
                    document
                        .add_geometry(Geometry::Point(Point3::try_new(3., 0., 0.).unwrap()))
                        .unwrap();
                }
                let before = format!("{document:?}");
                let transform = AffineTransform3::from_translation(
                    viboceros_geometry::Vector3::try_new(f64::MAX, 0., 0.).unwrap(),
                );
                // The first source transforms successfully; only the later
                // source overflows. Copy also stages an earlier valid instance.
                if copy {
                    assert!(
                        document
                            .copy_objects_with_transforms(
                                ids,
                                &[AffineTransform3::identity(), transform]
                            )
                            .is_err()
                    );
                } else {
                    assert!(document.transform_objects(ids, transform).is_err());
                }
                assert_eq!(
                    format!("{document:?}"),
                    before,
                    "copy={copy}, active={active}"
                );
            }
        }
    }

    struct IdentityMorph;
    impl PointMorph for IdentityMorph {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Ok(point)
        }
    }

    #[test]
    fn resolution_is_unique_ordered_read_only_and_reports_lowest_missing_id() {
        let mut document = Document::default();
        let ids = (0..32)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let before = format!("{document:?}");
        for mask in 0..256 {
            let indices = (0..8)
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| i * 4)
                .collect::<Vec<_>>();
            let request = indices
                .iter()
                .rev()
                .flat_map(|i| [ids[*i], ids[*i]])
                .collect::<Vec<_>>();
            assert_eq!(
                document.resolve_object_indices(request.clone()).unwrap(),
                indices
            );
            let missing = [ObjectId::new(), ObjectId::new()];
            assert_eq!(
                document.resolve_object_indices(request.into_iter().chain(missing)),
                Err(DocumentError::ObjectNotFound(
                    *missing.iter().min().unwrap()
                ))
            );
        }
        assert_eq!(format!("{document:?}"), before);
    }

    #[test]
    fn bulk_edit_failures_preserve_document_and_redo_before_staging() {
        let mut fixture = Document::default();
        let point = Geometry::Point(Point3::try_new(1., 2., 3.).unwrap());
        let first = fixture.add_geometry(point.clone()).unwrap();
        let locked = fixture.add_geometry(point.clone()).unwrap();
        fixture.set_objects_locked([locked], true).unwrap();
        fixture.add_geometry(point.clone()).unwrap();
        fixture.undo().unwrap();
        for operation in 0..7 {
            for with_missing in [false, true] {
                let mut document = fixture.clone();
                let mut ids = vec![locked, first, first];
                let missing = ObjectId::new();
                if with_missing {
                    ids.push(missing);
                }
                let before = format!("{document:?}");
                let transform = AffineTransform3::identity();
                let result = match operation {
                    0 => document.transform_objects(ids, transform),
                    1 => document
                        .copy_objects_transformed(ids, transform)
                        .map(|v| v.len()),
                    2 => document
                        .copy_objects_morphed(ids, &IdentityMorph)
                        .map(|v| v.len()),
                    3 => document.morph_objects(ids, &IdentityMorph),
                    4 => document
                        .replace_object_geometries(ids.into_iter().map(|id| (id, point.clone()))),
                    5 => document
                        .copy_object_geometries_into_source_groups(
                            ids.into_iter().map(|id| (id, point.clone())),
                        )
                        .map(|v| v.len()),
                    _ => document.set_objects_color(ids, Some(ColorRgb::BLACK)),
                };
                assert_eq!(
                    result,
                    Err(if with_missing {
                        DocumentError::ObjectNotFound(missing)
                    } else {
                        DocumentError::ObjectLocked(locked)
                    }),
                    "operation={operation}"
                );
                assert_eq!(format!("{document:?}"), before, "operation={operation}");
            }
        }
    }

    #[test]
    fn mixed_group_copies_retain_memberships_and_replay_exactly() {
        let mut document = Document::default();
        let ids = (0..4)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document.add_group(None, [ids[1], ids[2]]).unwrap();
        let original_objects = document.objects.clone();
        let original_groups = document.groups.clone();
        let copies = document
            .copy_objects_transformed(ids.iter().rev().copied(), AffineTransform3::identity())
            .unwrap();
        assert!(document.object(copies[0]).unwrap().group_ids.is_empty());
        assert!(document.object(copies[3]).unwrap().group_ids.is_empty());
        let group = document.object(copies[1]).unwrap().group_ids[0];
        assert_eq!(document.object(copies[2]).unwrap().group_ids, [group]);
        assert_eq!(
            document.group(group).unwrap().members,
            [copies[1], copies[2]].into_iter().collect()
        );
        let copied_objects = document.objects.clone();
        let copied_groups = document.groups.clone();
        document.undo().unwrap();
        assert_eq!(document.objects, original_objects);
        assert_eq!(document.groups, original_groups);
        document.redo().unwrap();
        assert_eq!(document.objects, copied_objects);
        assert_eq!(document.groups, copied_groups);
    }

    #[test]
    #[ignore = "manual bulk transform timing"]
    fn benchmark_bulk_transform_and_copy() {
        let mut document = Document::default();
        document.begin_transaction("fixture").unwrap();
        let ids = (0..20_000)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document.commit_transaction().unwrap();
        let transform = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(1., 2., 3.).unwrap(),
        );
        let start = std::time::Instant::now();
        assert_eq!(
            document
                .transform_objects(ids.iter().copied(), transform)
                .unwrap(),
            ids.len()
        );
        eprintln!("20k objects, transform: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        let copies = document.copy_objects_transformed(ids, transform).unwrap();
        eprintln!("20k objects, copy: {:?}", start.elapsed());
        assert_eq!(copies.len(), 20_000);
        for (i, object) in document.objects.iter().enumerate() {
            let (x, y, z) = if i < 20_000 {
                (i as f64 + 1., 2., 3.)
            } else {
                ((i - 20_000) as f64 + 2., 4., 6.)
            };
            assert_eq!(
                object.geometry,
                Geometry::Point(Point3::try_new(x, y, z).unwrap())
            );
        }
    }
}

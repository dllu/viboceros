//! Geometry-only staging and a shared, identity-preserving history commit.

use super::*;

impl Document {
    pub(super) fn stage_object_geometries(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
        mut operation: impl FnMut(&Geometry) -> Result<Geometry, GeometryError>,
    ) -> Result<Vec<(usize, Geometry)>, DocumentError> {
        let indices = self.resolve_object_indices(ids)?;
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        indices
            .into_iter()
            .map(|index| Ok((index, operation(&self.objects[index].geometry)?)))
            .collect()
    }

    // Indices must be unique and validated for editability, and the object
    // table must not change between staging and commit. No geometry operation
    // is allowed here: all fallible geometry work precedes document mutation.
    pub(super) fn commit_object_geometries(
        &mut self,
        mut staged: Vec<(usize, Geometry)>,
        transaction_label: &'static str,
        edit_label: &'static str,
    ) -> Result<usize, DocumentError> {
        staged.retain(|(index, geometry)| self.objects[*index].geometry != *geometry);
        if staged.is_empty() {
            return Ok(0);
        }
        let count = staged.len();
        let owns_transaction = self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction(transaction_label)?;
        }
        for (index, geometry) in staged {
            let source = &self.objects[index];
            let after = Object {
                id: source.id,
                geometry,
                attributes: source.attributes.clone(),
                isolation: source.isolation,
                group_ids: source.group_ids.clone(),
            };
            let id = after.id;
            // Move the old object into history; only the new geometry needs
            // a clone for the separate live-document and redo states.
            let before = std::mem::replace(&mut self.objects[index], after.clone());
            self.record_edit(
                edit_label,
                Edit::ObjectChanged {
                    id,
                    selected: self.is_selected(id),
                    states: Box::new([before, after]),
                },
            );
        }
        if owns_transaction {
            self.commit_transaction()?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scale(f64);

    impl PointMorph for Scale {
        fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(point.x() * self.0, point.y() * self.0, point.z() * self.0)
        }
    }

    #[test]
    fn geometry_edit_paths_preserve_noops_metadata_and_history() {
        for operation in 0..3 {
            for active in [false, true] {
                let mut document = Document::default();
                let ids = [0., 1.].map(|x| {
                    document
                        .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                        .unwrap()
                });
                document.add_group(Some("Both".into()), ids).unwrap();
                document.add_group(None, [ids[1]]).unwrap();
                document
                    .set_objects_color(ids, Some(ColorRgb::BLACK))
                    .unwrap();
                document
                    .select_objects_direct([ids[1], ids[0]], SelectionMode::Replace)
                    .unwrap();
                document
                    .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
                    .unwrap();
                document.undo().unwrap();
                if active {
                    document.begin_transaction("caller").unwrap();
                }
                let original = document.clone();
                for factor in [1., 2.] {
                    let request = [ids[1], ids[0], ids[1]];
                    let changed = match operation {
                        0 => document.transform_objects(
                            request,
                            AffineTransform3::try_uniform_scale(
                                Point3::try_new(0., 0., 0.).unwrap(),
                                factor,
                            )
                            .unwrap(),
                        ),
                        1 => document.morph_objects(request, &Scale(factor)),
                        _ => document.replace_object_geometries(request.map(|id| {
                            let x = if id == ids[0] { 0. } else { factor };
                            (id, Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                        })),
                    }
                    .unwrap();
                    assert_eq!(changed, usize::from(factor == 2.));
                    if factor == 1. {
                        assert_eq!(format!("{document:?}"), format!("{original:?}"));
                    }
                }
                let mut expected = original.objects.clone();
                expected[1].geometry = Geometry::Point(Point3::try_new(2., 0., 0.).unwrap());
                assert_eq!(document.objects, expected);
                assert_eq!(document.groups, original.groups);
                assert_eq!(document.selection_order, original.selection_order);
                if active {
                    document.commit_transaction().unwrap();
                }
                document.undo().unwrap();
                assert_eq!(document.objects, original.objects);
                assert_eq!(document.groups, original.groups);
                assert_eq!(document.selection, original.selection);
                document.redo().unwrap();
                assert_eq!(document.objects, expected);
                assert_eq!(document.groups, original.groups);
                assert_eq!(document.selection_order, original.selection_order);
            }
        }
    }

    struct UnreachableMorph;

    #[test]
    fn morph_copies_visit_unique_sources_in_table_order_and_fail_before_mutation() {
        struct RecordingMorph(std::cell::RefCell<Vec<Point3>>);
        impl PointMorph for RecordingMorph {
            fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
                self.0.borrow_mut().push(point);
                Point3::try_new(point.x() * 2., point.y(), point.z())
            }
        }

        for fail in [false, true] {
            for active in [false, true] {
                let mut document = Document::default();
                let points = [1., if fail { f64::MAX } else { 2. }]
                    .map(|x| Point3::try_new(x, 0., 0.).unwrap());
                let ids =
                    points.map(|point| document.add_geometry(Geometry::Point(point)).unwrap());
                document.add_group(Some("Both".into()), ids).unwrap();
                document.add_group(None, [ids[1]]).unwrap();
                document
                    .select_objects_direct(ids, SelectionMode::Replace)
                    .unwrap();
                document.add_geometry(Geometry::Point(points[0])).unwrap();
                document.undo().unwrap();
                let original_objects = document.objects.clone();
                let original_groups = document.groups.clone();
                if active {
                    document.begin_transaction("caller").unwrap();
                    document.add_geometry(Geometry::Point(points[0])).unwrap();
                }
                let before = format!("{document:?}");
                let morph = RecordingMorph(Default::default());
                let result = document.copy_objects_morphed([ids[1], ids[0], ids[1]], &morph);
                assert_eq!(*morph.0.borrow(), points);
                if fail {
                    assert!(result.is_err());
                    assert_eq!(format!("{document:?}"), before);
                } else {
                    let copies = result.unwrap();
                    assert_eq!(copies.len(), 2);
                    for (index, id) in copies.iter().enumerate() {
                        let object = document.object(*id).unwrap();
                        assert_eq!(
                            object.geometry,
                            Geometry::Point(
                                Point3::try_new(2. * (index + 1) as f64, 0., 0.).unwrap()
                            )
                        );
                        assert_eq!(object.group_ids.len(), index + 1);
                    }
                    let objects = document.objects.clone();
                    let groups = document.groups.clone();
                    if active {
                        document.commit_transaction().unwrap();
                    }
                    document.undo().unwrap();
                    assert_eq!(document.objects, original_objects);
                    assert_eq!(document.groups, original_groups);
                    document.redo().unwrap();
                    assert_eq!(document.objects, objects);
                    assert_eq!(document.groups, groups);
                    assert_eq!(document.selection, copies.into_iter().collect());
                }
            }
        }
    }

    impl PointMorph for UnreachableMorph {
        fn morph_point(&self, _: Point3) -> Result<Point3, GeometryError> {
            panic!("editability must be checked before invoking geometry operations");
        }
    }

    #[test]
    fn late_locked_source_precedes_geometry_work() {
        let mut document = Document::default();
        let ids = [f64::MAX, 1.].map(|x| {
            document
                .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                .unwrap()
        });
        document.set_objects_locked([ids[1]], true).unwrap();
        let before = format!("{document:?}");
        let transform = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(f64::MAX, 0., 0.).unwrap(),
        );
        assert_eq!(
            document.transform_objects(ids, transform),
            Err(DocumentError::ObjectLocked(ids[1]))
        );
        assert_eq!(
            document.morph_objects(ids, &UnreachableMorph),
            Err(DocumentError::ObjectLocked(ids[1]))
        );
        assert_eq!(
            document.copy_objects_morphed(ids, &UnreachableMorph),
            Err(DocumentError::ObjectLocked(ids[1]))
        );
        assert_eq!(format!("{document:?}"), before);
    }
}

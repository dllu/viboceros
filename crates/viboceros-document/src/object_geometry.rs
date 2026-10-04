//! Geometry-only staging and a shared, identity-preserving history commit.

use super::*;

#[cfg(test)]
mod replacement_tests;

/// Whether assigning equal geometry represents an undoable object replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementHistory {
    /// Geometric edits do not consume undo history for unchanged objects.
    ChangesOnly,
    /// Record every requested replacement, including equal geometry. This also
    /// updates selection recall and clears the redo branch on commit.
    EveryReplacement,
}

impl Document {
    /// Apply a separate affine map to each object, staging the entire edit
    /// before mutation. Duplicate IDs use their last supplied map. Geometry
    /// metadata and existing representations follow ordinary affine edits.
    pub fn transform_objects_individually(
        &mut self,
        transforms: impl IntoIterator<Item = (ObjectId, AffineTransform3)>,
        history: ReplacementHistory,
    ) -> Result<usize, DocumentError> {
        let transforms = transforms.into_iter().collect::<BTreeMap<_, _>>();
        let indices = self.resolve_object_indices(transforms.keys().copied())?;
        for &index in &indices {
            self.ensure_object_editable(&self.objects[index])?;
        }
        let staged = indices
            .into_iter()
            .map(|index| {
                let object = &self.objects[index];
                Ok((
                    index,
                    object
                        .geometry
                        .transformed_for_edit(transforms[&object.id], self.tolerance)?,
                ))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        self.commit_object_geometries_with_control_selection(staged, history)
    }

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
        staged: Vec<(usize, Geometry)>,
        transaction_label: &'static str,
        edit_label: &'static str,
        history: ReplacementHistory,
        preserve_geometry_user_text: bool,
    ) -> Result<usize, DocumentError> {
        self.commit_object_geometries_with_text_policy(
            staged,
            transaction_label,
            edit_label,
            history,
            |_, _| preserve_geometry_user_text,
        )
    }

    pub(super) fn commit_object_geometries_with_text_policy(
        &mut self,
        mut staged: Vec<(usize, Geometry)>,
        transaction_label: &'static str,
        edit_label: &'static str,
        history: ReplacementHistory,
        mut preserve_geometry_user_text: impl FnMut(&Object, &Geometry) -> bool,
    ) -> Result<usize, DocumentError> {
        if history == ReplacementHistory::ChangesOnly {
            staged.retain(|(index, geometry)| *self.objects[*index].geometry != *geometry);
        }
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
            let preserve_text = preserve_geometry_user_text(source, &geometry);
            let after = Object {
                id: source.id,
                geometry: geometry.into(),
                geometry_user_text: if preserve_text {
                    source.geometry_user_text.clone()
                } else {
                    BTreeMap::new()
                },
                attributes: source.attributes.clone(),
                isolation: source.isolation,
                group_ids: source.group_ids.clone(),
            };
            let id = after.id;
            // Move the old object into history; live and redo states share
            // the new immutable geometry without cloning its payload.
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
        self.synchronize_control_points();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformed_copies_retain_source_groups_metadata_selection_and_atomic_history() {
        let mut doc = Document::default();
        let source = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()))
            .unwrap();
        let group = doc
            .add_group(Some("source group".into()), [source])
            .unwrap();
        doc.set_object_user_text([source], "Code", Some("attribute"))
            .unwrap();
        doc.set_object_geometry_user_text([source], "Code", Some("geometry"))
            .unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        let translated = Geometry::Point(Point3::try_new(5., 0., 0.).unwrap());
        let before = format!("{doc:?}");
        assert!(
            doc.copy_object_geometries_in_source_groups([
                (source, translated.clone()),
                (ObjectId::new(), translated.clone())
            ])
            .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        let copy = doc
            .copy_object_geometries_in_source_groups([(source, translated.clone())])
            .unwrap()[0];
        assert!(doc.is_selected(source));
        assert!(!doc.is_selected(copy));
        let object = doc.object(copy).unwrap();
        assert_eq!(object.geometry(), &translated);
        assert_eq!(object.group_ids(), &[group]);
        assert_eq!(object.geometry_user_text()["Code"], "geometry");
        assert_eq!(object.attributes().user_text()["Code"], "attribute");
        assert_eq!(
            doc.group(group).unwrap().members().collect::<BTreeSet<_>>(),
            BTreeSet::from([source, copy])
        );
        doc.undo().unwrap();
        assert!(doc.object(copy).is_none());
        assert_eq!(doc.groups().count(), 1);
        assert!(doc.is_selected(source));
        doc.redo().unwrap();
        assert_eq!(
            doc.object(copy).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
        assert_eq!(doc.object(copy).unwrap().group_ids(), &[group]);
    }

    #[test]
    fn individual_transforms_stage_overflow_and_preserve_metadata_and_history() {
        let mut doc = Document::default();
        let a = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()))
            .unwrap();
        let b = doc
            .add_geometry(Geometry::Point(Point3::try_new(1e308, 0., 0.).unwrap()))
            .unwrap();
        doc.set_object_geometry_user_text([a], "Code", Some("geometry"))
            .unwrap();
        doc.set_object_user_text([a], "Code", Some("attribute"))
            .unwrap();
        let translation = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(3., 0., 0.).unwrap(),
        );
        let overflow = AffineTransform3::try_nonuniform_scale(
            Point3::try_new(0., 0., 0.).unwrap(),
            [4., 1., 1.],
        )
        .unwrap();
        let before = format!("{doc:?}");
        assert!(
            doc.transform_objects_individually(
                [(a, translation), (b, overflow)],
                ReplacementHistory::EveryReplacement
            )
            .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        assert_eq!(
            doc.transform_objects_individually(
                [
                    (a, overflow),
                    (a, translation),
                    (b, AffineTransform3::identity())
                ],
                ReplacementHistory::EveryReplacement
            )
            .unwrap(),
            2
        );
        assert_eq!(
            doc.object(a).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(5., 0., 0.).unwrap())
        );
        assert_eq!(
            doc.object(a).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
        assert_eq!(
            doc.object(a).unwrap().attributes().user_text()["Code"],
            "attribute"
        );
        doc.undo().unwrap();
        assert_eq!(
            doc.object(a).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(2., 0., 0.).unwrap())
        );
        doc.redo().unwrap();
        assert_eq!(
            doc.object(a).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(5., 0., 0.).unwrap())
        );
    }

    #[test]
    fn geometry_user_text_follows_transforms_and_copies_but_explicit_replacement_clears_it() {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        let before_invalid = format!("{document:?}");
        for (key, value) in [("", "x"), ("bad\0key", "x"), ("Code", "bad\0value")] {
            assert!(matches!(
                document.set_object_user_text([id], key, Some(value)),
                Err(DocumentError::InvalidUserText(_))
            ));
            assert!(matches!(
                document.set_object_geometry_user_text([id], key, Some(value)),
                Err(DocumentError::InvalidUserText(_))
            ));
        }
        let layer = document.current_layer_id();
        assert!(matches!(
            document.add_geometry_with_metadata(
                Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()),
                ObjectAttributes::on_layer(layer),
                BTreeMap::from([
                    ("Code".to_owned(), "one".to_owned()),
                    ("code".to_owned(), "two".to_owned()),
                ]),
            ),
            Err(DocumentError::InvalidUserText("duplicate key"))
        ));
        assert_eq!(format!("{document:?}"), before_invalid);
        document
            .set_object_user_text([id], "Code", Some("attribute"))
            .unwrap();
        document
            .set_object_geometry_user_text([id], "Code", Some("geometry"))
            .unwrap();
        let transform = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
        );
        document.transform_objects([id], transform).unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
        let copy = document.copy_objects_transformed([id], transform).unwrap()[0];
        assert_eq!(
            document.object(copy).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
        document
            .replace_object_geometries([(
                id,
                Geometry::Point(Point3::try_new(3., 0., 0.).unwrap()),
            )])
            .unwrap();
        assert!(document.object(id).unwrap().geometry_user_text().is_empty());
        assert_eq!(
            document.object(id).unwrap().attributes().user_text()["Code"],
            "attribute"
        );
        document.undo().unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
        document
            .set_object_geometry_user_text([id], "code", None)
            .unwrap();
        assert!(document.object(id).unwrap().geometry_user_text().is_empty());
        document.undo().unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
    }

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
                expected[1].geometry = Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()).into();
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
                            *object.geometry,
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

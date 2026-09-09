//! Geometry-free snapshots for object display and organization edits.

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ObjectProperties {
    pub id: ObjectId,
    pub attributes: ObjectAttributes,
    pub isolation: ObjectIsolation,
}

impl From<&Object> for ObjectProperties {
    fn from(object: &Object) -> Self {
        Self {
            id: object.id,
            attributes: object.attributes.clone(),
            isolation: object.isolation,
        }
    }
}

impl ObjectProperties {
    pub fn apply_to(&self, object: &mut Object) {
        debug_assert_eq!(self.id, object.id);
        object.attributes = self.attributes.clone();
        object.isolation = self.isolation;
    }
}

pub(super) fn replace(
    document: &mut Document,
    id: ObjectId,
    expected: &ObjectProperties,
    replacement: &ObjectProperties,
) -> Result<(), DocumentError> {
    if expected.id != id || replacement.id != id {
        return Err(DocumentError::HistoryInvariant(
            "changed property identity did not match",
        ));
    }
    let object = document
        .objects
        .iter_mut()
        .find(|object| object.id == id)
        .ok_or(DocumentError::HistoryInvariant(
            "changed property object was missing",
        ))?;
    if object.attributes != expected.attributes || object.isolation != expected.isolation {
        return Err(DocumentError::HistoryInvariant(
            "changed object properties did not match",
        ));
    }
    replacement.apply_to(object);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertices(document: &Document, id: ObjectId) -> *const Point3 {
        let Geometry::Polyline(curve) = document.object(id).unwrap().geometry() else {
            panic!("expected polyline");
        };
        curve.vertices().as_ptr()
    }

    #[test]
    fn property_edits_and_history_leave_geometry_allocation_untouched() {
        for operation in 0..5 {
            for transaction in 0..3 {
                let mut document = Document::default();
                let layer = document.add_layer("target", ColorRgb::BLACK).unwrap();
                let curve = Polyline3::try_new(
                    (0..10_000)
                        .map(|i| Point3::try_new(i as f64, 0., 0.).unwrap())
                        .collect(),
                    document.tolerance,
                )
                .unwrap();
                let id = document.add_geometry(Geometry::Polyline(curve)).unwrap();
                document.add_group(None, [id]).unwrap();
                document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
                let allocation = vertices(&document, id);
                let original = document.object(id).unwrap().clone();
                let groups = document.groups.clone();
                if transaction != 0 {
                    document.begin_transaction("caller").unwrap();
                }
                let changed = match operation {
                    0 => document.set_object_names([(id, Some("named".into()))]),
                    1 => document.set_objects_color([id], Some(ColorRgb::BLACK)),
                    2 => document.set_objects_visibility([id], false),
                    3 => document.set_objects_locked([id], true),
                    _ => document.set_objects_layer([id], layer),
                }
                .unwrap();
                assert_eq!(changed, 1);
                let after = document.object(id).unwrap().clone();
                assert_eq!(vertices(&document, id), allocation);
                if transaction == 2 {
                    document.rollback_transaction().unwrap();
                } else {
                    if transaction == 1 {
                        document.commit_transaction().unwrap();
                    }
                    assert!(matches!(
                        document.history.undo.last().unwrap().edits.as_slice(),
                        [Edit::ObjectPropertiesChanged { .. }]
                    ));
                    document.undo().unwrap();
                }
                assert_eq!(document.object(id).unwrap(), &original);
                assert_eq!(document.selection, BTreeSet::from([id]));
                assert_eq!(vertices(&document, id), allocation);
                if transaction != 2 {
                    document.redo().unwrap();
                    assert_eq!(document.object(id).unwrap(), &after);
                    assert_eq!(vertices(&document, id), allocation);
                }
                assert_eq!(document.groups, groups);
            }
        }
    }

    #[test]
    fn invalid_property_replay_is_read_only() {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
            .unwrap();
        let properties = ObjectProperties::from(document.object(id).unwrap());
        let before = format!("{document:?}");
        for fault in 0..4 {
            let mut expected = properties.clone();
            let mut replacement = properties.clone();
            let target = match fault {
                0 => {
                    expected.id = ObjectId::new();
                    id
                }
                1 => {
                    replacement.id = ObjectId::new();
                    id
                }
                2 => {
                    expected.attributes.name = Some("incorrect".into());
                    id
                }
                _ => {
                    let missing = ObjectId::new();
                    expected.id = missing;
                    replacement.id = missing;
                    missing
                }
            };
            assert!(replace(&mut document, target, &expected, &replacement).is_err());
            assert_eq!(format!("{document:?}"), before);
        }
    }
}

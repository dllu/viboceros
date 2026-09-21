use super::*;
use crate::{ColorRgb, Document, ObjectId, ReplacementHistory};
use viboceros_geometry::{AffineTransform3, GeometryError, LengthUnitSystem, Point3, PointMorph};

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 2., 3.).unwrap())
}

fn snapshot(document: &Document, id: ObjectId) -> GeometrySnapshot {
    document.object(id).unwrap().geometry_snapshot().clone()
}

#[test]
fn identity_is_distinct_from_value_equality_and_owns_its_lifetime() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<GeometrySnapshot>();
    let a = GeometrySnapshot::from(point(1.));
    let b = GeometrySnapshot::from(point(1.));
    assert_eq!(a, b);
    assert!(!a.shares_storage_with(&b));
    assert!(a.shares_storage_with(&a.clone()));
    let retained = {
        let mut document = Document::default();
        let id = document.add_geometry(point(7.)).unwrap();
        snapshot(&document, id)
    };
    assert_eq!(*retained, point(7.));
}

#[test]
fn replacement_history_and_branches_restore_exact_snapshot_identity() {
    let mut document = Document::default();
    let id = document.add_geometry(point(1.)).unwrap();
    let before = snapshot(&document, id);
    let mut branch = document.clone();
    assert!(snapshot(&branch, id).shares_storage_with(&before));
    assert_eq!(
        document
            .replace_object_geometries([(id, point(1.))])
            .unwrap(),
        0
    );
    assert!(snapshot(&document, id).shares_storage_with(&before));
    document
        .replace_object_geometries([(id, point(2.))])
        .unwrap();
    let after = snapshot(&document, id);
    assert!(!after.shares_storage_with(&before));
    assert_eq!(*before, point(1.));
    document.undo().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&before));
    document.redo().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&after));
    branch.replace_object_geometries([(id, point(3.))]).unwrap();
    assert_eq!(document.object(id).unwrap().geometry(), &point(2.));
    assert!(!snapshot(&branch, id).shares_storage_with(&after));
    document.begin_transaction("cancel").unwrap();
    document
        .replace_object_geometries([(id, point(4.))])
        .unwrap();
    document.rollback_transaction().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&after));
    document
        .replace_object_geometries_with_history(
            [(id, point(2.))],
            ReplacementHistory::EveryReplacement,
        )
        .unwrap();
    let equal = snapshot(&document, id);
    assert_eq!(equal, after);
    assert!(!equal.shares_storage_with(&after));
    document.undo().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&after));
    document.redo().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&equal));
}

struct Double;
impl PointMorph for Double {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(2. * p.x(), 2. * p.y(), 2. * p.z())
    }
}

#[test]
fn transforms_morphs_and_unit_changes_replace_without_mutating_snapshots() {
    for operation in 0..3 {
        let mut document = Document::default();
        let id = document.add_geometry(point(1.)).unwrap();
        let before = snapshot(&document, id);
        match operation {
            0 => {
                document
                    .transform_objects(
                        [id],
                        AffineTransform3::try_uniform_scale(
                            Point3::try_new(0., 0., 0.).unwrap(),
                            2.,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
            1 => {
                document.morph_objects([id], &Double).unwrap();
            }
            _ => {
                document.set_units(LengthUnitSystem::Meters, true).unwrap();
            }
        }
        let after = snapshot(&document, id);
        assert!(!before.shares_storage_with(&after));
        assert_ne!(before, after);
        assert_eq!(*before, point(1.));
        document.undo().unwrap();
        assert!(snapshot(&document, id).shares_storage_with(&before));
        document.redo().unwrap();
        assert!(snapshot(&document, id).shares_storage_with(&after));
    }
}

#[test]
fn metadata_edits_and_layer_copies_share_geometry_but_copies_edit_independently() {
    let mut document = Document::default();
    let id = document.add_geometry(point(1.)).unwrap();
    let before = snapshot(&document, id);
    let layer = document.add_layer("Copies", ColorRgb::BLACK).unwrap();
    document
        .set_objects_color([id], Some(ColorRgb::new(12, 34, 56)))
        .unwrap();
    document.set_objects_layer([id], layer).unwrap();
    document.set_objects_locked([id], true).unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&before));
    document.undo().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&before));
    let copies = document
        .copy_objects_to_layer([id], document.current_layer_id())
        .unwrap();
    assert!(snapshot(&document, copies[0]).shares_storage_with(&before));
    document
        .replace_object_geometries([(copies[0], point(9.))])
        .unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&before));
    assert_eq!(document.object(copies[0]).unwrap().geometry(), &point(9.));
    document.delete_object(id).unwrap();
    assert_eq!(*before, point(1.));
    document.undo().unwrap();
    assert!(snapshot(&document, id).shares_storage_with(&before));
}

#[test]
fn failed_batch_preserves_every_snapshot_identity() {
    let mut document = Document::default();
    let ids = [1., 2.].map(|x| document.add_geometry(point(x)).unwrap());
    document.set_objects_locked([ids[1]], true).unwrap();
    let before = ids.map(|id| snapshot(&document, id));
    assert!(
        document
            .replace_object_geometries([(ids[0], point(8.)), (ids[1], point(9.))])
            .is_err()
    );
    for (id, old) in ids.into_iter().zip(before) {
        assert!(snapshot(&document, id).shares_storage_with(&old));
    }
}

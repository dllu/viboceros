//! Transient live-object identity, independent of UUIDs and model-table order.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

impl Object {
    /// Process-local identity of this object version. History restores the
    /// original version and its number; file import creates a fresh number.
    pub const fn runtime_serial_number(&self) -> u64 {
        self.runtime_serial
    }
    pub(super) fn new_runtime_serial() -> u64 {
        NEXT_SERIAL.fetch_add(1, Ordering::Relaxed)
    }
    /// Last admission or geometry replacement event, including history replay.
    /// Independent of the persistent table position and restored version identity.
    pub const fn geometry_event_serial(&self) -> u64 {
        self.geometry_event_serial
    }
    pub(super) fn note_geometry_event(&mut self) {
        self.geometry_event_serial = Self::new_runtime_serial();
    }
}

// Keep model debug snapshots stable across transient display/lifetime events.
impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("id", &self.id)
            .field("geometry", &self.geometry)
            .field("geometry_user_text", &self.geometry_user_text)
            .field("attributes", &self.attributes)
            .field("isolation", &self.isolation)
            .field("group_ids", &self.group_ids)
            .finish()
    }
}

// Runtime sequence is observation metadata, not persistent model state.
impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.geometry == other.geometry
            && self.geometry_user_text == other.geometry_user_text
            && self.attributes == other.attributes
            && self.isolation == other.isolation
            && self.group_ids == other.group_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_restores_version_identity_and_renews_geometry_event_order() {
        let mut doc = Document::default();
        let ids = (0..3)
            .map(|i| {
                doc.add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let original = doc.objects().cloned().collect::<Vec<_>>();
        let serials = original
            .iter()
            .map(Object::runtime_serial_number)
            .collect::<Vec<_>>();
        doc.clear_history().unwrap();
        doc.delete_objects([ids[0], ids[2]]).unwrap();
        assert_eq!(
            doc.object(ids[1]).unwrap().runtime_serial_number(),
            serials[1]
        );
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(
            doc.object(ids[2]).unwrap().runtime_serial_number(),
            serials[2]
        );
        assert!(
            doc.object(ids[0]).unwrap().geometry_event_serial()
                > doc.object(ids[2]).unwrap().geometry_event_serial()
        );
        assert_eq!(
            doc.object(ids[1]).unwrap().runtime_serial_number(),
            serials[1]
        );
        doc.redo().unwrap();
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
    }
    #[test]
    fn geometry_replacement_versions_retain_their_identity_across_history() {
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        let original = doc.object(id).unwrap().clone();
        doc.clear_history().unwrap();
        doc.replace_object_geometries([(
            id,
            Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()),
        )])
        .unwrap();
        let changed = doc.object(id).unwrap().runtime_serial_number();
        assert!(changed > original.runtime_serial_number());
        doc.undo().unwrap();
        assert_eq!(*doc.object(id).unwrap(), original);
        let restored = doc.object(id).unwrap().runtime_serial_number();
        assert_eq!(restored, original.runtime_serial_number());
        doc.redo().unwrap();
        assert_eq!(doc.object(id).unwrap().runtime_serial_number(), changed);
    }
}

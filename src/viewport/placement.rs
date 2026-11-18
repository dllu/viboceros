//! Translated layout instances reuse the cached model geometry in every view.
use super::object_preview::PreviewObject;
use super::*;
use std::collections::BTreeMap;
use viboceros_command::placement::Placement;

impl Viewport {
    pub(super) fn placement_objects(
        &self,
        document: &Document,
        layout: &Placement,
    ) -> BTreeMap<ObjectId, PreviewObject> {
        let mut cache = self.display_cache.borrow_mut();
        let tolerance = document.tolerance();
        let control_net = document.subd_control_net();
        let mut objects = BTreeMap::new();
        for (id, transform) in layout.translations() {
            if let Some(object) = document.object(id) {
                objects.insert(
                    id,
                    PreviewObject {
                        geometry: cache.get(object, tolerance, control_net),
                        transform: Some(transform),
                    },
                );
            }
        }
        for (id, edited) in layout.grip_edits() {
            if let Some(source) = document.object(id) {
                objects.insert(
                    id,
                    PreviewObject {
                        geometry: cache.temporary_source(source, edited, tolerance, control_net),
                        transform: None,
                    },
                );
            }
        }
        objects
    }
}

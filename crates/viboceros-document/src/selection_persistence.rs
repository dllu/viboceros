//! Temporary object-getter picks survive modeless controls, not command completion.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ObjectSelectionState {
    Unselected = 0,
    Temporary = 1,
    Persistent = 2,
}

impl Document {
    pub fn object_selection_state(&self, id: ObjectId) -> ObjectSelectionState {
        if !self.selection.contains(&id) {
            ObjectSelectionState::Unselected
        } else if self.temporary_object_selection.contains(&id) {
            ObjectSelectionState::Temporary
        } else {
            ObjectSelectionState::Persistent
        }
    }

    /// Downgrade currently selected getter inputs without expanding groups,
    /// changing SelPrev, or recording model history. Unselected IDs are ignored.
    pub fn mark_object_selection_temporary(&mut self, ids: impl IntoIterator<Item = ObjectId>) {
        for id in ids {
            if self.selection.contains(&id) {
                self.temporary_object_selection.insert(id);
            }
        }
    }

    /// Finish a command while retaining persistent preselection and SelPrev.
    pub fn clear_temporary_object_selection(&mut self) -> usize {
        if self.temporary_object_selection.is_empty() {
            return 0;
        }
        let temporary = std::mem::take(&mut self.temporary_object_selection);
        let before = self.selection.len();
        self.selection.retain(|id| !temporary.contains(id));
        self.selection_order.retain(|id| !temporary.contains(id));
        before - self.selection.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sources() -> (Document, ObjectId, ObjectId) {
        let mut doc = Document::default();
        let a = doc
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        let b = doc
            .add_geometry(Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))
            .unwrap();
        doc.clear_history().unwrap();
        doc.select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        (doc, a, b)
    }
    #[test]
    fn command_cleanup_retains_persistent_peers_and_pending_redo() {
        let (mut doc, a, b) = sources();
        doc.add_geometry(Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()))
            .unwrap();
        doc.undo().unwrap();
        let redo = doc.redo_label().map(str::to_owned);
        doc.mark_object_selection_temporary([b]);
        assert_eq!(
            doc.object_selection_state(a),
            ObjectSelectionState::Persistent
        );
        assert_eq!(
            doc.object_selection_state(b),
            ObjectSelectionState::Temporary
        );
        assert_eq!(doc.clear_temporary_object_selection(), 1);
        assert_eq!(
            doc.object_selection_state(b),
            ObjectSelectionState::Unselected
        );
        assert!(doc.is_selected(a));
        assert_eq!(doc.redo_label(), redo.as_deref());
        doc.select_objects_direct([b], SelectionMode::Add).unwrap();
        doc.mark_object_selection_temporary([b]);
        doc.select_objects_direct([b], SelectionMode::Add).unwrap();
        assert_eq!(
            doc.object_selection_state(b),
            ObjectSelectionState::Persistent
        );
    }
    #[test]
    fn rollback_restores_getter_status_but_undo_restores_persistent_selection() {
        let (mut doc, _, b) = sources();
        doc.mark_object_selection_temporary([b]);
        doc.begin_transaction("Rejected deletion").unwrap();
        doc.delete_objects([b]).unwrap();
        doc.rollback_transaction().unwrap();
        assert_eq!(
            doc.object_selection_state(b),
            ObjectSelectionState::Temporary
        );
        doc.delete_objects([b]).unwrap();
        doc.undo().unwrap();
        assert_eq!(
            doc.object_selection_state(b),
            ObjectSelectionState::Persistent
        );
    }
}

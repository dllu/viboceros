//! Restore ordered curve picks without touching model history.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn restore_curve_selection(&mut self, ids: Vec<ObjectId>) {
        if self.document.selected_object_ids().eq(ids.iter().copied()) {
            return;
        }
        let ids = ids
            .into_iter()
            .filter(|id| self.document.is_object_selectable(*id))
            .collect::<Vec<_>>();
        let _ = self.document.set_transient_selection(ids);
    }
}

//! Persistent in-place edit controls, independent of each modelling getter.
use super::*;
impl VibocerosApp {
    pub(super) fn show_block_edit(&mut self, context: &egui::Context) {
        if !self.document.is_block_editing() {
            return;
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new("Block edit")
            .open(&mut open)
            .collapsible(false)
            .show(context, |ui| {
                ui.label("Edit the exposed members with ordinary commands.");
                ui.label(format!(
                    "{} editable object(s)",
                    self.document.block_edit_objects().len()
                ));
                if ui.button("Save and close").clicked() {
                    action = Some("BlockEdit SaveAndClose");
                }
                if ui.button("Discard and cancel").clicked() {
                    action = Some("BlockEdit DiscardAndCancel");
                }
            });
        if !open {
            action = Some("BlockEdit DiscardAndCancel");
        }
        if let Some(action) = action {
            self.execute_command(action);
        }
    }
}

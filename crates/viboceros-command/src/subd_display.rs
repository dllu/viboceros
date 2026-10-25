//! A public command entry for headless clients; the GUI also admits it nested.
use super::*;
pub(super) struct SubDDisplayToggle;
impl Command for SubDDisplayToggle {
    fn name(&self) -> &'static str {
        "SubDDisplayToggle"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage("SubDDisplayToggle"));
        }
        document.set_subd_control_net(!document.subd_control_net());
        Ok(format!(
            "SubD display: {}",
            if document.subd_control_net() {
                "Flat"
            } else {
                "Smooth"
            }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subd_display_preserves_redo_selection_and_rejects_invalid_input() {
        let mut doc = Document::default();
        let commands = CommandRegistry::with_builtins();
        commands.execute(&mut doc, "Line 0,0,0 1,2,3").unwrap();
        let id = doc.objects().next().unwrap().id();
        doc.clear_history().unwrap();
        commands.execute(&mut doc, "Point 9,8,7").unwrap();
        doc.undo().unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        let source = doc.object(id).unwrap().clone();
        for flat in [true, false] {
            commands.execute(&mut doc, "_SubDDisplayToggle").unwrap();
            assert_eq!(doc.subd_control_net(), flat);
            assert!(doc.can_redo());
            assert!(!doc.can_undo());
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
            assert_eq!(doc.object(id), Some(&source));
        }
        assert!(
            commands
                .execute(&mut doc, "SubDDisplayToggle nonsense")
                .is_err()
        );
        assert!(!doc.subd_control_net());
        assert!(doc.can_redo());
    }
}

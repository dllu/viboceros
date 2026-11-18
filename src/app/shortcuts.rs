use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum FileAction {
    Open,
    Save,
    SaveAs,
}

impl FileAction {
    pub(super) const ALL: [Self; 3] = [Self::Open, Self::Save, Self::SaveAs];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Open => "Open 3DM…",
            Self::Save => "Save",
            Self::SaveAs => "Save As…",
        }
    }

    pub(super) fn shortcut(self) -> egui::KeyboardShortcut {
        let command = egui::Modifiers::COMMAND;
        match self {
            Self::Open => egui::KeyboardShortcut::new(command, egui::Key::O),
            Self::Save => egui::KeyboardShortcut::new(command, egui::Key::S),
            Self::SaveAs => {
                egui::KeyboardShortcut::new(command | egui::Modifiers::SHIFT, egui::Key::S)
            }
        }
    }

    fn path_command(self) -> &'static str {
        match self {
            Self::Open => "Open3dm",
            Self::Save | Self::SaveAs => "SaveAs",
        }
    }
}

impl VibocerosApp {
    pub(super) fn file_action_enabled(&self, action: FileAction) -> bool {
        (matches!(action, FileAction::Save) && self.document_path.is_some())
            || (self.command_file_child_idle()
                && !self.command_file.is_active()
                && self.selection_menu.is_none())
    }

    pub(super) fn file_action(&mut self, action: FileAction) {
        if matches!(action, FileAction::Save) && self.document_path.is_some() {
            self.log_save_shortcut();
            return;
        }
        if !self.file_action_enabled(action) {
            self.push_log("Finish the current command before choosing a file path.".into());
            return;
        }
        if self.command_line.file_path != Some(action) {
            self.command_line.file_path = Some(action);
            self.reset_file_path_input();
        }
        self.command_focus_requested = true;
    }

    pub(super) fn run_file_path_action(&mut self) -> bool {
        let Some(action) = self.command_line.file_path else {
            return false;
        };
        let path = self.command_input.trim();
        if path.is_empty() {
            return self.cancel_file_path_action();
        }
        let command = format!("{} {path}", action.path_command());
        self.push_log(format!("> {command}"));
        match self.try_run_3dm_command(&command).expect("3DM file action") {
            Ok(message) => {
                self.push_log(message);
                self.cancel_file_path_action();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn cancel_file_path_action(&mut self) -> bool {
        if self.command_line.file_path.take().is_none() {
            return false;
        }
        self.reset_file_path_input();
        true
    }

    fn log_save_shortcut(&mut self) {
        self.push_log("> Save".into());
        match self
            .try_run_3dm_command("Save")
            .expect("Save is a 3DM command")
        {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn handle_document_shortcuts(&mut self, ui: &mut egui::Ui) {
        let idle = self.command_file_child_idle()
            && self.selection_menu.is_none()
            && !self.command_file.is_active();
        let can_edit_model = idle && !self.editing_text(ui.ctx());
        let command = egui::Modifiers::COMMAND;
        let actions = ui.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                let egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                let file = FileAction::ALL.into_iter().find(|action| {
                    let binding = action.shortcut();
                    *key == binding.logical_key && modifiers.matches_exact(binding.modifiers)
                });
                let model = if can_edit_model {
                    match (
                        *key,
                        modifiers.matches_exact(command),
                        modifiers.matches_exact(command | egui::Modifiers::SHIFT),
                    ) {
                        (egui::Key::Z, true, _) => Some("Undo"),
                        (egui::Key::Y, true, _) | (egui::Key::Z, _, true) => Some("Redo"),
                        (egui::Key::A, true, _) => Some("SelAll"),
                        _ => None,
                    }
                } else {
                    None
                };
                if file.is_none() && model.is_none() {
                    return true;
                }
                if *pressed && !*repeat {
                    actions.push((file, model));
                }
                false
            });
            actions
        });
        for (file, model) in actions {
            if let Some(action) = file {
                self.file_action(action);
            } else if let Some(command) = model {
                self.execute_command(command);
            }
        }
    }

    pub(super) fn editing_text(&self, context: &egui::Context) -> bool {
        context.text_edit_focused()
            && !(self.command_input.is_empty()
                && self.command_line.file_path.is_none()
                && context.memory(|memory| memory.has_focus(command_line::input_id())))
    }
}

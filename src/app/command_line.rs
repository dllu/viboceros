//! Command editing, completion, and recall, independent of modeling state.
use super::*;
mod completion;
mod history;
#[cfg(test)]
pub(super) use completion::command_completions;

#[derive(Default)]
pub(super) struct CommandLineState {
    history: history::History,
    completion: completion::CompletionState,
    completion_cursor: Option<usize>,
}

impl CommandLineState {
    pub fn load() -> (Self, Option<String>) {
        let (history, error) = history::History::load(history::default_path());
        (
            Self {
                history,
                ..Self::default()
            },
            error,
        )
    }

    pub fn remember(&mut self, input: &str) -> Option<String> {
        self.completion.reset();
        self.history.remember(input).err().map(|error| {
            format!(
                "Command history could not be saved; recall remains available this session: {error}"
            )
        })
    }
}

impl VibocerosApp {
    fn command_line_idle(&self) -> bool {
        self.end_analysis_pick.is_none()
            && self.zoom_factor_pending.is_none()
            && self.snap_size_pending.is_none()
            && self.active_command.is_none()
            && self.object_prompt.is_none()
            && self.group_prompt.is_none()
            && self.intersection_prompt.is_none()
            && self.edge_prompt.is_none()
            && self.plane_prompt.is_none()
    }

    pub(super) fn remember_command_input(&mut self, input: &str) {
        if input.is_empty() {
            return;
        }
        let name = input.split_whitespace().next().unwrap_or("");
        let command_name = name.trim_start_matches(['\'', '_', '-']);
        let explicit_command = self.commands.recognizes(command_name)
            || command_name.eq_ignore_ascii_case("CPlane")
            || command_name.eq_ignore_ascii_case("NamedView")
            || viboceros_command::interface::COMMAND_NAMES
                .iter()
                .any(|n| n.eq_ignore_ascii_case(command_name));
        if (self.command_line_idle() || explicit_command)
            && let Some(error) = self.command_line.remember(input)
        {
            self.push_log(error);
        }
    }

    pub(super) fn capture_global_command_typing(&mut self, root: &mut egui::Ui) {
        if root.ctx().text_edit_focused()
            || root.input(|input| input.modifiers.command || input.modifiers.alt)
        {
            return;
        }
        if self.command_line_idle() {
            let older = root.input_mut(|input| {
                consume_exact_key(input, egui::Modifiers::NONE, egui::Key::ArrowUp)
            });
            let newer = root.input_mut(|input| {
                consume_exact_key(input, egui::Modifiers::NONE, egui::Key::ArrowDown)
            });
            if (older || newer)
                && let Some(text) = self
                    .command_line
                    .history
                    .navigate(&self.command_input, older)
            {
                self.command_input = text;
                self.command_line.completion.reset();
                self.command_focus_requested = true;
            }
        }
        let typed = root.input_mut(|input| {
            let mut typed = String::new();
            input.events.retain(|event| {
                if let egui::Event::Text(text) = event
                    && text.chars().any(|character| !character.is_control())
                {
                    typed.push_str(text);
                    false
                } else {
                    true
                }
            });
            typed
        });
        if !typed.is_empty() {
            self.queue_command_text(&typed);
        }
    }

    fn queue_command_text(&mut self, typed: &str) {
        self.command_input.push_str(typed);
        self.command_line.completion.reset();
        self.command_focus_requested = true;
    }

    pub(super) fn show_command_line(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("command_line")
            .resizable(true)
            .default_size(120.0)
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(82.0)
                    .show(ui, |ui| {
                        for line in &self.command_log {
                            ui.label(line);
                        }
                    });
                ui.separator();
                let idle = self.command_line_idle();
                self.command_line
                    .completion
                    .refresh(&self.commands, &self.command_input, idle);
                ui.horizontal(|ui| {
                    let label = if self.zoom_factor_pending.is_some() {
                        "Zoom Factor"
                    } else if self.plane_prompt.is_some() {
                        "CPlane"
                    } else if let Some(prompt) = &self.object_prompt {
                        prompt.label()
                    } else if self.group_prompt.is_some() {
                        "AddToGroup"
                    } else if self.intersection_prompt.is_some() {
                        "IntersectTwoSets"
                    } else if let Some(prompt) = &self.edge_prompt {
                        prompt.name()
                    } else {
                        self.active_command
                            .map_or("Command", InteractiveCommand::name)
                    };
                    ui.label(RichText::new(format!("{label}:")).strong());
                    let id = ui.make_persistent_id("main_command_input");
                    let focused = ui.memory(|memory| memory.focused() == Some(id));
                    let (tab, reverse, older, newer) = if focused && idle {
                        ui.input_mut(|input| {
                            // Consume Shift+Tab first: plain Tab must not steal it.
                            let reverse =
                                consume_exact_key(input, egui::Modifiers::SHIFT, egui::Key::Tab);
                            let tab = reverse
                                || consume_exact_key(input, egui::Modifiers::NONE, egui::Key::Tab);
                            let older =
                                consume_exact_key(input, egui::Modifiers::NONE, egui::Key::ArrowUp);
                            let newer = consume_exact_key(
                                input,
                                egui::Modifiers::NONE,
                                egui::Key::ArrowDown,
                            );
                            (tab, reverse, older, newer)
                        })
                    } else {
                        (false, false, false, false)
                    };
                    if tab || older || newer {
                        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                    }
                    if older || newer {
                        if let Some(text) = self
                            .command_line
                            .history
                            .navigate(&self.command_input, older)
                        {
                            self.command_input = text;
                            self.command_line.completion.reset();
                            self.command_focus_requested = true;
                        }
                    } else if tab && let Some(text) = self.command_line.completion.cycle(reverse) {
                        self.command_line.completion_cursor = Some(completion_cursor(&text));
                        self.command_input = text;
                        self.command_focus_requested = true;
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.command_input)
                            .id(id)
                            .lock_focus(true)
                            .desired_width(f32::INFINITY)
                            .hint_text(if self.end_analysis_pick.is_some() {
                                "Pick curves; Enter finishes, Esc cancels"
                            } else if self.snap_size_pending.is_some() {
                                "Enter a positive grid snap spacing; Enter or Esc cancels"
                            } else if self.zoom_factor_pending.is_some() {
                                "Enter a positive factor; Enter or Esc cancels"
                            } else if self.plane_prompt.is_some() {
                                "Define the construction plane; Esc returns to the previous prompt"
                            } else if let Some(prompt) = &self.object_prompt {
                                prompt.hint()
                            } else if let Some(prompt) = &self.group_prompt {
                                prompt.hint()
                            } else if let Some(prompt) = &self.intersection_prompt {
                                prompt.hint()
                            } else if let Some(prompt) = &self.edge_prompt {
                                prompt.hint()
                            } else if self.active_command.is_some() {
                                if self
                                    .active_command
                                    .is_some_and(InteractiveCommand::collects_curve_points)
                                {
                                    "Pick curve points; press Enter to finish or Esc to cancel"
                                } else {
                                    "Type coordinates or pick; Esc cancels"
                                }
                            } else {
                                "Type a command • Tab to complete • ↑ for history"
                            }),
                    );
                    if response.changed() {
                        self.command_line.completion.reset();
                    }
                    if self.command_focus_requested {
                        let cursor = self
                            .command_line
                            .completion_cursor
                            .take()
                            .unwrap_or_else(|| self.command_input.chars().count());
                        request_command_focus(ui.ctx(), id, &response, cursor);
                        self.command_focus_requested = false;
                    }
                    if response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    {
                        self.run_command();
                        request_command_focus(
                            ui.ctx(),
                            id,
                            &response,
                            self.command_input.chars().count(),
                        );
                    }
                });
                self.show_edge_choices(ui);
                let idle = self.command_line_idle();
                self.command_line
                    .completion
                    .refresh(&self.commands, &self.command_input, idle);
                let completion = &self.command_line.completion;
                if !completion.candidates.is_empty() {
                    let mut chosen = None;
                    let start = completion.selected.unwrap_or(0) / 8 * 8;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Tab / Shift+Tab:").small());
                        for (index, candidate) in
                            completion.candidates.iter().enumerate().skip(start).take(8)
                        {
                            if ui
                                .selectable_label(
                                    completion.selected == Some(index),
                                    &candidate.label,
                                )
                                .clicked()
                            {
                                chosen = Some(index);
                            }
                        }
                        if completion.candidates.len() > 8 {
                            ui.small(format!("{} matches", completion.candidates.len()));
                        }
                    });
                    if let Some(index) = chosen
                        && let Some(text) = self.command_line.completion.choose(index)
                    {
                        self.command_line.completion_cursor = Some(completion_cursor(&text));
                        self.command_input = text;
                        self.command_focus_requested = true;
                    }
                }
            });
    }
}

// Egui's consume_key deliberately ignores extra Shift/Alt modifiers. Keep
// text-selection and platform shortcuts intact by matching these keys exactly.
fn consume_exact_key(
    input: &mut egui::InputState,
    modifiers: egui::Modifiers,
    key: egui::Key,
) -> bool {
    let mut found = false;
    input.events.retain(|event| {
        let matched = matches!(event, egui::Event::Key { key: event_key, modifiers: event_modifiers, pressed: true, .. } if *event_key == key && *event_modifiers == modifiers);
        found |= matched;
        !matched
    });
    found
}

fn completion_cursor(text: &str) -> usize {
    let directory = text.ends_with("/\"") || cfg!(windows) && text.ends_with("\\\"");
    text.chars().count() - usize::from(directory)
}

fn request_command_focus(
    context: &egui::Context,
    id: egui::Id,
    response: &egui::Response,
    cursor: usize,
) {
    // Re-requesting existing focus resets egui's keyboard lock filter, which
    // makes the next Tab/arrow move focus to a suggestion instead of editing.
    if !context.memory(|memory| memory.has_focus(id)) {
        response.request_focus();
    }
    context.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            egui::EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            },
        )
    });
    let mut state = egui::TextEdit::load_state(context, id).unwrap_or_default();
    let cursor = egui::text::CCursor::new(cursor);
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
    egui::TextEdit::store_state(context, id, state);
}

#[cfg(test)]
pub(super) mod tests {
    use std::path::PathBuf;
    pub(in crate::app) struct TempDirectory(pub PathBuf);
    impl TempDirectory {
        pub fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "viboceros-command-line-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

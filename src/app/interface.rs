//! Application adapter: interface actions do not enter the model command lifecycle.

use super::*;
use viboceros_command::interface::{
    self, InterfaceCommand, InterfaceState, SwitchAction, ViewportTarget,
};

impl VibocerosApp {
    pub(super) fn interface_state(&self) -> InterfaceState {
        InterfaceState {
            grid_snap: self.grid_snap,
            osnap: self.osnap,
            smart_track: self.smart_track,
            active_viewport: self.active_viewport,
            display_modes: self.viewports.iter().map(|v| v.display_mode).collect(),
        }
    }

    pub(super) fn apply_interface_command(&mut self, command: InterfaceCommand) {
        let mut state = self.interface_state();
        match state.apply(command) {
            Ok(message) => {
                if matches!(
                    command,
                    InterfaceCommand::ZoomExtents
                        | InterfaceCommand::ZoomSelected
                        | InterfaceCommand::ZoomAllExtents
                        | InterfaceCommand::ZoomAllSelected
                ) {
                    let selected = matches!(
                        command,
                        InterfaceCommand::ZoomSelected | InterfaceCommand::ZoomAllSelected
                    );
                    let all = matches!(
                        command,
                        InterfaceCommand::ZoomAllExtents | InterfaceCommand::ZoomAllSelected
                    );
                    let target = if all {
                        "all viewports"
                    } else {
                        "active viewport"
                    };
                    let result = if all {
                        Viewport::zoom_all(&mut self.viewports, &self.document, selected)
                    } else if selected {
                        self.viewports[self.active_viewport].zoom_selected(&self.document)
                    } else {
                        self.viewports[self.active_viewport].zoom_extents(&self.document)
                    };
                    self.push_log(match result {
                        Ok(true) if selected => {
                            format!("Zoomed to selected visible objects ({target})")
                        }
                        Ok(false) if selected => "No selected visible objects to zoom to".into(),
                        Ok(true) => format!("Zoomed to visible extents ({target})"),
                        Ok(false) => "No visible objects to zoom to".into(),
                        Err(error) => format!("Error: {error}"),
                    });
                    return;
                }
                self.grid_snap = state.grid_snap;
                self.osnap = state.osnap;
                self.smart_track = state.smart_track;
                for (viewport, mode) in self.viewports.iter_mut().zip(state.display_modes) {
                    viewport.display_mode = mode;
                }
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn try_run_interface_command(&mut self, input: &str) -> bool {
        let mut tokens = input.split_whitespace();
        if tokens.next().is_some_and(|name| {
            name.trim_start_matches(['\'', '_', '-'])
                .eq_ignore_ascii_case("Help")
        }) {
            self.push_log(format!("> {input}"));
            match tokens.collect::<Vec<_>>().as_slice() {
                [] => {
                    let mut names = self.commands.command_names();
                    names.extend(interface::COMMAND_NAMES);
                    names.push("CPlane");
                    names.sort_unstable();
                    names.dedup();
                    self.push_log(format!("Commands: {}", names.join(", ")));
                    self.push_log("Enter Help UI for interface commands and shortcuts.".into());
                    self.command_input.clear();
                }
                [topic] if topic.trim_start_matches('_').eq_ignore_ascii_case("UI") => {
                    self.push_log(interface::HELP.into());
                    self.push_log(viboceros_command::construction_plane::USAGE.into());
                    self.command_input.clear();
                }
                _ => self.push_log("Usage: Help [UI]".into()),
            }
            return true;
        }
        let Some(command) = interface::parse(input) else {
            return false;
        };
        self.push_log(format!("> {input}"));
        match command {
            Ok(command) => {
                self.apply_interface_command(command);
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn handle_interface_shortcuts(&mut self, ui: &mut egui::Ui) {
        self.handle_plane_shortcuts(ui);
        // These keys have no text-editing meaning. Text editors retain their
        // own undo history; document undo/redo is not intercepted here.
        let shortcuts = [
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::E,
                InterfaceCommand::ZoomExtents,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::E,
                InterfaceCommand::ZoomAllExtents,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::F9,
                InterfaceCommand::SetSnap(SwitchAction::Toggle),
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::F4,
                InterfaceCommand::SetOsnap(SwitchAction::Toggle),
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::W,
                InterfaceCommand::SetDisplayMode {
                    viewport: ViewportTarget::Active,
                    mode: DisplayMode::Wireframe,
                },
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::S,
                InterfaceCommand::SetDisplayMode {
                    viewport: ViewportTarget::Active,
                    mode: DisplayMode::Shaded,
                },
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::G,
                InterfaceCommand::SetDisplayMode {
                    viewport: ViewportTarget::Active,
                    mode: DisplayMode::Ghosted,
                },
            ),
        ];
        // Preserve event order, reject extra modifiers (notably Alt+F4), and
        // consume auto-repeat without repeatedly toggling a setting.
        let actions = ui.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } = event
                    && let Some((_, _, command)) =
                        shortcuts.iter().find(|(pattern, shortcut_key, _)| {
                            key == shortcut_key && modifiers.matches_exact(*pattern)
                        })
                {
                    if *pressed && !*repeat {
                        actions.push(*command);
                    }
                    false
                } else {
                    true
                }
            });
            actions
        });
        for action in actions {
            self.apply_interface_command(action);
        }
    }
}

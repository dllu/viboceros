//! Compact command-first chrome; modeling commands live in the command line.

use super::*;
use viboceros_command::interface::{InterfaceCommand, SwitchAction, ViewportTarget};

impl VibocerosApp {
    pub(super) fn show_toolbar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                let idle = self.active_command.is_none()
                    && self.plane_prompt.is_none()
                    && self.object_prompt.is_none();
                if ui
                    .add_enabled(idle && self.document.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.execute_command("Undo");
                }
                if ui
                    .add_enabled(idle && self.document.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.execute_command("Redo");
                }
                ui.separator();
                let viewport = &mut self.viewports[self.active_viewport];
                let mut kind = viewport.kind();
                let mut preset_picked = false;
                egui::ComboBox::from_id_salt("view_kind")
                    .width(95.0)
                    .selected_text(kind.label())
                    .show_ui(ui, |ui| {
                        for choice in [
                            ViewKind::Top,
                            ViewKind::Perspective,
                            ViewKind::Front,
                            ViewKind::Right,
                        ] {
                            preset_picked |= ui
                                .selectable_value(&mut kind, choice, choice.label())
                                .clicked();
                        }
                    })
                    .response
                    .on_hover_text("View preset for the active viewport");
                if preset_picked {
                    viewport.set_view_kind(kind);
                }
                let mut mode = viewport.display_mode;
                egui::ComboBox::from_id_salt("display_mode")
                    .width(95.0)
                    .selected_text(mode.label())
                    .show_ui(ui, |ui| {
                        for choice in DisplayMode::ALL {
                            ui.selectable_value(&mut mode, choice, choice.label());
                        }
                    })
                    .response
                    .on_hover_text("Display mode for the active viewport · Ctrl/Cmd+Alt+W/S/G");
                if mode != viewport.display_mode {
                    self.apply_interface_command(InterfaceCommand::SetDisplayMode {
                        viewport: ViewportTarget::Active,
                        mode,
                    });
                }
                ui.separator();
                for (enabled, label, hint, command) in [
                    (
                        self.grid_snap,
                        "Grid Snap",
                        "Snap to a one-unit grid · F9",
                        InterfaceCommand::SetSnap(SwitchAction::Toggle),
                    ),
                    (
                        self.osnap,
                        "Osnap",
                        "Object snaps · F4",
                        InterfaceCommand::SetOsnap(SwitchAction::Toggle),
                    ),
                    (
                        self.smart_track,
                        "SmartTrack",
                        "Track axes from command reference points",
                        InterfaceCommand::SmartTrack(SwitchAction::Toggle),
                    ),
                ] {
                    if ui
                        .selectable_label(enabled, label)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.apply_interface_command(command);
                    }
                }
                ui.separator();
                ui.weak(format!(
                    "{} selected",
                    self.document.selected_object_count()
                ));
                if ui
                    .small_button("?")
                    .on_hover_text("Interface commands and shortcuts")
                    .clicked()
                {
                    self.push_log(viboceros_command::interface::HELP.into());
                }
            });
        });
    }
}

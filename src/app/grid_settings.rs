//! Grid properties panel; edits are applied through the typed Grid command.

use super::*;
use viboceros_command::interface::{GridUpdate, InterfaceCommand, SnapSpacing, ViewportTarget};

impl VibocerosApp {
    pub(super) fn show_grid_settings(&mut self, root: &egui::Ui) {
        if !self.grid_settings_open {
            return;
        }
        let mut open = self.grid_settings_open;
        let mut apply_to = self.grid_settings_apply_to;
        let current = self.viewports[self.active_viewport].grid_settings();
        let mut update = GridUpdate::default();
        egui::Window::new("Grid settings")
            .open(&mut open)
            .resizable(false)
            .show(root.ctx(), |ui| {
                egui::ComboBox::from_label("Apply to")
                    .selected_text(if apply_to == ViewportTarget::All {
                        "All viewports"
                    } else {
                        "Active viewport"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut apply_to,
                            ViewportTarget::Active,
                            "Active viewport",
                        );
                        ui.selectable_value(&mut apply_to, ViewportTarget::All, "All viewports");
                    });
                ui.separator();
                let mut spacing = current.snap_spacing;
                if ui
                    .add(
                        egui::DragValue::new(&mut spacing)
                            .speed(0.1)
                            .prefix("Snap spacing "),
                    )
                    .changed()
                {
                    update.snap_spacing = SnapSpacing::try_new(spacing);
                }
                let mut spacing = current.minor_spacing;
                if ui
                    .add(
                        egui::DragValue::new(&mut spacing)
                            .speed(0.1)
                            .prefix("Minor line spacing "),
                    )
                    .changed()
                {
                    update.minor_spacing = SnapSpacing::try_new(spacing);
                }
                let mut major_interval = current.major_interval;
                if ui
                    .add(
                        egui::DragValue::new(&mut major_interval)
                            .range(1..=100_000)
                            .prefix("Major every "),
                    )
                    .changed()
                {
                    update.major_interval = Some(major_interval);
                }
                let mut line_count = current.line_count;
                if ui
                    .add(
                        egui::DragValue::new(&mut line_count)
                            .range(0..=100_000)
                            .prefix("Lines from origin "),
                    )
                    .changed()
                {
                    update.line_count = Some(line_count);
                }
                ui.separator();
                let mut show_grid = current.show_grid;
                if ui.checkbox(&mut show_grid, "Show grid lines").changed() {
                    update.show_grid = Some(show_grid);
                }
                let mut show_axes = current.show_axes;
                if ui.checkbox(&mut show_axes, "Show grid axes").changed() {
                    update.show_axes = Some(show_axes);
                }
                let mut show_world_axes = current.show_world_axes;
                if ui
                    .checkbox(&mut show_world_axes, "Show world axes icon")
                    .changed()
                {
                    update.show_world_axes = Some(show_world_axes);
                }
            });
        self.grid_settings_open = open;
        self.grid_settings_apply_to = apply_to;
        if !update.is_empty() {
            self.apply_interface_command(InterfaceCommand::Grid { update, apply_to });
        }
    }
}

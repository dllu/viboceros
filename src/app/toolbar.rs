//! Compact command-first chrome; modeling commands live in the command line.

use super::*;
use viboceros_command::interface::{InterfaceCommand, SwitchAction, ViewportTarget, ZoomScale};

impl VibocerosApp {
    pub(super) fn show_toolbar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                let idle = self.active_command.is_none()
                    && self.plane_prompt.is_none()
                    && self.object_prompt.is_none()
                    && self.group_prompt.is_none()
                    && self.edge_prompt.is_none();
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
                    .selected_text(viewport.view_label())
                    .show_ui(ui, |ui| {
                        for choice in [
                            ViewKind::Top,
                            ViewKind::Bottom,
                            ViewKind::Perspective,
                            ViewKind::Front,
                            ViewKind::Back,
                            ViewKind::Right,
                            ViewKind::Left,
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
                egui::containers::menu::MenuButton::new("View options").ui(ui, |ui| {
                    let mut scale = self.zoom_scale;
                    if ui
                        .add(
                            egui::DragValue::new(&mut scale)
                                .speed(0.01)
                                .prefix("Zoom step "),
                        )
                        .on_hover_text("View zoom scale factor; 0.9 is the Rhino default")
                        .changed()
                        && let Some(scale) = ZoomScale::try_new(scale)
                    {
                        self.apply_interface_command(InterfaceCommand::SetZoomScale(scale));
                    }
                    ui.separator();
                    for (label, current, parallel_view) in [
                        (
                            "Parallel extents ",
                            self.zoom_extents_borders.parallel,
                            true,
                        ),
                        (
                            "Perspective extents ",
                            self.zoom_extents_borders.perspective,
                            false,
                        ),
                    ] {
                        let mut value = current;
                        if ui
                            .add(egui::DragValue::new(&mut value).speed(0.01).prefix(label))
                            .on_hover_text("Scale of the Zoom Extents fitting bounds")
                            .changed()
                            && let Some(value) = ZoomScale::try_new(value)
                        {
                            self.apply_interface_command(InterfaceCommand::SetZoomExtentsBorder {
                                parallel: parallel_view.then_some(value),
                                perspective: (!parallel_view).then_some(value),
                            });
                        }
                    }
                });
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
                egui::containers::menu::MenuButton::new("Snap modes")
                    .config(
                        egui::containers::menu::MenuConfig::default()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| self.show_snap_modes(ui));
                if let Some(label) = self.one_shot_snap_label() {
                    ui.strong(format!("Next pick: {label}"));
                }
                ui.separator();
                ui.weak(format!(
                    "{} selected",
                    self.document.selected_object_count()
                ));
                let (units_label, units_hint) = model_units_status(&self.document);
                ui.weak(units_label).on_hover_text(units_hint);
                if ui
                    .small_button("?")
                    .on_hover_text("Interface commands and shortcuts")
                    .clicked()
                {
                    self.push_log(viboceros_command::interface::HELP.into());
                    self.push_log(snapping::HELP.into());
                }
            });
        });
    }
}

/// Bound user-provided unit names before text layout. The indicator is always
/// derived from the document, so imports and undo/redo cannot leave stale state.
fn model_units_status(document: &Document) -> (String, String) {
    use viboceros_geometry::LengthUnitSystem;
    let units = document.units();
    let mut chars = units.name().chars();
    let mut name: String = chars
        .by_ref()
        .take(24)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if chars.next().is_some() {
        name.push('…');
    }
    if name.trim().is_empty() {
        name = "Custom units".into();
    }
    let tolerance = document.tolerance();
    let mut hint = format!(
        "Model units: {name}\nAbsolute tolerance: {} model units\nRelative tolerance: {}\nAngular tolerance: {} radians",
        tolerance.absolute(),
        tolerance.relative(),
        tolerance.angular(),
    );
    match units {
        LengthUnitSystem::Custom {
            meters_per_unit, ..
        } => {
            hint.push_str(&format!(
                "\nCustom scale: {meters_per_unit} metres per unit"
            ));
        }
        LengthUnitSystem::None => hint.push_str("\nCoordinates have no physical unit scale."),
        LengthUnitSystem::Unset => {
            hint.push_str("\nUnit scale is unknown; physical conversion is unavailable.")
        }
        _ => {}
    }
    (name, hint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::LengthUnitSystem;

    #[test]
    fn unit_status_tracks_document_history_without_mutation() {
        let mut document = Document::new(Tolerance::try_new(0.001, 0.0001, 0.00001).unwrap());
        let before = format!("{document:?}");
        let original = model_units_status(&document);
        assert_eq!(original.0, "Millimetres");
        assert!(original.1.contains("Absolute tolerance: 0.001 model units"));
        assert!(original.1.contains("Relative tolerance: 0.0001"));
        assert!(original.1.contains("Angular tolerance: 0.00001 radians"));
        assert_eq!(format!("{document:?}"), before);
        document.set_units(LengthUnitSystem::Inches, false).unwrap();
        assert_eq!(model_units_status(&document).0, "Inches");
        document.undo().unwrap();
        assert_eq!(model_units_status(&document), original);
        document.redo().unwrap();
        assert_eq!(model_units_status(&document).0, "Inches");
    }

    #[test]
    fn custom_unit_status_bounds_unicode_and_controls() {
        for (name, expected) in [
            ("界".repeat(100_000), format!("{}…", "界".repeat(24))),
            ("\n\t\r".into(), "Custom units".into()),
            ("".into(), "Custom units".into()),
            ("A\nB".into(), "A B".into()),
        ] {
            let document = Document::with_units(
                Tolerance::DEFAULT,
                LengthUnitSystem::Custom {
                    name,
                    meters_per_unit: 0.125,
                },
            )
            .unwrap();
            let (label, hint) = model_units_status(&document);
            assert_eq!(label, expected);
            assert!(hint.contains("Custom scale: 0.125 metres per unit"));
            assert!(hint.len() < 500);
        }
    }

    #[test]
    fn unitless_and_unset_have_distinct_status_and_warnings() {
        let mut document = Document::default();
        document.set_units(LengthUnitSystem::None, false).unwrap();
        let (label, hint) = model_units_status(&document);
        assert_eq!(label, "Unitless");
        assert!(hint.contains("no physical unit scale"));
        document.set_units(LengthUnitSystem::Unset, false).unwrap();
        let (label, hint) = model_units_status(&document);
        assert_eq!(label, "Unset units");
        assert!(hint.contains("physical conversion is unavailable"));
    }
}

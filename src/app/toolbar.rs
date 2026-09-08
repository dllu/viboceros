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
                let (units_label, units_hint) = model_units_status(&self.document);
                ui.weak(units_label).on_hover_text(units_hint);
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

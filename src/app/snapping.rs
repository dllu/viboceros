//! Persistent feature selection and prompt-scoped, one-pick overrides.
use super::*;
use viboceros_drafting::{ObjectSnapKind, ObjectSnapModes};

pub(super) const HELP: &str = "Snap modes: choose Point/End/Mid/Cen/Quad/Near in the toolbar menu. Right-click a mode to isolate/restore it; Shift-click for one pick. At a point prompt type Point, End, Mid, Cen, Quad, Near or NoSnap for one pick. Near and mesh-wire snapping are off by default. Enable mesh Near/Mid with SnapToMeshes Enable or the mesh-wire checkbox. SnapSize changes the active viewport grid snap spacing; ApplyTo=AllViewports changes every view. Grid settings controls grid lines and axes; F7 toggles grid lines. Ortho/F8 constrains picks from the last point; OrthoAngle sets its angular increment. Planar keeps free picks at the previous point's CPlane elevation; SetPlanar sets its state. Persistent modes are restored after an accepted point. DisableOsnap/F4 suspends persistent modes without changing the selection.";

const FEATURES: [(ObjectSnapKind, &str); 6] = [
    (ObjectSnapKind::Point, "Point"),
    (ObjectSnapKind::End, "End"),
    (ObjectSnapKind::Mid, "Mid"),
    (ObjectSnapKind::Center, "Cen"),
    (ObjectSnapKind::Quad, "Quad"),
    (ObjectSnapKind::Near, "Near"),
];

pub(super) struct SnapControls {
    pub(super) mesh_edges: bool,
    pub(super) persistent: ObjectSnapModes,
    pub(super) model_override: Option<ObjectSnapModes>,
    // Transparent CPlane prompts must not consume the suspended model prompt's override.
    pub(super) plane_override: Option<ObjectSnapModes>,
    isolated: Option<(ObjectSnapKind, ObjectSnapModes)>,
}

impl Default for SnapControls {
    fn default() -> Self {
        Self {
            mesh_edges: false,
            persistent: ObjectSnapModes::LANDMARKS,
            model_override: None,
            plane_override: None,
            isolated: None,
        }
    }
}

impl SnapControls {
    fn set(&mut self, kind: ObjectSnapKind, enabled: bool) {
        self.persistent = self.persistent.with(kind, enabled);
        self.isolated = None;
    }

    fn isolate(&mut self, kind: ObjectSnapKind) {
        if let Some((old_kind, previous)) = self.isolated
            && old_kind == kind
        {
            self.persistent = previous;
            self.isolated = None;
        } else {
            self.isolated = Some((kind, self.persistent));
            self.persistent = ObjectSnapModes::only(kind);
        }
    }
}

fn parse_one_shot(input: &str) -> Option<ObjectSnapModes> {
    let name = input
        .trim()
        .trim_start_matches(['\'', '_'])
        .to_ascii_lowercase();
    let kind = match name.as_str() {
        "point" => ObjectSnapKind::Point,
        "end" | "endpoint" => ObjectSnapKind::End,
        "mid" | "midpoint" => ObjectSnapKind::Mid,
        "cen" | "center" => ObjectSnapKind::Center,
        "quad" | "quadrant" => ObjectSnapKind::Quad,
        "near" | "nearest" => ObjectSnapKind::Near,
        "nosnap" => return Some(ObjectSnapModes::NONE),
        _ => return None,
    };
    Some(ObjectSnapModes::only(kind))
}

impl VibocerosApp {
    fn model_requests_point(&self) -> bool {
        self.edge_prompt
            .as_ref()
            .is_some_and(|p| p.split_selection().is_some())
            || (self.active_command.is_some()
                && self.active_command != Some(InteractiveCommand::DomainFace)
                && !self.picking_alignment_curve())
    }

    fn requests_snap_point(&self) -> bool {
        self.plane_prompt.as_ref().map_or_else(
            || self.model_requests_point(),
            construction_plane::PlanePrompt::requests_point,
        )
    }

    fn snap_override(&self) -> Option<ObjectSnapModes> {
        if self.plane_prompt.is_some() {
            self.snaps.plane_override
        } else {
            self.snaps.model_override
        }
    }

    pub(super) fn effective_snap_modes(&self) -> ObjectSnapModes {
        self.snap_override().unwrap_or(if self.osnap {
            self.snaps.persistent
        } else {
            ObjectSnapModes::NONE
        })
    }

    pub(super) fn discard_inactive_snap_overrides(&mut self) {
        if !self.model_requests_point() {
            self.snaps.model_override = None;
        }
        if !self
            .plane_prompt
            .as_ref()
            .is_some_and(|p| p.requests_point())
        {
            self.snaps.plane_override = None;
        }
    }

    fn set_one_shot_snap(&mut self, modes: ObjectSnapModes) {
        if !self.requests_snap_point() {
            self.push_log("One-shot object snaps require a point prompt".into());
            return;
        }
        if self.plane_prompt.is_some() {
            self.snaps.plane_override = Some(modes);
        } else {
            self.snaps.model_override = Some(modes);
        }
        self.push_log(format!(
            "Next pick: {}",
            self.one_shot_snap_label().unwrap()
        ));
    }

    pub(super) fn try_one_shot_snap(&mut self, input: &str) -> bool {
        let Some(modes) = parse_one_shot(input) else {
            return false;
        };
        // Point remains the modeling command outside a point prompt.
        if !self.requests_snap_point() {
            return false;
        }
        self.set_one_shot_snap(modes);
        self.command_input.clear();
        true
    }

    pub(super) fn one_shot_snap_label(&self) -> Option<&'static str> {
        let modes = self.snap_override()?;
        Some(
            FEATURES
                .iter()
                .find(|(kind, _)| modes.contains(*kind))
                .map_or("NoSnap", |(_, label)| *label),
        )
    }

    pub(super) fn show_snap_modes(&mut self, ui: &mut egui::Ui) {
        if ui.button("Grid settings…").clicked() {
            self.apply_interface_command(viboceros_command::interface::InterfaceCommand::Grid {
                update: viboceros_command::interface::GridUpdate::default(),
                apply_to: viboceros_command::interface::ViewportTarget::Active,
            });
            ui.close();
        }
        let mut spacing = self.viewports[self.active_viewport].snap_spacing();
        if ui
            .add(
                egui::DragValue::new(&mut spacing)
                    .speed(0.1)
                    .prefix("Grid snap spacing "),
            )
            .changed()
            && let Some(spacing) = viboceros_command::interface::SnapSpacing::try_new(spacing)
        {
            self.apply_interface_command(
                viboceros_command::interface::InterfaceCommand::SnapSize {
                    spacing: Some(spacing),
                    apply_to: viboceros_command::interface::ViewportTarget::Active,
                },
            );
        }
        ui.separator();
        ui.weak("Persistent object snaps");
        for (kind, label) in FEATURES {
            let mut enabled = self.snaps.persistent.contains(kind);
            let response = ui.checkbox(&mut enabled, label).on_hover_text(
                "Click: toggle · Right-click: isolate/restore · Shift-click: next pick only",
            );
            if response.secondary_clicked() {
                self.snaps.isolate(kind);
            } else if response.clicked() && ui.input(|i| i.modifiers.shift) {
                self.set_one_shot_snap(ObjectSnapModes::only(kind));
                ui.close();
            } else if response.changed() {
                self.snaps.set(kind, enabled);
            }
        }
        ui.separator();
        let mut mesh_edges = self.snaps.mesh_edges;
        if ui.checkbox(&mut mesh_edges, "Snap to mesh wires").changed() {
            self.apply_interface_command(
                viboceros_command::interface::InterfaceCommand::SnapToMeshes(if mesh_edges {
                    viboceros_command::interface::SwitchAction::On
                } else {
                    viboceros_command::interface::SwitchAction::Off
                }),
            );
        }
        if ui
            .add_enabled(
                self.requests_snap_point(),
                egui::Button::new("NoSnap (next pick)"),
            )
            .clicked()
        {
            self.set_one_shot_snap(ObjectSnapModes::NONE);
            ui.close();
        }
        ui.weak("Shift-click selects a one-shot snap.");
    }
}

#[cfg(test)]
mod tests;

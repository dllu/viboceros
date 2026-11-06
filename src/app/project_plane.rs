//! Infinite plane targets use the shared point input without editing CPlanes.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_command::{ObjectSelectionFilter, infinite_plane};
use viboceros_document::ObjectId;
use viboceros_geometry::Plane;

mod points;
mod reference;
mod state;
pub(super) use state::{PlaneGetter, PlaneMode};

fn plane_value(plane: Plane) -> String {
    let [x, y, z] = plane.origin().to_array();
    let [a, b, c] = plane.normal().as_vector().to_array();
    format!("{x},{y},{z},{a},{b},{c}")
}

impl VibocerosApp {
    // NamedCPlane is also a viewport command. The nested getter owns its
    // option and name text before the transparent view-command dispatcher.
    pub(super) fn continue_project_plane_name_input(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || input.starts_with('\'') {
            return false;
        }
        let Some(prompt) = self.intersection_prompt.clone() else {
            return false;
        };
        let Some(getter) = prompt.plane_getter() else {
            return false;
        };
        if getter.mode == PlaneMode::Named
            || input
                .trim_start_matches('_')
                .eq_ignore_ascii_case("NamedCPlane")
        {
            return self.continue_project_plane(&prompt, input);
        }
        false
    }
    pub(super) fn picking_project_plane_points(&self) -> bool {
        matches!(
            self.active_command,
            Some(InteractiveCommand::ProjectPlane { .. })
        )
    }
    pub(super) fn clear_project_plane_state(&mut self) {
        if self.picking_project_plane_points() {
            self.active_command = None;
            self.drafting_plane = None;
            self.point_filter = None;
            self.point_constraint = None;
            self.snaps.model_override = None;
        }
    }
    fn update_project_plane_marker(&mut self) {
        self.clear_project_plane_state();
        if let Some(p) = self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.plane_getter())
            && p.points_active()
        {
            let command = self.intersection_prompt.as_ref().unwrap().name();
            self.active_command = Some(InteractiveCommand::ProjectPlane {
                command,
                mode: p.mode,
                step: p.points.len(),
                anchor: p.points.first().copied(),
            });
        }
    }
    fn add_project_plane(&mut self, definition: String) {
        if self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .is_some_and(|p| p.is_interpolation())
        {
            self.finish_surface_interpolation_plane(&definition);
            return;
        }
        if let Some(partition) = self
            .intersection_prompt
            .as_mut()
            .and_then(|p| p.boolean.as_mut())
            .and_then(|o| o.partition.as_mut())
        {
            partition.planes.push(definition);
            partition.plane_getter = None;
            self.clear_project_plane_state();
            self.command_input.clear();
            let action = if self
                .intersection_prompt
                .as_ref()
                .unwrap()
                .boolean
                .as_ref()
                .unwrap()
                .kind
                .is_mesh_trim()
            {
                "continues to region picking"
            } else {
                "splits"
            };
            self.push_log(format!(
                "Added infinite plane cutter; Enter {action} or select another cutter"
            ));
            self.log_intersection_prompt();
            return;
        }

        if self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .is_some_and(|s| s.is_iplane())
        {
            self.finish_iplane(definition);
            return;
        }
        let s = self
            .intersection_prompt
            .as_mut()
            .unwrap()
            .projection
            .as_mut()
            .unwrap();
        s.planes.push(definition);
        s.plane_getter = None;
        self.clear_project_plane_state();
        self.command_input.clear();
        let action = if self
            .intersection_prompt
            .as_ref()
            .unwrap()
            .projection
            .as_ref()
            .unwrap()
            .is_pull()
        {
            "pulls"
        } else {
            "projects"
        };
        self.push_log(format!(
            "Added infinite plane target; Enter {action} or select another target"
        ));
        self.log_intersection_prompt();
    }
    fn accept_project_plane_frame(&mut self, frame: Frame3) {
        if self.picking_surface_interpolation_plane() {
            self.finish_surface_interpolation_frame(frame);
        } else {
            self.add_project_plane(plane_value(Plane::new(frame.origin(), frame.z_axis())));
        }
    }
    pub(super) fn begin_iplane_getter(&mut self) {
        let prompt = self.intersection_prompt.clone().unwrap();
        self.continue_project_plane(&prompt, "IP");
    }
    pub(super) fn continue_project_plane(&mut self, prompt: &TwoSetsPrompt, input: &str) -> bool {
        let input = input.trim();
        let word = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if (prompt.first.is_some()
            || prompt
                .boolean
                .as_ref()
                .is_some_and(|b| b.kind.is_mesh_trim()))
            && !self.picking_project_direction()
            && matches!(word.as_str(), "ip" | "iplane" | "infiniteplane")
        {
            self.clear_project_plane_state();
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .set_plane_getter(Some(PlaneGetter {
                    mode: PlaneMode::Choose,
                    frame: self.viewports[self.active_viewport].construction_plane(),
                    points: Vec::new(),
                    curve: None,
                }));
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        let Some(mut getter) = prompt.plane_getter().cloned() else {
            return false;
        };
        if word == "cancel" {
            if self.discard_surface_interpolation_plane() {
                return true;
            }
            self.cancel_interactive_command(true);
            return true;
        }
        if word == "undo" {
            getter.points.pop();
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .set_plane_getter(Some(getter));
            self.update_project_plane_marker();
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        if input.is_empty() {
            if self.discard_surface_interpolation_plane() {
                return true;
            }
            if getter.mode == PlaneMode::Choose {
                let p = infinite_plane::axis_plane(
                    "CPlaneXY",
                    self.viewports[self.active_viewport].construction_plane(),
                )
                .unwrap();
                self.add_project_plane(plane_value(p));
            } else if prompt.projection.as_ref().is_some_and(|p| p.single_plane()) {
                self.begin_iplane_getter();
            } else {
                self.intersection_prompt
                    .as_mut()
                    .unwrap()
                    .set_plane_getter(None);
                self.clear_project_plane_state();
                self.command_input.clear();
                self.log_intersection_prompt();
            }
            return true;
        }
        if getter.mode == PlaneMode::Named {
            let result = self
                .named_cplanes
                .get(input.trim_matches('"'))
                .map(|p| p.plane);
            match result {
                Ok(p) => self.accept_project_plane_frame(p),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        if let Some(plane) = infinite_plane::axis_plane(
            input,
            self.viewports[self.active_viewport].construction_plane(),
        ) {
            if self.picking_surface_interpolation_plane() {
                let frame = infinite_plane::axis_frame(
                    input,
                    self.viewports[self.active_viewport].construction_plane(),
                )
                .unwrap();
                self.accept_project_plane_frame(frame);
            } else {
                self.add_project_plane(plane_value(plane));
            }
            return true;
        }
        let mode = match word.as_str() {
            "rectangle" => Some(PlaneMode::Rectangle),
            "3point" => Some(PlaneMode::ThreePoint),
            "vertical" => Some(PlaneMode::Vertical),
            "throughpoint" => Some(PlaneMode::Through),
            "object" => Some(PlaneMode::Object),
            "aroundcurve" => Some(PlaneMode::AroundCurve),
            "namedcplane" => Some(PlaneMode::Named),
            _ => None,
        };
        if let Some(mode) = mode {
            getter.mode = mode;
            getter.points.clear();
            getter.curve = None;
            getter.frame = self.viewports[self.active_viewport].construction_plane();
            self.intersection_prompt
                .as_mut()
                .unwrap()
                .set_plane_getter(Some(getter));
            self.update_project_plane_marker();
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        if matches!(
            getter.mode,
            PlaneMode::Choose | PlaneMode::Object | PlaneMode::AroundCurve
        ) && !getter.points_active()
        {
            let words = input.split_whitespace().collect::<Vec<_>>();
            let parsed = match words.as_slice() {
                [id] => id.parse::<ObjectId>().ok().map(|id| (id, None)),
                [id, face] => {
                    id.parse::<ObjectId>()
                        .ok()
                        .zip(face.trim_start_matches('_').split_once('=').and_then(
                            |(key, value)| {
                                key.eq_ignore_ascii_case("Face")
                                    .then(|| value.parse::<usize>().ok())
                                    .flatten()
                            },
                        ))
                        .map(|(id, face)| (id, Some(face)))
                }
                _ => None,
            };
            if let Some((id, face)) = parsed {
                if getter.mode == PlaneMode::AroundCurve {
                    self.pick_project_plane_reference(&[id]);
                } else {
                    self.accept_project_object_plane(id, face);
                }
                self.command_input.clear();
                return true;
            }
        }
        if getter.points_active()
            && (self.try_continue_point_constraint(input)
                || self.try_continue_point_filter(input)
                || self.try_continue_point_input(input))
        {
            return true;
        }
        // Parent options and commands retain their existing transition paths.
        if input.contains('=')
            || self.commands.recognizes(
                input
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches(['_', '-']),
            )
        {
            return false;
        }
        self.command_input.clear();
        self.log_intersection_prompt();
        true
    }
}

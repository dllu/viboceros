//! Plane selection and reference picks use the affine copy/history session.
use super::*;
use viboceros_command::scale_by_plane::{PlaneChoice, Prompt};

impl VibocerosApp {
    pub(super) fn picking_scale_by_plane_object(&self) -> bool {
        matches!(self.active_command, Some(InteractiveCommand::ScaleByPlane(p)) if p.plane.is_none() && p.options.plane == PlaneChoice::Object)
    }

    pub(super) fn picking_scale_by_plane_view(&self) -> bool {
        matches!(self.active_command, Some(InteractiveCommand::ScaleByPlane(p)) if p.plane.is_none() && p.options.plane == PlaneChoice::FromView)
    }

    pub(super) fn accept_scale_by_plane_view(&mut self, index: usize) -> bool {
        if !self.picking_scale_by_plane_view() {
            return false;
        }
        let Some(view) = self.viewports.get(index) else {
            return false;
        };
        self.finish_scale_by_plane_frame(view.construction_plane());
        true
    }

    pub(super) fn accept_scale_by_plane_object(
        &mut self,
        id: ObjectId,
        face: Option<usize>,
    ) -> bool {
        if !self.picking_scale_by_plane_object() {
            return false;
        }
        match viboceros_command::scale_by_plane::object_frame(&self.document, id, face) {
            Ok(frame) => {
                self.finish_scale_by_plane_frame(frame);
                if let Some(InteractiveCommand::ScaleByPlane(ref mut prompt)) = self.active_command
                {
                    prompt.plane_target = Some((id, face));
                }
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    fn finish_scale_by_plane_frame(&mut self, frame: Frame3) {
        let Some(InteractiveCommand::ScaleByPlane(mut prompt)) = self.active_command else {
            return;
        };
        prompt.plane = Some(frame);
        prompt.plane_menu = false;
        self.active_command = Some(InteractiveCommand::ScaleByPlane(prompt));
        self.command_input.clear();
        self.push_log(prompt.prompt().into());
    }

    pub(super) fn try_continue_scale_by_plane(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ScaleByPlane(mut prompt)) = self.active_command else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if self.picking_scale_by_plane_object() {
            let arguments = word.split_whitespace().collect::<Vec<_>>();
            if arguments
                .first()
                .is_some_and(|s| s.parse::<ObjectId>().is_ok())
            {
                match viboceros_command::scale_by_plane::object_target(&arguments) {
                    Ok((id, face)) => {
                        self.accept_scale_by_plane_object(id, face);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                return true;
            }
        }
        if self.picking_scale_by_plane_view()
            && (word.is_empty() || word.eq_ignore_ascii_case("Enter"))
        {
            self.accept_scale_by_plane_view(self.active_viewport);
            return true;
        }
        if word.eq_ignore_ascii_case("Rigid") || word.to_ascii_lowercase().starts_with("rigid=") {
            let rigid = if word.eq_ignore_ascii_case("Rigid") {
                Some(!prompt.options.rigid)
            } else {
                word.split_once('=').and_then(|(_, value)| {
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "yes" => Some(true),
                        "no" => Some(false),
                        _ => None,
                    }
                })
            };
            if let Some(rigid) = rigid {
                if self.update_scale_nu_rigid_layout(rigid) {
                    prompt.options.rigid = rigid;
                    self.commands.remember_rigid_option("ScaleByPlane", rigid);
                    self.active_command = Some(InteractiveCommand::ScaleByPlane(prompt));
                }
            } else {
                self.push_log("Error: Rigid must be Yes or No".into());
            }
            self.command_input.clear();
            return true;
        }
        let plane_value = word
            .split_once('=')
            .filter(|(n, _)| n.eq_ignore_ascii_case("Plane"))
            .map(|(_, v)| v)
            .or_else(|| {
                word.split_once(' ')
                    .filter(|(n, _)| n.eq_ignore_ascii_case("Plane"))
                    .map(|(_, v)| v)
            });
        if word.eq_ignore_ascii_case("Plane") {
            if prompt.origin.is_some() {
                self.push_log("Choose Plane before the origin".into());
            } else {
                prompt.plane_menu = true;
                self.active_command = Some(InteractiveCommand::ScaleByPlane(prompt));
                self.push_log(prompt.prompt().into());
            }
            self.command_input.clear();
            return true;
        }
        if plane_value.is_some() || prompt.plane_menu || PlaneChoice::parse(word).is_some() {
            if prompt.origin.is_some() {
                self.push_log("Choose Plane before the origin".into());
            } else if let Some(choice) = PlaneChoice::parse(plane_value.unwrap_or(word)) {
                prompt = Prompt::new(
                    viboceros_command::scale_by_plane::Options {
                        plane: choice,
                        ..prompt.options
                    },
                    self.viewports[self.active_viewport].construction_plane(),
                );
                self.active_command = Some(InteractiveCommand::ScaleByPlane(prompt));
                self.push_log(prompt.prompt().into());
            } else {
                self.push_log("Error: invalid ScaleByPlane plane".into());
            }
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        false
    }

    pub(super) fn accept_scale_by_plane_point(
        &mut self,
        mut prompt: Prompt,
        point: Point3,
    ) -> bool {
        if prompt.plane_menu {
            return false;
        }
        let Some(_) = prompt.plane else {
            if prompt.options.plane != PlaneChoice::ThreePoint {
                return false;
            }
            let result = match prompt.plane_points {
                [None, _] => {
                    prompt.plane_points[0] = Some(point);
                    Ok(None)
                }
                [Some(origin), None] => origin
                    .vector_to(point)
                    .and_then(|v| v.normalized(self.document.tolerance()))
                    .map(|_| {
                        prompt.plane_points[1] = Some(point);
                        None
                    }),
                [Some(origin), Some(x)] => {
                    Frame3::try_from_points(origin, x, point, self.document.tolerance()).map(Some)
                }
            };
            return match result {
                Ok(frame) => {
                    prompt.plane = frame;
                    self.active_command = Some(InteractiveCommand::ScaleByPlane(prompt));
                    self.push_log(prompt.prompt().into());
                    true
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    false
                }
            };
        };
        if let Some(origin) = prompt.origin {
            if let Some(reference) = prompt.reference {
                // The session supplies its captured frame through context,
                // retaining full axis precision across viewport changes.
                let definition = if let Some((id, face)) = prompt.plane_target {
                    face.map_or_else(
                        || format!("Plane=Object {id}"),
                        |face| format!("Plane=Object {id} Face={face}"),
                    )
                } else {
                    "Plane=ActiveCPlane".into()
                };
                let script = format!(
                    "ScaleByPlane {definition} {} {} {} Rigid={}",
                    format_model_point(origin),
                    format_model_point(reference),
                    format_model_point(point),
                    if prompt.options.rigid { "Yes" } else { "No" }
                );
                return self
                    .apply_transform_step(&script, InteractiveCommand::ScaleByPlane(prompt));
            }
            prompt.reference = Some(point);
        } else {
            prompt.origin = Some(point);
        }
        self.active_command = Some(InteractiveCommand::ScaleByPlane(prompt));
        self.push_log(prompt.prompt().into());
        true
    }
}

//! Independent axis prompts share the existing affine copy/history session.
use super::*;
use viboceros_command::nonuniform_scale::{
    ScaleNuPrompt, constrained_reference_factor, reference_factor,
};

impl VibocerosApp {
    pub(super) fn scale_nu_plane(&self, prompt: ScaleNuPrompt) -> Frame3 {
        if prompt.world {
            viboceros_command::CommandContext::default().construction_plane
        } else {
            self.drafting_plane
                .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane())
        }
    }

    pub(super) fn try_continue_scale_nu(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ScaleNu(mut prompt)) = self.active_command else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("WorldCoordinates") {
            if prompt.origin.is_none() {
                prompt.world = true;
                self.active_command = Some(InteractiveCommand::ScaleNu(prompt));
                self.push_log("ScaleNU directions: World coordinates".into());
            } else {
                self.push_log("Choose WorldCoordinates before the origin".into());
            }
            self.command_input.clear();
            return true;
        }
        let enter = input.trim().is_empty() || word.eq_ignore_ascii_case("Enter");
        let value = if enter {
            if prompt.origin.is_none() {
                self.cancel_interactive_command(true);
                self.command_input.clear();
                return true;
            }
            if prompt.reference.is_some() {
                self.push_log("Pick the second axis reference point".into());
                self.command_input.clear();
                return true;
            }
            self.commands.axis_scale_defaults("ScaleNU").unwrap()[prompt.axis().unwrap()]
        } else if let Ok(value) = input.parse::<f64>() {
            value
        } else {
            return false;
        };
        if prompt.reference.is_some() {
            if value.is_finite() {
                prompt.distance = Some(value.abs());
                self.active_command = Some(InteractiveCommand::ScaleNu(prompt));
                self.push_log(format!(
                    "Axis distance: {}; pick the second reference point",
                    value.abs()
                ));
            } else {
                self.push_log("Error: axis distances must be finite".into());
            }
        } else if prompt.origin.is_none() {
            self.push_log("ScaleNU requires a point at this prompt".into());
        } else {
            self.accept_scale_nu_factor(prompt, value);
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_scale_nu_point(
        &mut self,
        mut prompt: ScaleNuPrompt,
        point: Point3,
    ) -> bool {
        let Some(origin) = prompt.origin else {
            prompt.origin = Some(point);
            self.active_command = Some(InteractiveCommand::ScaleNu(prompt));
            self.push_log(prompt.prompt().into());
            return true;
        };
        let axis = prompt.axis().unwrap();
        let plane = self.scale_nu_plane(prompt);
        if let Some(reference) = prompt.reference {
            match constrained_reference_factor(
                plane,
                origin,
                axis,
                reference,
                point,
                prompt.distance,
                self.document.tolerance(),
            ) {
                Ok(value) => self.accept_scale_nu_factor(prompt, value),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    false
                }
            }
        } else {
            // Validate the axis distance before retaining a reference. Other
            // coordinates do not affect this axis's scale.
            if let Err(error) =
                reference_factor(plane, origin, axis, point, point, self.document.tolerance())
            {
                self.push_log(format!("Error: {error}"));
                return false;
            }
            let previous = self
                .affine_preview()
                .and_then(|preview| preview.last_transform);
            prompt.reference = Some(point);
            self.active_command = Some(InteractiveCommand::ScaleNu(prompt));
            self.update_affine_preview(previous);
            self.push_log(prompt.prompt().into());
            true
        }
    }

    fn accept_scale_nu_factor(&mut self, mut prompt: ScaleNuPrompt, value: f64) -> bool {
        if !value.is_finite() {
            self.push_log("Error: scale factors must be finite".into());
            return false;
        }
        let axis = prompt.axis().unwrap();
        self.commands.remember_axis_scale("ScaleNU", axis, value);
        prompt.factors[axis] = Some(value);
        prompt.reference = None;
        prompt.distance = None;
        let [Some(x), Some(y), Some(z)] = prompt.factors else {
            self.active_command = Some(InteractiveCommand::ScaleNu(prompt));
            let previous = self
                .affine_preview()
                .and_then(|preview| preview.last_transform);
            self.update_affine_preview(previous);
            self.push_log(prompt.prompt().into());
            return true;
        };
        let origin = prompt.origin.unwrap();
        let script = format!(
            "ScaleNU {} {x} {y} {z}{}",
            format_model_point(origin),
            if prompt.world {
                " WorldCoordinates"
            } else {
                ""
            }
        );
        // Copy repeats from the same sources and origin, beginning with X.
        let continuation = ScaleNuPrompt {
            origin: Some(origin),
            ..ScaleNuPrompt::new(prompt.world)
        };
        self.apply_transform_step(&script, InteractiveCommand::ScaleNu(continuation))
    }
}

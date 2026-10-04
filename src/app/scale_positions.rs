//! ScalePositions point phases retain their initial numeric getter mode.
use super::*;
use viboceros_command::scale_positions::{
    ScaleMode, ScalePositionsPrompt, reference_factor, scale_map,
};

impl VibocerosApp {
    pub(super) fn try_continue_scale_positions(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ScalePositions(mut prompt)) = self.active_command else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(n, v)| (n, Some(v)));
        if prompt.choosing_mode || name.eq_ignore_ascii_case("Mode") {
            if prompt.factor.is_some() || prompt.reference.is_some() {
                self.push_log("Choose Mode before entering a factor or first reference".into());
                self.command_input.clear();
                return true;
            }
            let value = if prompt.choosing_mode {
                Some(word)
            } else {
                value
            };
            if let Some(value) = value {
                if let Some(mode) = ScaleMode::parse(value) {
                    prompt.mode = mode;
                    prompt.choosing_mode = false;
                } else {
                    self.push_log("Choose Mode=1D, Mode=2D or Mode=3D".into());
                }
            } else {
                prompt.choosing_mode = true;
            }
            self.active_command = Some(InteractiveCommand::ScalePositions(prompt));
            self.push_log(prompt.prompt().into());
            self.command_input.clear();
            return true;
        }
        let enter = word.is_empty() || word.eq_ignore_ascii_case("Enter");
        if enter && prompt.origin.is_none() {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if prompt.factor.is_some() {
            return false;
        }
        let value = if enter {
            self.commands
                .transform_scalar_default("ScalePositions")
                .unwrap()
        } else if let Ok(value) = input.parse::<f64>() {
            value.abs()
        } else {
            return false;
        };
        if !value.is_finite() || value == 0. {
            self.push_log("ScalePositions requires a finite nonzero scale factor".into());
        } else if prompt.origin.is_none() {
            self.push_log("Pick an origin before entering a factor".into());
        } else {
            prompt.factor = Some(value);
            if prompt.reference.is_some() {
                self.apply_position_scale(prompt, None, None);
            } else if prompt.getter_mode == ScaleMode::OneDimensional {
                self.active_command = Some(InteractiveCommand::ScalePositions(prompt));
                self.push_log(prompt.prompt().into());
            } else {
                self.apply_position_scale(prompt, None, None);
            }
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_scale_positions_point(
        &mut self,
        mut prompt: ScalePositionsPrompt,
        point: Point3,
    ) -> bool {
        if prompt.choosing_mode {
            self.push_log(prompt.prompt().into());
            return false;
        }
        let Some(origin) = prompt.origin else {
            prompt.origin = Some(point);
            self.active_command = Some(InteractiveCommand::ScalePositions(prompt));
            self.push_log(prompt.prompt().into());
            return true;
        };
        if prompt.factor.is_some() {
            self.apply_position_scale(prompt, Some(point), None)
        } else if let Some(reference) = prompt.reference {
            match reference_factor(
                prompt.mode,
                origin,
                reference,
                point,
                self.document.tolerance(),
            ) {
                Ok(factor) => {
                    prompt.factor = Some(factor);
                    self.apply_position_scale(prompt, Some(reference), Some(point))
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    false
                }
            }
        } else {
            if let Err(error) =
                reference_factor(prompt.mode, origin, point, point, self.document.tolerance())
            {
                self.push_log(format!("Error: {error}"));
                return false;
            }
            prompt.reference = Some(point);
            self.active_command = Some(InteractiveCommand::ScalePositions(prompt));
            self.push_log(prompt.prompt().into());
            true
        }
    }

    fn apply_position_scale(
        &mut self,
        prompt: ScalePositionsPrompt,
        direction: Option<Point3>,
        target: Option<Point3>,
    ) -> bool {
        let origin = prompt.origin.unwrap();
        let factor = prompt.factor.unwrap();
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        if let Err(error) = scale_map(
            prompt.mode,
            plane,
            origin,
            factor,
            direction,
            self.document.tolerance(),
        ) {
            self.push_log(format!("Error: {error}"));
            return false;
        }
        let script = if let Some(target) = target {
            format!(
                "ScalePositions {} {} {} Mode={}",
                format_model_point(origin),
                format_model_point(prompt.reference.unwrap()),
                format_model_point(target),
                prompt.mode.name()
            )
        } else {
            format!(
                "ScalePositions {} {}{} Mode={}",
                format_model_point(origin),
                factor,
                direction.map_or(String::new(), |p| format!(" {}", format_model_point(p))),
                prompt.mode.name()
            )
        };
        let continuation = ScalePositionsPrompt {
            factor: if prompt.reference.is_none() && prompt.getter_mode == ScaleMode::OneDimensional
            {
                Some(factor)
            } else {
                None
            },
            ..prompt
        };
        self.apply_transform_step(&script, InteractiveCommand::ScalePositions(continuation))
    }
}

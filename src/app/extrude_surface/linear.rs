//! Staged straight direction, base point, and distance input.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn try_start_surface_extrusion_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrf")
        }) {
            return false;
        }
        let Ok(parsed) = SurfaceExtrusionInput::parse_with_options(
            &words.collect::<Vec<_>>(),
            self.commands.surface_extrusion_settings().options,
        ) else {
            return false;
        };
        let mut sources = match self.prepare_surface_extrusion_sources(parsed.faces) {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        sources.linear = Some(LinearSession {
            draft: SurfaceExtrusionDraft::capture(&self.document, &sources.targets),
            plane: self.viewports[self.active_viewport].construction_plane(),
            base: None,
            hover: None,
        });
        self.cancel_interactive_command(false);
        self.surface_extrusion_sources = Some(sources);
        self.active_command = Some(InteractiveCommand::ExtrudeSurface {
            stage: Stage::Distance,
            options: parsed.options,
        });
        self.command_input.clear();
        self.log_surface_extrusion_prompt(Stage::Distance, parsed.options);
        true
    }
    pub(in crate::app) fn try_continue_surface_extrusion(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeSurface { stage, mut options }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.set_view_prompt.is_some() {
            return false;
        }
        let keyword = input.trim_start_matches(['_', '-']);
        let next = if keyword.eq_ignore_ascii_case("Direction") {
            Some(Stage::DirectionStart)
        } else if keyword.eq_ignore_ascii_case("SetBasePoint") {
            Some(Stage::BasePoint)
        } else {
            None
        };
        if let Some(stage) = next {
            self.set_surface_extrusion_stage(stage, options);
            self.command_input.clear();
            return true;
        }
        if stage == Stage::Distance && (input.is_empty() || keyword.eq_ignore_ascii_case("Enter")) {
            if let Some(distance) = self.commands.surface_extrusion_settings().distance {
                self.accept_surface_extrusion_length(distance, options);
            } else {
                self.log_surface_extrusion_prompt(stage, options);
            }
            self.command_input.clear();
            return true;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return false;
        }
        if words
            .iter()
            .all(|s| s.contains('=') || SurfaceExtrusionOptions::recognizes(s))
        {
            let old_direction = options.direction;
            match options.update(&words) {
                Ok(()) => {
                    if options.direction != old_direction {
                        self.reset_surface_extrusion_hover();
                    }
                    self.active_command =
                        Some(InteractiveCommand::ExtrudeSurface { stage, options });
                    self.log_surface_extrusion_prompt(stage, options);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        if input == "0" || input.eq_ignore_ascii_case("w0") {
            let point = if input == "0" {
                self.viewports[self.active_viewport]
                    .construction_plane()
                    .origin()
            } else {
                Point3::try_new(0., 0., 0.).unwrap()
            };
            self.accept_drafting_point(point);
            self.command_input.clear();
            return true;
        }
        if stage == Stage::Distance
            && let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            )
        {
            match length {
                Ok(h) => {
                    self.accept_surface_extrusion_length(h, options);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        false
    }

    fn accept_surface_extrusion_length(&mut self, distance: f64, options: SurfaceExtrusionOptions) {
        let direction = self
            .surface_extrusion_preview()
            .and_then(|preview| preview.hover)
            .map_or(1., f64::signum);
        self.finish_surface_extrusion(distance * direction, distance, options);
    }

    fn reset_surface_extrusion_hover(&mut self) {
        if let Some(session) = self
            .surface_extrusion_sources
            .as_mut()
            .and_then(|sources| sources.linear.as_mut())
        {
            session.hover = None;
        }
    }

    fn set_surface_extrusion_stage(&mut self, stage: Stage, options: SurfaceExtrusionOptions) {
        self.reset_surface_extrusion_hover();
        let command = InteractiveCommand::ExtrudeSurface { stage, options };
        self.active_command = Some(command);
        self.log_surface_extrusion_prompt(stage, options);
    }

    fn log_surface_extrusion_prompt(&mut self, stage: Stage, options: SurfaceExtrusionOptions) {
        if stage == Stage::Distance {
            let distance = self
                .commands
                .surface_extrusion_settings()
                .distance
                .unwrap_or(0.);
            self.push_log(format!(
                "Extrusion distance <{distance}> (Direction SetBasePoint {})",
                options.command_options(),
            ));
        } else {
            self.push_log(
                InteractiveCommand::ExtrudeSurface { stage, options }
                    .prompt()
                    .into(),
            );
        }
    }

    pub(in crate::app) fn accept_surface_extrusion_point(
        &mut self,
        stage: Stage,
        mut options: SurfaceExtrusionOptions,
        point: Point3,
    ) -> bool {
        match stage {
            Stage::Distance => {
                let Some(height) = self
                    .surface_extrusion_preview()
                    .and_then(|preview| signed_height(preview.constraint, point))
                else {
                    self.push_log(
                        "Error: surface source changed or has no extrusion boundary".into(),
                    );
                    return false;
                };
                return self.finish_surface_extrusion(height, height.abs(), options);
            }
            Stage::BasePoint => {
                if let Some(session) = self
                    .surface_extrusion_sources
                    .as_mut()
                    .and_then(|sources| sources.linear.as_mut())
                {
                    session.base = Some(point);
                }
            }
            Stage::DirectionStart => {
                self.set_surface_extrusion_stage(Stage::DirectionEnd(point), options);
                return true;
            }
            Stage::DirectionEnd(start) => {
                if start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: extrusion direction points must differ".into());
                    return false;
                }
                let Ok(direction) = start.direction_to(point) else {
                    return false;
                };
                options.direction = Some(direction.as_vector());
            }
        }
        self.set_surface_extrusion_stage(Stage::Distance, options);
        true
    }

    pub(in crate::app) fn surface_extrusion_preview(&self) -> Option<SurfaceExtrusionPreview<'_>> {
        if self.plane_prompt.is_some()
            || self.nested_measurement.is_some()
            || self.set_view_prompt.is_some()
        {
            return None;
        }
        self.surface_extrusion_sources
            .as_ref()?
            .linear_preview(&self.document, self.active_command)
    }

    pub(in crate::app) fn observe_surface_extrusion_hover(
        &mut self,
        point: Option<Point3>,
    ) -> bool {
        let Some(height) = point
            .zip(self.surface_extrusion_preview())
            .and_then(|(point, preview)| signed_height(preview.constraint, point))
            .filter(|value| value.abs() > self.document.tolerance().absolute())
        else {
            return false;
        };
        let session = self
            .surface_extrusion_sources
            .as_mut()
            .unwrap()
            .linear
            .as_mut()
            .unwrap();
        if session.hover == Some(height) {
            return false;
        }
        session.hover = Some(height);
        true
    }
}

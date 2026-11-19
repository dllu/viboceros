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
        let stage = if parsed.boundary == BoundaryInput::None {
            Stage::Distance
        } else {
            Stage::Boundary
        };
        self.command_input.clear();
        self.set_surface_extrusion_stage(stage, parsed.options);
        if let BoundaryInput::Reference(reference) = parsed.boundary {
            self.pick_straight_surface_boundary(reference);
        }
        true
    }

    pub(in crate::app) fn picking_straight_surface_boundary(&self) -> bool {
        self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && matches!(
                self.active_command,
                Some(InteractiveCommand::ExtrudeSurface {
                    stage: Stage::Boundary,
                    ..
                })
            )
    }

    pub(super) fn pick_straight_surface_boundary(&mut self, reference: BoundaryRef) {
        let Some(InteractiveCommand::ExtrudeSurface {
            stage: Stage::Boundary,
            options,
        }) = self.active_command
        else {
            return;
        };
        self.finish_surface_face_command(&format!(
            "ExtrudeSrf ToBoundary={reference} {}",
            options.command_options()
        ));
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
        let (option, reference) = keyword
            .split_once('=')
            .map_or((keyword, None), |(key, value)| (key, Some(value)));
        if option.eq_ignore_ascii_case("ToBoundary") {
            self.set_surface_extrusion_stage(Stage::Boundary, options);
            if let Some(reference) = reference {
                self.enter_straight_surface_boundary(reference);
            }
            self.command_input.clear();
            return true;
        }
        let next = if keyword.eq_ignore_ascii_case("Direction") {
            let to_boundary = matches!(
                stage,
                Stage::Boundary
                    | Stage::DirectionStart { to_boundary: true }
                    | Stage::DirectionEnd {
                        to_boundary: true,
                        ..
                    }
            );
            Some(Stage::DirectionStart { to_boundary })
        } else if stage == Stage::Distance && keyword.eq_ignore_ascii_case("SetBasePoint") {
            Some(Stage::BasePoint)
        } else {
            None
        };
        if let Some(stage) = next {
            self.set_surface_extrusion_stage(stage, options);
            self.command_input.clear();
            return true;
        }
        if input.is_empty() || keyword.eq_ignore_ascii_case("Enter") {
            match stage {
                Stage::Distance => {
                    if let Some(distance) = self.commands.surface_extrusion_settings().distance {
                        self.accept_surface_extrusion_length(distance, options);
                    } else {
                        self.log_surface_extrusion_prompt(stage, options);
                    }
                }
                Stage::Boundary => self.cancel_interactive_command(true),
                Stage::BasePoint => self.set_surface_extrusion_base(None, options),
                Stage::DirectionStart { to_boundary } | Stage::DirectionEnd { to_boundary, .. } => {
                    self.set_surface_extrusion_stage(Stage::after_direction(to_boundary), options);
                }
            }
            self.command_input.clear();
            return true;
        }
        if stage == Stage::Boundary {
            if keyword.eq_ignore_ascii_case("Cancel") || input == "!" {
                self.cancel_interactive_command(true);
            } else {
                let reference = keyword
                    .split_once(char::is_whitespace)
                    .filter(|(name, _)| {
                        name.trim_start_matches(['_', '\''])
                            .eq_ignore_ascii_case("SelID")
                    })
                    .map_or(keyword, |(_, id)| id.trim());
                self.enter_straight_surface_boundary(reference);
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

    fn enter_straight_surface_boundary(&mut self, input: &str) {
        match BoundaryRef::parse(input) {
            Ok(reference) => self.pick_straight_surface_boundary(reference),
            Err(_) => {
                self.push_log("Select a boundary surface or enter SelID with its object ID".into())
            }
        }
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

    fn set_surface_extrusion_base(
        &mut self,
        base: Option<Point3>,
        options: SurfaceExtrusionOptions,
    ) {
        if let Some(session) = self
            .surface_extrusion_sources
            .as_mut()
            .and_then(|sources| sources.linear.as_mut())
        {
            session.base = base;
        }
        self.set_surface_extrusion_stage(Stage::Distance, options);
    }

    fn log_surface_extrusion_prompt(&mut self, stage: Stage, options: SurfaceExtrusionOptions) {
        if stage == Stage::Distance {
            let distance = self
                .commands
                .surface_extrusion_settings()
                .distance
                .unwrap_or(0.);
            self.push_log(format!(
                "Extrusion distance <{distance}> (Direction SetBasePoint ToBoundary {})",
                options.command_options(),
            ));
            return;
        }
        let prompt = InteractiveCommand::ExtrudeSurface { stage, options }.prompt();
        if matches!(
            stage,
            Stage::DirectionStart { .. } | Stage::DirectionEnd { .. }
        ) {
            let axis = options.direction.or_else(|| {
                let session = self.surface_extrusion_sources.as_ref()?.linear.as_ref()?;
                Some(
                    session
                        .draft
                        .as_ref()?
                        .constraint(None, options, session.plane)
                        .direction?
                        .as_vector(),
                )
            });
            if let Some(axis) = axis {
                let [x, y, z] = axis.to_array();
                self.push_log(format!("{prompt} <{x:.3},{y:.3},{z:.3}>"));
                return;
            }
        }
        self.push_log(prompt.into());
    }

    pub(in crate::app) fn accept_surface_extrusion_point(
        &mut self,
        stage: Stage,
        mut options: SurfaceExtrusionOptions,
        point: Point3,
    ) -> bool {
        match stage {
            Stage::Boundary => return false,
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
                self.set_surface_extrusion_base(Some(point), options);
            }
            Stage::DirectionStart { to_boundary } => {
                self.set_surface_extrusion_stage(
                    Stage::DirectionEnd {
                        start: point,
                        to_boundary,
                    },
                    options,
                );
            }
            Stage::DirectionEnd { start, to_boundary } => {
                if start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: extrusion direction points must differ".into());
                    return false;
                }
                let Ok(direction) = start.direction_to(point) else {
                    return false;
                };
                options.direction = Some(direction.as_vector());
                self.set_surface_extrusion_stage(Stage::after_direction(to_boundary), options);
            }
        }
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

impl Stage {
    fn after_direction(to_boundary: bool) -> Self {
        if to_boundary {
            Self::Boundary
        } else {
            Self::Distance
        }
    }
}

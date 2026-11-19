//! Distance, direction, and base-point getters for tapered extrusion.
use super::*;
use crate::viewport::surface_extrusion::signed_height;
use viboceros_command::{
    extrude_surface::SurfaceFaces, extrude_surface_tapered::SurfaceTaperedInput,
    extrude_tapered::TaperedExtrusionOptions, surface_reference::BoundaryInput,
    translation::DestinationConstraint,
};
use viboceros_geometry::Vector3;

mod boundary;
mod preview;
mod sources;
use sources::Sources;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Stage {
    Distance,
    BasePoint,
    Boundary,
    DirectionStart { to_boundary: bool },
    DirectionEnd { start: Point3, to_boundary: bool },
    Angle,
    Corners,
}

impl Stage {
    pub(super) const fn prompt(self) -> &'static str {
        match self {
            Self::Distance => "Extrusion distance",
            Self::BasePoint => "BasePoint",
            Self::Boundary => "Select a boundary surface",
            Self::DirectionStart { .. } => "Base point for direction",
            Self::DirectionEnd { .. } => "Second direction point",
            Self::Angle => "Draft angle",
            Self::Corners => "Corners (Sharp Round Smooth)",
        }
    }

    fn after_direction(to_boundary: bool) -> Self {
        if to_boundary {
            Self::Boundary
        } else {
            Self::Distance
        }
    }
}

pub(super) struct Session {
    sources: Sources,
    plane: Frame3,
    base: Option<Point3>,
    hover: Option<f64>,
    preview: preview::Preview,
}

impl Session {
    fn constraint(&self, options: TaperedExtrusionOptions) -> DestinationConstraint {
        DestinationConstraint {
            anchor: self.base.unwrap_or(self.sources.center),
            direction: Some(
                options
                    .direction
                    .or(self.sources.normal)
                    .and_then(|vector| vector.normalized_nonzero().ok())
                    .unwrap_or(self.plane.z_axis()),
            ),
            distance: None,
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_start_tapered_extrusion_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(command) = words.next().map(|word| word.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let surface = command.eq_ignore_ascii_case("ExtrudeSrfTapered");
        if !surface && !command.eq_ignore_ascii_case("ExtrudeCrvTapered") {
            return false;
        }
        let settings = self.commands.tapered_extrusion_settings(surface);
        let words = words.collect::<Vec<_>>();
        let (options, faces, sources, boundary) = if surface {
            let Ok(parsed) = SurfaceTaperedInput::parse_with_options(&words, settings.options)
            else {
                return false;
            };
            let faces = match self.prepare_surface_extrusion_sources(parsed.faces) {
                Ok(Some(faces)) => faces,
                Ok(None) => return false,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            };
            let Some(sources) = Sources::faces(&self.document, &faces.targets) else {
                return false;
            };
            (parsed.options, Some(faces), sources, parsed.boundary)
        } else {
            let Ok((options, boundary)) =
                TaperedExtrusionOptions::parse_command_options(&words, settings.options)
            else {
                return false;
            };
            let Some(sources) = Sources::curves(&self.document) else {
                return false;
            };
            (options, None, sources, boundary)
        };
        self.cancel_interactive_command(false);
        self.surface_extrusion_sources = faces;
        self.tapered_extrusion_session = Some(Session {
            sources,
            plane: self.viewports[self.active_viewport].construction_plane(),
            base: None,
            hover: None,
            preview: preview::Preview::default(),
        });
        self.command_input.clear();
        self.open_tapered_boundary_or_distance(surface, boundary, options);
        true
    }

    fn set_tapered_stage(&mut self, surface: bool, stage: Stage, options: TaperedExtrusionOptions) {
        let command = InteractiveCommand::ExtrudeTapered {
            surface,
            stage,
            options,
        };
        self.active_command = Some(command);
        if stage == Stage::Distance {
            let distance = self.commands.tapered_extrusion_settings(surface).distance;
            self.push_log(format!(
                "Extrusion distance <{distance}> (Direction {} FlipAngle ToBoundary SetBasePoint)",
                options.command_options(surface),
            ));
        } else if stage == Stage::Boundary && surface {
            self.push_log("Select a boundary surface (Direction)".into());
        } else if matches!(
            stage,
            Stage::DirectionStart { .. } | Stage::DirectionEnd { .. }
        ) && let Some(axis) = self
            .tapered_extrusion_session
            .as_ref()
            .and_then(|session| session.constraint(options).direction)
        {
            let [x, y, z] = axis.as_vector().to_array();
            self.push_log(format!("{} <{x:.3},{y:.3},{z:.3}>", stage.prompt()));
        } else {
            self.push_log(stage.prompt().into());
        }
    }

    pub(super) fn try_continue_tapered_extrusion(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeTapered {
            surface,
            stage,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.set_view_prompt.is_some() {
            return false;
        }
        let keyword = input.trim_start_matches(['_', '-']);
        if keyword.eq_ignore_ascii_case("Cancel") || input == "!" {
            return false;
        }
        if stage == Stage::Boundary {
            self.continue_tapered_boundary(surface, keyword, options);
        } else if input.is_empty() || keyword.eq_ignore_ascii_case("Enter") {
            if stage == Stage::Distance {
                let distance = self.commands.tapered_extrusion_settings(surface).distance;
                self.accept_tapered_length(surface, distance, options);
            } else {
                let to_boundary = matches!(
                    stage,
                    Stage::DirectionStart { to_boundary: true }
                        | Stage::DirectionEnd {
                            to_boundary: true,
                            ..
                        }
                );
                self.set_tapered_stage(surface, Stage::after_direction(to_boundary), options);
            }
        } else if stage == Stage::Angle || stage == Stage::Corners {
            let key = if stage == Stage::Angle {
                "DraftAngle"
            } else {
                "Corners"
            };
            match options.update(&[&format!("{key}={keyword}")]) {
                Ok(()) => self.set_tapered_stage(surface, Stage::Distance, options),
                Err(error) => self.log_tapered_error(surface, error),
            }
        } else if stage == Stage::Distance {
            let next = match keyword.to_ascii_lowercase().as_str() {
                "direction" => Some(Stage::DirectionStart { to_boundary: false }),
                "setbasepoint" => Some(Stage::BasePoint),
                "draftangle" => Some(Stage::Angle),
                "corners" => Some(Stage::Corners),
                _ => None,
            };
            if let Some(next) = next {
                self.set_tapered_stage(surface, next, options);
            } else if let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            ) {
                match length {
                    Ok(distance) => {
                        self.accept_tapered_length(surface, distance, options);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            } else {
                let words = input.split_whitespace().collect::<Vec<_>>();
                if !words
                    .iter()
                    .all(|word| TaperedExtrusionOptions::recognizes(word))
                {
                    return false;
                }
                let updated = if surface {
                    SurfaceTaperedInput::parse_with_options(&words, options).and_then(|input| {
                        if input.faces != SurfaceFaces::Single {
                            return Err(viboceros_command::CommandError::Usage(
                                "Select source faces before choosing a distance",
                            ));
                        }
                        Ok((input.options, input.boundary))
                    })
                } else {
                    TaperedExtrusionOptions::parse_command_options(&words, options)
                };
                match updated {
                    Ok((updated, boundary)) => {
                        if options.direction != updated.direction {
                            self.tapered_extrusion_session.as_mut().unwrap().hover = None;
                        }
                        self.open_tapered_boundary_or_distance(surface, boundary, updated);
                    }
                    Err(error) => self.log_tapered_error(surface, error),
                }
            }
        } else if input == "0" || input.eq_ignore_ascii_case("w0") {
            let point = if input == "0" {
                self.viewports[self.active_viewport]
                    .construction_plane()
                    .origin()
            } else {
                Point3::try_new(0., 0., 0.).unwrap()
            };
            self.accept_drafting_point(point);
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }

    fn log_tapered_error(&mut self, surface: bool, error: viboceros_command::CommandError) {
        let error = match error {
            viboceros_command::CommandError::Usage(_) if surface => {
                viboceros_command::CommandError::Usage(
                    viboceros_command::extrude_surface_tapered::USAGE,
                )
            }
            error => error,
        };
        self.push_log(format!("Error: {error}"));
    }

    pub(super) fn tapered_distance_constraint(&self) -> Option<DestinationConstraint> {
        if self.plane_prompt.is_some()
            || self.nested_measurement.is_some()
            || self.set_view_prompt.is_some()
        {
            return None;
        }
        let Some(InteractiveCommand::ExtrudeTapered {
            stage: Stage::Distance,
            options,
            ..
        }) = self.active_command
        else {
            return None;
        };
        let session = self.tapered_extrusion_session.as_ref()?;
        session
            .sources
            .is_current(&self.document)
            .then(|| session.constraint(options))
    }

    pub(super) fn accept_tapered_extrusion_point(
        &mut self,
        surface: bool,
        stage: Stage,
        mut options: TaperedExtrusionOptions,
        point: Point3,
    ) -> bool {
        match stage {
            Stage::Distance => {
                let Some(height) = self
                    .tapered_distance_constraint()
                    .and_then(|constraint| signed_height(constraint, point))
                else {
                    self.push_log(
                        "Error: extrusion source changed; select the objects again".into(),
                    );
                    return false;
                };
                return self.finish_tapered_extrusion(surface, height, options);
            }
            Stage::BasePoint => {
                let session = self.tapered_extrusion_session.as_mut().unwrap();
                session.base = Some(point);
                session.hover = None;
            }
            Stage::DirectionStart { to_boundary } => {
                self.set_tapered_stage(
                    surface,
                    Stage::DirectionEnd {
                        start: point,
                        to_boundary,
                    },
                    options,
                );
                return true;
            }
            Stage::DirectionEnd { start, to_boundary } => {
                if start.is_near(point, self.document.tolerance()) {
                    self.push_log("Error: extrusion direction points must differ".into());
                    return false;
                }
                let Ok(axis) = start.direction_to(point) else {
                    return false;
                };
                options.direction = Some(axis.as_vector());
                self.tapered_extrusion_session.as_mut().unwrap().hover = None;
                self.set_tapered_stage(surface, Stage::after_direction(to_boundary), options);
                return true;
            }
            Stage::Angle | Stage::Corners | Stage::Boundary => return false,
        }
        self.set_tapered_stage(surface, Stage::Distance, options);
        true
    }

    pub(super) fn update_tapered_hover(&mut self, point: Option<Point3>) -> Option<f64> {
        let height = signed_height(self.tapered_distance_constraint()?, point?)?;
        if height.abs() <= self.document.tolerance().absolute() {
            return None;
        }
        self.tapered_extrusion_session.as_mut()?.hover = Some(height);
        Some(height)
    }

    fn accept_tapered_length(
        &mut self,
        surface: bool,
        distance: f64,
        options: TaperedExtrusionOptions,
    ) -> bool {
        let sign = self
            .tapered_extrusion_session
            .as_ref()
            .and_then(|session| session.hover)
            .map_or(1., f64::signum);
        let committed = self.finish_tapered_extrusion(surface, distance * sign, options);
        if committed {
            self.commands
                .remember_tapered_extrusion_distance(surface, distance);
        }
        committed
    }

    fn finish_tapered_extrusion(
        &mut self,
        surface: bool,
        height: f64,
        options: TaperedExtrusionOptions,
    ) -> bool {
        self.finish_tapered_input(surface, &height.to_string(), options)
    }

    fn finish_tapered_input(
        &mut self,
        surface: bool,
        target: &str,
        options: TaperedExtrusionOptions,
    ) -> bool {
        let Some(session) = self.tapered_extrusion_session.take() else {
            return false;
        };
        if !session.sources.is_current(&self.document) {
            self.tapered_extrusion_session = Some(session);
            self.push_log("Error: extrusion source changed; select the objects again".into());
            return false;
        }
        let plane = session.plane;
        let command = if surface {
            "ExtrudeSrfTapered"
        } else {
            "ExtrudeCrvTapered"
        };
        let command = format!("{command} {target} {}", options.command_options(surface));
        if surface {
            let success = self.finish_surface_face_command_with_plane(&command, Some(plane));
            if !success {
                self.tapered_extrusion_session = Some(session);
            }
            return success;
        }
        let saved = self.active_command;
        let saved_plane = self.drafting_plane;
        let saved_constraint = self.point_constraint.take();
        let saved_filter = self.point_filter.take();
        if self.try_execute_command_with_plane(&command, Some(plane)) {
            self.active_command = None;
            self.drafting_plane = None;
            true
        } else {
            self.active_command = saved;
            self.tapered_extrusion_session = Some(session);
            self.drafting_plane = saved_plane;
            self.point_constraint = saved_constraint;
            self.point_filter = saved_filter;
            false
        }
    }
}

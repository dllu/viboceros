//! Distribution selection and point getters share the accepted layout with the preview.
use super::*;
use viboceros_command::component_selection::{ComponentPick, ComponentSelectionKind};
use viboceros_command::distribute::{self as solver, Direction, Layout, Sources};
use viboceros_command::{CommandError, DistributionSettings};

mod input;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Getter {
    Menu,
    First,
    Second { start: Point3, plane: Frame3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Direction(Getter),
    Mode(Getter),
    Spacing {
        resume: Getter,
        first: Option<Point3>,
    },
}

struct Preview {
    direction: Direction,
    layout: Result<Layout, CommandError>,
}

pub(super) struct Session {
    stage: Stage,
    settings: DistributionSettings,
    sources: Sources,
    components: Vec<ComponentPick>,
    postselected: bool,
    preview: Option<Preview>,
}

impl Session {
    pub(super) fn wants_point(&self) -> bool {
        matches!(
            self.stage,
            Stage::Direction(Getter::First | Getter::Second { .. }) | Stage::Spacing { .. }
        )
    }

    pub(super) fn anchor(&self) -> Option<Point3> {
        match self.stage {
            Stage::Direction(Getter::Second { start, .. }) => Some(start),
            Stage::Spacing { first, .. } => first,
            _ => None,
        }
    }

    pub(super) fn hint(&self) -> &'static str {
        match self.stage {
            Stage::Direction(Getter::Menu) => {
                "Choose Direction, XAxis, YAxis, ZAxis, Mode, or Spacing."
            }
            Stage::Direction(Getter::First) => {
                "Pick the first direction point or edit Mode and Spacing."
            }
            Stage::Direction(Getter::Second { .. }) => {
                "Pick the second direction point or edit Mode and Spacing."
            }
            Stage::Spacing { first: None, .. } => {
                "Enter spacing, choose Automatic, or pick the first distance point."
            }
            Stage::Spacing { first: Some(_), .. } => "Pick the second distance point.",
            Stage::Mode(_) => "Choose Gap or Center.",
        }
    }

    fn prompt(&self) -> String {
        let spacing = self
            .settings
            .spacing
            .map_or_else(|| "Automatic".into(), |value| value.to_string());
        let settings = || format!("Mode={:?} Spacing={spacing}", self.settings.mode);
        match self.stage {
            Stage::Direction(Getter::Menu) => {
                format!(
                    "Distribution options (Direction XAxis YAxis ZAxis {})",
                    settings()
                )
            }
            Stage::Direction(Getter::First) => format!("First direction point ({})", settings()),
            Stage::Direction(Getter::Second { .. }) => {
                format!("Second direction point ({})", settings())
            }
            Stage::Spacing { first, .. } => {
                let prompt = if first.is_some() {
                    "Second distance point".to_owned()
                } else {
                    format!("{:?} distance", self.settings.mode)
                };
                format!("{prompt} <{spacing}> (Automatic)")
            }
            Stage::Mode(_) => "Mode (Gap Center)".into(),
        }
    }

    pub(super) fn layout_preview(&self) -> Option<&Layout> {
        if !matches!(self.stage, Stage::Direction(Getter::Second { .. })) {
            return None;
        }
        self.preview.as_ref()?.layout.as_ref().ok()
    }

    fn layout(&self, direction: Direction, plane: Frame3) -> Result<Layout, CommandError> {
        self.sources.layout(direction, plane, self.settings)
    }
}

impl VibocerosApp {
    fn distribution_components(&self) -> Vec<ComponentPick> {
        self.commands
            .component_selection()
            .live_picks(&self.document)
            .into_iter()
            .filter(|p| {
                matches!(
                    p.kind,
                    ComponentSelectionKind::MeshVertex | ComponentSelectionKind::SubDVertex
                )
            })
            .collect()
    }

    pub(super) fn distribute_unit_count(&self) -> usize {
        Sources::capture(&self.document, &self.distribution_components()).unit_count()
    }

    pub(super) fn selecting_distribution_sources(&self) -> bool {
        self.plane_prompt.is_none()
            && self.object_prompt.as_ref().is_some_and(|p| {
                p.description.command == "Distribute"
                    && p.phase == object_selection::ObjectPromptPhase::Selecting
            })
    }

    pub(super) fn start_distribute(&mut self, input: &str, postselected: bool) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Distribute")
        }) {
            return false;
        }
        let args = words.collect::<Vec<_>>();
        let components = self.distribution_components();
        if let Ok((direction, settings)) = solver::parse_positional(&args) {
            if components.is_empty() {
                return false;
            }
            // Explicit complete input can also use active mesh or SubD vertex picks.
            return self.run_distribute_with_components(direction, settings, components);
        }
        let (getter, args) = if args.first().is_some_and(|word| {
            word.trim_start_matches('_')
                .eq_ignore_ascii_case("Direction")
        }) {
            (Getter::First, &args[1..])
        } else {
            (Getter::Menu, args.as_slice())
        };
        let Ok(settings) = DistributionSettings::parse_over(args, self.distribute_settings) else {
            return false;
        };
        let sources = Sources::capture(&self.document, &components);
        if sources.unit_count() < 3 {
            let raw_count = self.document.selected_object_count()
                + self.document.selected_control_points().count()
                + components.len();
            if raw_count >= 3 && !postselected {
                self.push_log(format!(
                    "Error: {}",
                    CommandError::InsufficientDistributionObjects {
                        actual: sources.unit_count(),
                    }
                ));
            } else {
                self.cancel_interactive_command(false);
                self.start_transform_source_prompt("Distribute");
                self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            }
            return true;
        }
        self.cancel_interactive_command(false);
        self.distribute_session = Some(Session {
            stage: Stage::Direction(getter),
            settings,
            sources,
            components,
            postselected,
            preview: None,
        });
        self.active_command = Some(InteractiveCommand::Distribute);
        self.log_distribute();
        true
    }

    fn run_distribute_with_components(
        &mut self,
        direction: Direction,
        settings: DistributionSettings,
        components: Vec<ComponentPick>,
    ) -> bool {
        let plane = self.viewports[self.active_viewport].construction_plane();
        self.cancel_interactive_command(false);
        let result = Sources::capture(&self.document, &components)
            .layout(direction, plane, settings)
            .and_then(|layout| layout.commit(&mut self.document, false));
        match result {
            Ok(message) => {
                self.distribute_settings = settings;
                self.commands.component_selection().consume(&components);
                self.push_log(message);
            }
            Err(e) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {e}"));
            }
        }
        true
    }

    pub(super) fn remember_distribute_positional(&mut self, input: &str) {
        let mut words = input.split_whitespace();
        if words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Distribute")
        }) && let Ok((_, settings)) = solver::parse_positional(&words.collect::<Vec<_>>())
        {
            self.distribute_settings = settings;
        }
    }

    fn log_distribute(&mut self) {
        if let Some(session) = &self.distribute_session {
            self.push_log(session.prompt());
        }
    }

    pub(super) fn validate_distribute(&mut self) {
        if self.distribute_session.as_ref().is_some_and(|session| {
            !session.sources.is_current(&self.document)
                || session.components != self.distribution_components()
        }) {
            self.cancel_interactive_command(false);
            self.push_log(format!("Error: {}", CommandError::DistributeStale));
        }
    }

    pub(super) fn update_distribute_preview(&mut self, cursor: Option<Point3>) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(session) = &mut self.distribute_session else {
            return false;
        };
        let Stage::Direction(Getter::Second { start, plane }) = session.stage else {
            return false;
        };
        let Some(cursor) = cursor else { return false };
        let direction = Direction::Points(start, cursor);
        if session
            .preview
            .as_ref()
            .is_some_and(|preview| preview.direction == direction)
        {
            return false;
        }
        session.preview = Some(Preview {
            direction,
            layout: session.layout(direction, plane),
        });
        true
    }

    pub(super) fn accept_distribute_point(&mut self, point: Point3) -> Option<bool> {
        let session = self.distribute_session.as_mut()?;
        let plane = self.viewports[self.active_viewport].construction_plane();
        match session.stage {
            Stage::Direction(Getter::First) => {
                session.stage = Stage::Direction(Getter::Second {
                    start: point,
                    plane,
                });
                self.log_distribute();
                Some(true)
            }
            Stage::Direction(Getter::Second { start, plane }) => {
                Some(self.commit_distribute(Direction::Points(start, point), plane))
            }
            Stage::Spacing {
                resume,
                first: None,
            } => {
                session.stage = Stage::Spacing {
                    resume,
                    first: Some(point),
                };
                self.log_distribute();
                Some(true)
            }
            Stage::Spacing {
                resume,
                first: Some(first),
            } => match first.distance_to(point) {
                Ok(distance) => {
                    self.set_distribute_spacing(Some(distance), resume);
                    Some(true)
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    Some(false)
                }
            },
            _ => Some(false),
        }
    }

    fn commit_distribute(&mut self, direction: Direction, plane: Frame3) -> bool {
        self.validate_distribute();
        let Some(session) = &mut self.distribute_session else {
            return false;
        };
        let layout = match session.preview.take() {
            Some(preview) if preview.direction == direction => preview.layout,
            _ => session.layout(direction, plane),
        };
        let postselected = session.postselected;
        match layout.and_then(|layout| layout.commit(&mut self.document, postselected)) {
            Ok(message) => {
                let session = self.distribute_session.take().unwrap();
                self.distribute_settings = session.settings;
                self.commands
                    .component_selection()
                    .consume(&session.components);
                self.cancel_interactive_command(false);
                self.push_log(message);
                true
            }
            Err(e) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }

    fn set_distribute_spacing(&mut self, spacing: Option<f64>, resume: Getter) {
        let session = self.distribute_session.as_mut().unwrap();
        session.settings.spacing = spacing;
        session.stage = Stage::Direction(resume);
        session.preview = None;
        self.log_distribute();
    }
}

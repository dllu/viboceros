//! Point, size and reference getters for solid and mesh spheres.
use super::*;
use viboceros_command::{
    CommandError,
    circle_input::CircleDefinition,
    mesh_sphere::{self, Topology, TopologyOption},
    sphere_command::{commit, size_argument},
    sphere_input::{Preferences, SphereInput},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum Output {
    Solid,
    Mesh {
        topology: Topology,
        pending: Option<TopologyOption>,
    },
}

impl Output {
    pub(in crate::app) const fn name(self) -> &'static str {
        match self {
            Self::Solid => "Sphere",
            Self::Mesh { .. } => "MeshSphere",
        }
    }

    pub(super) fn topology(self) -> Option<Topology> {
        match self {
            Self::Solid => None,
            Self::Mesh { topology, .. } => Some(topology),
        }
    }

    pub(super) fn size_preferences(self, commands: &CommandRegistry) -> Preferences {
        match self {
            Self::Solid => commands.sphere_preferences(),
            Self::Mesh { .. } => Preferences::default(),
        }
    }

    fn remember_size(self, commands: &CommandRegistry, preferences: Preferences) {
        if self == Self::Solid {
            commands.remember_sphere_preferences(preferences);
        }
    }

    fn edit_topology(&mut self, text: &str, at_start: bool) -> Option<Result<(), CommandError>> {
        let Self::Mesh { topology, pending } = self else {
            return None;
        };
        if text.eq_ignore_ascii_case("Cancel") {
            return None;
        }
        let (option, value) = if let Some(option) = *pending {
            let value = text.trim().trim_start_matches('_');
            if value.is_empty() || value.eq_ignore_ascii_case("Enter") {
                *pending = None;
                return Some(Ok(()));
            }
            (option, Some(value))
        } else {
            TopologyOption::parse(text)?
        };
        if !at_start || !option.available(*topology) {
            return Some(Err(CommandError::Usage(
                "Choose topology settings offered at the MeshSphere center prompt",
            )));
        }
        if let Some(value) = value {
            let mut candidate = *topology;
            if let Err(error) = option
                .assign(&mut candidate, value)
                .and_then(|()| candidate.validate())
            {
                return Some(Err(error));
            }
            *topology = candidate;
            *pending = None;
        } else {
            *pending = Some(option);
        }
        Some(Ok(()))
    }

    fn pending_prompt(self) -> Option<String> {
        match self {
            Self::Mesh {
                topology,
                pending: Some(option),
            } => Some(option.prompt(topology)),
            _ => None,
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_start_sphere(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        let Some(name) = words.next() else {
            return false;
        };
        let name = name.trim_start_matches(['_', '-']);
        let plane = self.viewports[self.active_viewport].construction_plane();
        let (mut input, output, option) = if name.eq_ignore_ascii_case("MeshSphere") {
            let words = words.collect::<Vec<_>>();
            let initial = self.commands.mesh_sphere_preferences();
            let parsed = mesh_sphere::parse_topology(&words, initial);
            let (topology, definition, pending) = if let Ok((topology, definition)) = parsed {
                (topology, definition, None)
            } else if let Some((last, prefix)) = words.split_last()
                && let Some((option, None)) = TopologyOption::parse(last)
                && let Ok((topology, definition)) = mesh_sphere::parse_topology(prefix, initial)
                && definition.is_empty()
                && option.available(topology)
            {
                (topology, definition, Some(option))
            } else {
                return false;
            };
            let option = match definition.as_slice() {
                [] => None,
                [option] => Some(*option),
                _ => return false,
            };
            (
                SphereInput::new(plane),
                Output::Mesh { topology, pending },
                option,
            )
        } else if name.eq_ignore_ascii_case("Sphere") || name.eq_ignore_ascii_case("Sph") {
            let option = words.next();
            if words.next().is_some() {
                return false;
            }
            (
                SphereInput::with_preferences(plane, self.commands.sphere_preferences()),
                Output::Solid,
                option,
            )
        } else {
            return false;
        };
        if let Some(option) = option {
            if self.start_sphere_guide(option, output) {
                return true;
            }
            // Inline FitPoints uses the registry's ordinary selection prompt.
            if !input.option(option) {
                return false;
            }
        }
        self.cancel_interactive_command(false);
        self.push_log(format!("> {text}"));
        self.sphere_prompt(input, output);
        true
    }

    fn start_sphere_guide(&mut self, option: &str, output: Output) -> bool {
        let option = option.trim_start_matches(['_', '-']);
        if option.eq_ignore_ascii_case("AroundCurve") {
            self.start_sphere_around(output);
        } else if option.eq_ignore_ascii_case("Tangent") {
            self.start_tangent_sphere(output);
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }

    fn sphere_prompt(&mut self, input: SphereInput, output: Output) {
        self.active_command = Some(InteractiveCommand::Sphere { input, output });
        self.drafting_plane = input
            .mouse_plane()
            .or_else(|| input.anchor().map(|_| input.construction_plane()));
        self.command_input.clear();
        let prompt = output.pending_prompt().unwrap_or_else(|| {
            let mut prompt = input
                .prompt()
                .replace("reference sphere", "sphere")
                .replace("Reference sphere", "Sphere");
            if let Some(size) = input.remembered_size(output.size_preferences(&self.commands)) {
                prompt.push_str(&format!(" <{size}>"));
            }
            if let Some(topology) = output.topology() {
                prompt = prompt.replace("remembered ", "default ");
                if input.at_start() {
                    prompt.push_str("; ");
                    prompt.push_str(&topology.arguments());
                }
            }
            prompt
        });
        self.push_log(format!("{}: {prompt}", output.name()));
    }

    pub(super) fn try_continue_sphere(&mut self, text: &str) -> bool {
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let Some(InteractiveCommand::Sphere {
            mut input,
            mut output,
        }) = self.active_command
        else {
            return false;
        };
        let word = text.trim().trim_start_matches(['_', '-']);
        if let Some(result) = output.edit_topology(word, input.at_start()) {
            match result {
                Ok(()) => self.sphere_prompt(input, output),
                Err(error) => self.sphere_input_error(error),
            }
            return true;
        }
        if input.at_start() && self.start_sphere_guide(word, output) {
            return true;
        }
        if input.at_start() && word.eq_ignore_ascii_case("FitPoints") {
            let command = output.topology().map_or_else(
                || "Sphere FitPoints".into(),
                |topology| format!("MeshSphere {} FitPoints", topology.arguments()),
            );
            self.cancel_interactive_command(false);
            if !self.try_start_object_prompt(&command) {
                self.execute_command(&command);
            }
            return true;
        }
        let mut preferences = output.size_preferences(&self.commands);
        if input.option(text) {
            input.update_size_mode(&mut preferences);
            output.remember_size(&self.commands, preferences);
            self.point_constraint = None;
            self.point_filter = None;
            self.sphere_prompt(input, output);
            return true;
        }
        let scalar = text
            .split_once('=')
            .or_else(|| text.split_once(char::is_whitespace))
            .filter(|(name, _)| input.option(name.trim()))
            .map(|(_, value)| value.trim());
        if !input.requests_size() {
            return false;
        }
        if scalar.is_some() {
            input.update_size_mode(&mut preferences);
        }
        match size_argument(
            input,
            scalar.unwrap_or(text),
            self.document.units(),
            preferences,
        ) {
            Ok(Some(size)) => {
                let result = input.number(size);
                self.advance_sphere(input, output, preferences, result);
                true
            }
            Ok(None) if scalar.is_none() => false,
            Ok(None) => {
                self.sphere_input_error("expected a sphere size");
                true
            }
            Err(e) => {
                self.sphere_input_error(e);
                true
            }
        }
    }

    pub(super) fn accept_sphere_point(
        &mut self,
        mut input: SphereInput,
        output: Output,
        point: Point3,
    ) -> bool {
        if output.pending_prompt().is_some() {
            return false;
        }
        input.set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = input.point(point);
        if input.requests_size()
            && let Err(error) = result
        {
            self.sphere_input_error(format_args!(
                "invalid sphere radius or orientation ({error})"
            ));
            return false;
        }
        self.advance_sphere(
            input,
            output,
            output.size_preferences(&self.commands),
            result,
        )
    }

    fn advance_sphere(
        &mut self,
        input: SphereInput,
        output: Output,
        mut preferences: Preferences,
        result: Result<Option<CircleDefinition>, viboceros_geometry::GeometryError>,
    ) -> bool {
        match result {
            Ok(Some(definition)) => {
                input.update_radius(&mut preferences, definition.radius);
                self.finish_sphere(output, definition, preferences, None)
            }
            Ok(None) => {
                output.remember_size(&self.commands, preferences);
                self.point_constraint = None;
                self.point_filter = None;
                self.sphere_prompt(input, output);
                true
            }
            Err(e) => {
                self.sphere_input_error(e);
                false
            }
        }
    }

    pub(super) fn finish_sphere(
        &mut self,
        output: Output,
        definition: CircleDefinition,
        preferences: Preferences,
        release: Option<ObjectId>,
    ) -> bool {
        let result = match output {
            Output::Solid => commit(&mut self.document, definition, release),
            Output::Mesh { topology, .. } => {
                mesh_sphere::commit(&mut self.document, topology, definition, release)
            }
        };
        match result {
            Ok(message) => {
                self.cancel_interactive_command(false);
                output.remember_size(&self.commands, preferences);
                if let Some(topology) = output.topology() {
                    self.commands.remember_mesh_sphere_preferences(topology);
                }
                self.push_log(message);
                true
            }
            Err(e) => {
                self.sphere_input_error(e);
                false
            }
        }
    }

    fn sphere_input_error(&mut self, error: impl std::fmt::Display) {
        self.command_file.fail_dispatch();
        self.push_log(format!("Error: {error}"));
    }

    pub(super) fn sphere_preview_input(&self) -> Option<SphereInput> {
        if self.plane_prompt.is_none()
            && let Some(InteractiveCommand::Sphere { input, output }) = self.active_command
            && output.pending_prompt().is_none()
        {
            Some(input)
        } else {
            None
        }
    }

    pub(super) fn mesh_sphere_fit_preview(&self) -> Option<(Topology, CircleDefinition)> {
        let pending = self.object_prompt.as_ref()?;
        if pending.description.command != mesh_sphere::FIT_COMMAND {
            return None;
        }
        let source = viboceros_command::point_fitting::PointFitKind::Sphere;
        if source.selected_point_count(&self.document) < source.minimum_points() {
            return None;
        }
        let words = pending
            .command_override
            .as_deref()
            .unwrap_or(mesh_sphere::FIT_COMMAND)
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>();
        let (topology, _) =
            mesh_sphere::parse_topology(&words, self.commands.mesh_sphere_preferences()).ok()?;
        let definition = SphereInput::fit_optional(&source.selected_points(&self.document))
            .ok()
            .flatten()?;
        Some((topology, definition))
    }

    pub(super) fn mesh_sphere_preview_topology(&self) -> Option<Topology> {
        match self.active_command {
            Some(
                InteractiveCommand::Sphere { output, .. }
                | InteractiveCommand::SphereGuide { output, .. },
            ) => output.topology(),
            _ => None,
        }
    }
}

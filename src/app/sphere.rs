//! Point, size and reference getters for the solid Sphere command.
use super::*;
use viboceros_command::{
    circle_input::CircleDefinition,
    sphere_command::{commit, size_argument},
    sphere_input::{Preferences, SphereInput},
};

impl VibocerosApp {
    pub(super) fn try_start_sphere(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        let Some(name) = words.next() else {
            return false;
        };
        let name = name.trim_start_matches(['_', '-']);
        if !name.eq_ignore_ascii_case("Sphere") && !name.eq_ignore_ascii_case("Sph") {
            return false;
        }
        let mut input = SphereInput::with_preferences(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.sphere_preferences(),
        );
        if let Some(option) = words.next() {
            if words.next().is_some() {
                return false;
            }
            if self.start_sphere_guide(option) {
                return true;
            }
            if !input.option(option) {
                return false;
            }
        }
        self.cancel_interactive_command(false);
        self.push_log(format!("> {text}"));
        self.sphere_prompt(input);
        true
    }

    fn start_sphere_guide(&mut self, option: &str) -> bool {
        let option = option.trim_start_matches(['_', '-']);
        if option.eq_ignore_ascii_case("AroundCurve") {
            self.start_sphere_around();
        } else if option.eq_ignore_ascii_case("Tangent") {
            self.start_tangent_sphere();
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }

    fn sphere_prompt(&mut self, input: SphereInput) {
        self.active_command = Some(InteractiveCommand::Sphere { input });
        if let Some(plane) = input.mouse_plane() {
            self.drafting_plane = Some(plane);
        }
        self.command_input.clear();
        let prompt = input
            .prompt()
            .replace("reference sphere", "sphere")
            .replace("Reference sphere", "Sphere");
        let default = input
            .remembered_size(self.commands.sphere_preferences())
            .map(|size| format!(" <{size}>"))
            .unwrap_or_default();
        self.push_log(format!("Sphere: {prompt}{default}"));
    }

    pub(super) fn try_continue_sphere(&mut self, text: &str) -> bool {
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let Some(InteractiveCommand::Sphere { mut input }) = self.active_command else {
            return false;
        };
        let word = text.trim().trim_start_matches(['_', '-']);
        if input.at_start() && self.start_sphere_guide(word) {
            return true;
        }
        if input.at_start() && word.eq_ignore_ascii_case("FitPoints") {
            self.cancel_interactive_command(false);
            if !self.try_start_object_prompt("Sphere FitPoints") {
                self.execute_command("Sphere FitPoints");
            }
            return true;
        }
        let mut preferences = self.commands.sphere_preferences();
        if input.option(text) {
            input.update_size_mode(&mut preferences);
            self.commands.remember_sphere_preferences(preferences);
            self.point_constraint = None;
            self.point_filter = None;
            self.sphere_prompt(input);
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
                self.advance_sphere(input, preferences, result);
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

    pub(super) fn accept_sphere_point(&mut self, mut input: SphereInput, point: Point3) -> bool {
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
        self.advance_sphere(input, self.commands.sphere_preferences(), result)
    }

    fn advance_sphere(
        &mut self,
        input: SphereInput,
        mut preferences: Preferences,
        result: Result<Option<CircleDefinition>, viboceros_geometry::GeometryError>,
    ) -> bool {
        match result {
            Ok(Some(definition)) => {
                input.update_radius(&mut preferences, definition.radius);
                self.finish_sphere(definition, preferences, None)
            }
            Ok(None) => {
                self.commands.remember_sphere_preferences(preferences);
                self.point_constraint = None;
                self.point_filter = None;
                self.sphere_prompt(input);
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
        definition: CircleDefinition,
        preferences: Preferences,
        release: Option<ObjectId>,
    ) -> bool {
        match commit(&mut self.document, definition, release) {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.commands.remember_sphere_preferences(preferences);
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
            && let Some(InteractiveCommand::Sphere { input }) = self.active_command
        {
            Some(input)
        } else {
            None
        }
    }
}

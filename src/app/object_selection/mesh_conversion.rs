//! Mesh selection and numeric options are staged until explicit acceptance.
use super::*;
use viboceros_command::{quadrangulate_mesh as quad, triangulate_nonplanar_quads as nonplanar};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::Tolerance;
#[cfg(test)]
#[path = "mesh_conversion/tests.rs"]
mod nonplanar_tests;
#[cfg(test)]
#[path = "quadrangulate_mesh/tests.rs"]
mod quadrangulate_tests;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Settings {
    Quadrangulate(quad::Options),
    Nonplanar(nonplanar::Options),
}
impl Settings {
    fn name(self) -> &'static str {
        match self {
            Self::Quadrangulate(_) => "QuadrangulateMesh",
            Self::Nonplanar(_) => "TriangulateNonPlanarQuads",
        }
    }
    fn command(self) -> String {
        match self {
            Self::Quadrangulate(o) => o.command(),
            Self::Nonplanar(o) => o.command(),
        }
    }
    fn initial(
        name: &str,
        registry: &viboceros_command::CommandRegistry,
        args: &[&str],
    ) -> Result<Self, viboceros_command::CommandError> {
        if name == "QuadrangulateMesh" {
            registry
                .quadrangulate_mesh_options(args)
                .map(Self::Quadrangulate)
        } else {
            registry
                .triangulate_nonplanar_quads_options(args)
                .map(Self::Nonplanar)
        }
    }
    fn parse(self, args: &[&str]) -> Result<Self, viboceros_command::CommandError> {
        match self {
            Self::Quadrangulate(o) => quad::parse(args, o).map(Self::Quadrangulate),
            Self::Nonplanar(o) => nonplanar::parse(args, o).map(Self::Nonplanar),
        }
    }
    fn option(self, name: &str) -> Option<&'static str> {
        let names: &[&str] = match self {
            Self::Quadrangulate(_) => &["Planarity", "Rectangularity"],
            Self::Nonplanar(_) => &["SelectBy", "Distance", "Angle", "SplitMethod"],
        };
        let name = if name.eq_ignore_ascii_case("Mode") {
            "SelectBy"
        } else if name.eq_ignore_ascii_case("Split") {
            "SplitMethod"
        } else {
            name
        };
        names.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    options: Settings,
    value: Option<&'static str>,
    parents: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: Tolerance,
}
impl Session {
    pub(super) fn label(&self) -> &'static str {
        self.value.unwrap_or(self.options.name())
    }
    pub(super) fn hint(&self, phase: ObjectPromptPhase) -> &'static str {
        match self.value {
            Some("Planarity" | "Angle") => {
                "Enter an angle from 0 to 180 degrees; Enter keeps the current value"
            }
            Some("Rectangularity") => {
                "Enter a rectangularity ratio of at least 1; Enter keeps the current value"
            }
            Some("Distance") => "Enter a nonnegative distance; Enter keeps the current value",
            Some("SelectBy") => "Choose Distance, Angle or Both; Enter keeps the current value",
            Some(_) => {
                "Choose ShortestDiagonal, LongestDiagonal, MinimumArea, MaximumArea, MinimumAngle or MaximumAngle"
            }
            None if phase == ObjectPromptPhase::Selecting => {
                "Select meshes; Enter opens conversion options; Esc cancels"
            }
            None => "Change conversion options; Enter applies, Esc cancels",
        }
    }
    fn pin(&mut self, doc: &Document) {
        self.parents = doc
            .selected_objects()
            .filter(|o| matches!(o.geometry(), Geometry::Mesh(_)))
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect();
        self.tolerance = doc.tolerance();
    }
    fn valid(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.parents.iter().all(|(id, source)| {
                doc.is_selected(*id)
                    && doc.is_object_selectable(*id)
                    && doc
                        .object(*id)
                        .is_some_and(|o| source.shares_storage_with(o.geometry_snapshot()))
            })
            && doc
                .selected_objects()
                .filter(|o| matches!(o.geometry(), Geometry::Mesh(_)))
                .count()
                == self.parents.len()
    }
}
impl VibocerosApp {
    pub(super) fn initialize_mesh_conversion(&mut self, command: &str) {
        let Some(p) = self.object_prompt.as_mut().filter(|p| {
            matches!(
                p.description.command,
                "QuadrangulateMesh" | "TriangulateNonPlanarQuads"
            )
        }) else {
            return;
        };
        let words = command.split_whitespace().skip(1).collect::<Vec<_>>();
        let Ok(options) = Settings::initial(p.description.command, &self.commands, &words) else {
            return;
        };
        let mut session = Session {
            options,
            value: None,
            parents: vec![],
            tolerance: self.document.tolerance(),
        };
        if p.phase == ObjectPromptPhase::Options {
            session.pin(&self.document);
        }
        p.command_override = Some(options.command());
        p.mesh_conversion = Some(session);
    }
    pub(super) fn continue_mesh_conversion(&mut self, input: &str) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.mesh_conversion.is_some())
        else {
            return false;
        };
        let first = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        if first.eq_ignore_ascii_case("Cancel")
            || (!input.is_empty()
                && self.commands.recognizes(first)
                && pending
                    .mesh_conversion
                    .as_ref()
                    .unwrap()
                    .options
                    .option(first)
                    .is_none())
        {
            self.cancel_object_prompt(true);
            return first.eq_ignore_ascii_case("Cancel");
        }
        let mut session = pending.mesh_conversion.clone().unwrap();
        let phase = pending.phase;
        if input.is_empty() && session.value.is_none() {
            if phase == ObjectPromptPhase::Selecting {
                session.pin(&self.document);
                if session.parents.is_empty() {
                    self.push_log("Select at least one mesh".into());
                } else {
                    self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Options;
                }
            } else {
                if !session.valid(&self.document) {
                    self.cancel_object_prompt(false);
                    self.push_log(format!(
                        "{} inputs changed; select meshes again",
                        session.options.name()
                    ));
                    return true;
                }
                let command = session.options.command();
                match self.commands.execute(&mut self.document, &command) {
                    Ok(message) => {
                        self.object_prompt = None;
                        self.push_log(message);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                return true;
            }
        } else if phase == ObjectPromptPhase::Options
            && session.value.is_none()
            && session.options.option(first).is_some()
            && input.split_whitespace().count() == 1
        {
            if matches!(session.options, Settings::Nonplanar(_))
                && first.eq_ignore_ascii_case("SelectBy")
            {
                session.options = session
                    .options
                    .parse(&["SelectBy"])
                    .expect("bare SelectBy toggles the active criterion");
            } else {
                session.value = session.options.option(first);
            }
        } else {
            let text = if let Some(name) = session.value {
                if input.is_empty() {
                    String::new()
                } else {
                    format!("{name}={input}")
                }
            } else {
                input.to_owned()
            };
            let words = text.split_whitespace().collect::<Vec<_>>();
            match session.options.parse(&words) {
                Ok(options) => {
                    session.options = options;
                    session.value = None;
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.command_input.clear();
                    return true;
                }
            }
        }
        let p = self.object_prompt.as_mut().unwrap();
        p.command_override = Some(session.options.command());
        p.mesh_conversion = Some(session);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
}

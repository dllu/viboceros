//! Mesh selection and numeric options are staged until explicit acceptance.
use super::*;
use viboceros_command::quadrangulate_mesh::{self, Options};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::Tolerance;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    options: Options,
    value: Option<&'static str>,
    parents: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: Tolerance,
}
impl Session {
    pub(super) fn label(&self) -> &'static str {
        self.value.unwrap_or("QuadrangulateMesh")
    }
    pub(super) fn hint(&self, phase: ObjectPromptPhase) -> &'static str {
        match self.value {
            Some("Planarity") => {
                "Enter an angle from 0 to 180 degrees; Enter keeps the current value"
            }
            Some(_) => "Enter a rectangularity ratio of at least 1; Enter keeps the current value",
            None if phase == ObjectPromptPhase::Selecting => {
                "Select meshes; Enter opens Planarity and Rectangularity options; Esc cancels"
            }
            None => "Change Planarity or Rectangularity; Enter quadrangulates, Esc cancels",
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
    pub(super) fn initialize_quadrangulate_mesh(&mut self, command: &str) {
        let Some(p) = self
            .object_prompt
            .as_mut()
            .filter(|p| p.description.command == "QuadrangulateMesh")
        else {
            return;
        };
        let words = command.split_whitespace().skip(1).collect::<Vec<_>>();
        let Ok(options) = self.commands.quadrangulate_mesh_options(&words) else {
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
        p.quadrangulate_mesh = Some(session);
    }
    pub(super) fn continue_quadrangulate_mesh(&mut self, input: &str) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.quadrangulate_mesh.is_some())
        else {
            return false;
        };
        let first = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        if first.eq_ignore_ascii_case("Cancel")
            || (!input.is_empty() && self.commands.recognizes(first))
        {
            self.cancel_object_prompt(true);
            return first.eq_ignore_ascii_case("Cancel");
        }
        let mut session = pending.quadrangulate_mesh.clone().unwrap();
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
                    self.push_log("QuadrangulateMesh inputs changed; select meshes again".into());
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
            && ["Planarity", "Rectangularity"]
                .iter()
                .any(|n| first.eq_ignore_ascii_case(n))
            && input.split_whitespace().count() == 1
        {
            session.value = Some(if first.eq_ignore_ascii_case("Planarity") {
                "Planarity"
            } else {
                "Rectangularity"
            });
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
            match quadrangulate_mesh::parse(&words, session.options) {
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
        p.quadrangulate_mesh = Some(session);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
}

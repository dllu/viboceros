//! One source-selection session with typed face-size and output-layer prompts.
use super::*;
use std::sync::Arc;
use viboceros_command::mesh_from_lines::Options;
use viboceros_document::{GeometrySnapshot, HistoryGroup};
#[cfg(test)]
mod tests;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Sides,
    Layer,
}
#[derive(Clone, Debug)]
struct Stamp(Arc<HistoryGroup>);
impl PartialEq for Stamp {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    options: Options,
    value: Option<Value>,
    parents: Vec<(ObjectId, GeometrySnapshot)>,
    history: Stamp,
    tolerance: viboceros_geometry::Tolerance,
}
impl Session {
    pub(super) fn label(&self) -> &'static str {
        match self.value {
            Some(Value::Sides) => "MaxSidesPerFace",
            Some(Value::Layer) => "OutputLayer",
            None => "MeshFromLines",
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        match self.value {
            Some(Value::Sides) => "Enter an integer of at least three",
            Some(Value::Layer) => "Enter Current, a layer name, or a layer ID",
            None => {
                "Select at least three lines; Enter builds the mesh. MaxSidesPerFace, DeleteInput, OutputLayer"
            }
        }
    }
    fn command(&self) -> String {
        format!(
            "MeshFromLines MaxSidesPerFace={} DeleteInput={} OutputLayer={}",
            self.options.preferences.max_sides,
            if self.options.preferences.delete_input {
                "Yes"
            } else {
                "No"
            },
            self.options
                .output_layer
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "Current".into())
        )
    }
    fn valid(&self, doc: &Document) -> bool {
        doc.history_group_is_current(&self.history.0)
            && self.tolerance == doc.tolerance()
            && self.parents.iter().all(|(id, snapshot)| {
                doc.object(*id)
                    .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            })
    }
}
fn configure(pending: &mut PendingObjectCommand) {
    let session = pending.mesh_from_lines.as_ref().unwrap();
    pending.phase = if session.value.is_none() {
        ObjectPromptPhase::Selecting
    } else {
        ObjectPromptPhase::Options
    };
    pending.command_override = Some(session.command());
    pending.description.options[0].value = session.options.preferences.delete_input;
}
impl VibocerosApp {
    pub(super) fn start_mesh_from_lines_prompt(
        &mut self,
        input: &str,
        description: ObjectSelectionPrompt,
    ) -> bool {
        let words = input.split_whitespace().skip(1).collect::<Vec<_>>();
        let options = match self.commands.mesh_from_lines_options(&words) {
            Ok(o) => o,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let history = match self.document.begin_history_group("MeshFromLines") {
            Ok(h) => h,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let before = self
            .document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let parents = self
            .document
            .selected_objects()
            .filter(|o| description.filter.accepts_object(o))
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect();
        let session = Session {
            options,
            value: None,
            parents,
            history: Stamp(Arc::new(history)),
            tolerance: self.document.tolerance(),
        };
        let mut pending = PendingObjectCommand::selecting(description);
        pending.selection_before = Some(before);
        pending.mesh_from_lines = Some(session);
        configure(&mut pending);
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
    pub(super) fn continue_mesh_from_lines_prompt(&mut self, input: &str) -> bool {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.mesh_from_lines.is_none())
        {
            return false;
        }
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_object_prompt(true);
            return true;
        }
        let layer_value = self
            .object_prompt
            .as_ref()
            .and_then(|p| p.mesh_from_lines.as_ref())
            .is_some_and(|s| s.value == Some(Value::Layer))
            && self
                .document
                .layer_by_name(input.trim_matches('"'))
                .is_some();
        if !layer_value
            && !input.is_empty()
            && input
                .split_whitespace()
                .next()
                .is_some_and(|w| self.commands.recognizes(w))
        {
            self.cancel_object_prompt(true);
            return false;
        }
        let mut pending = self.object_prompt.take().unwrap();
        let mut session = pending.mesh_from_lines.take().unwrap();
        if !session.valid(&self.document) {
            self.push_log(
                "MeshFromLines inputs or document history changed; restart the command".into(),
            );
            self.command_input.clear();
            return true;
        }
        if input.is_empty() && session.value.is_none() {
            let selected = self
                .document
                .selected_objects()
                .filter(|o| pending.description.filter.accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if selected.len() < 3 {
                self.push_log("Select at least three line segments".into());
            } else {
                let command = format!(
                    "{} Sources={}",
                    session.command(),
                    selected
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
                match self.commands.execute(&mut self.document, &command) {
                    Ok(message) => self.push_log(message),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                return true;
            }
        } else if let Some(value) = session.value {
            let option = match value {
                Value::Sides => Some(format!("MaxSidesPerFace={input}")),
                Value::Layer => {
                    if input.eq_ignore_ascii_case("Current") {
                        Some("OutputLayer=Current".into())
                    } else {
                        let id = self
                            .document
                            .layers()
                            .find(|layer| {
                                layer.name() == input.trim_matches('"')
                                    || layer.id().to_string() == input
                            })
                            .map(|layer| layer.id());
                        id.map(|id| format!("OutputLayer={id}"))
                    }
                }
            };
            if let Some(option) = option {
                let full = format!("{} {option}", session.command());
                let mut words = full
                    .split_whitespace()
                    .skip(1)
                    .filter(|s| {
                        !s.starts_with(match value {
                            Value::Sides => "MaxSidesPerFace=",
                            Value::Layer => "OutputLayer=",
                        })
                    })
                    .collect::<Vec<_>>();
                words.push(&option);
                match self.commands.mesh_from_lines_options(&words) {
                    Ok(options) => {
                        session.options = options;
                        session.value = None;
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            } else {
                self.push_log("Unknown output layer".into());
            }
        } else if input.eq_ignore_ascii_case("MaxSidesPerFace") {
            session.value = Some(Value::Sides);
        } else if input.eq_ignore_ascii_case("OutputLayer") {
            session.value = Some(Value::Layer);
        } else {
            let option = if input.eq_ignore_ascii_case("DeleteInput") {
                format!(
                    "DeleteInput={}",
                    if session.options.preferences.delete_input {
                        "No"
                    } else {
                        "Yes"
                    }
                )
            } else {
                input.to_owned()
            };
            let old = session.command();
            let keys = option
                .split_whitespace()
                .filter_map(|s| s.split_once('=').map(|p| p.0))
                .collect::<Vec<_>>();
            let mut words = old
                .split_whitespace()
                .skip(1)
                .filter(|s| {
                    !keys.iter().any(|key| {
                        s.split_once('=')
                            .is_some_and(|p| p.0.eq_ignore_ascii_case(key))
                    })
                })
                .collect::<Vec<_>>();
            words.extend(option.split_whitespace());
            match self.commands.mesh_from_lines_options(&words) {
                Ok(options) => session.options = options,
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.commands
            .accept_mesh_from_lines_options(&session.options);
        pending.mesh_from_lines = Some(session);
        configure(&mut pending);
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
}

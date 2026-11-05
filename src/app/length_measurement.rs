//! Length's parent getter accepts curves and surface edges, with explicit overlaps.
use super::*;
use viboceros_command::chain_selection::Target;
use viboceros_command::component_selection::{ComponentAction, ComponentPick, stage_sources};
use viboceros_document::GeometrySnapshot;
#[derive(Debug)]
pub(super) struct Choices {
    targets: Vec<Target>,
    mode: SelectionMode,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: Tolerance,
}
impl VibocerosApp {
    pub(super) fn has_length_edges(&self) -> bool {
        self.commands
            .component_selection()
            .live_picks(&self.document)
            .iter()
            .any(|p| p.kind == viboceros_command::ComponentSelectionKind::BrepEdge)
    }
    pub(super) fn length_choice_highlights(&self) -> &[Target] {
        self.length_choices
            .as_ref()
            .map_or(&[], |p| p.targets.as_slice())
    }
    pub(super) fn validate_length_choices(&mut self) {
        if self.length_choices.as_ref().is_some_and(|p| {
            !self.length_chain_available()
                || p.tolerance != self.document.tolerance()
                || p.sources.iter().any(|(id, w)| {
                    !self.document.is_object_selectable(*id)
                        || !self
                            .document
                            .object(*id)
                            .is_some_and(|o| o.geometry_snapshot() == w)
                })
        }) {
            self.length_choices = None;
            self.push_log("Length inputs changed; select again".into());
        }
    }
    pub(super) fn pick_length_inputs(&mut self, targets: Vec<Target>, mode: SelectionMode) {
        if !self.length_chain_available() {
            return;
        }
        self.length_choices = None;
        if targets.is_empty() {
            return;
        }
        let edges = targets
            .iter()
            .filter_map(|&t| match t {
                Target::Curve(_) => None,
                Target::Edge(object, index) => Some(ComponentPick {
                    object,
                    index,
                    kind: viboceros_command::ComponentSelectionKind::BrepEdge,
                }),
            })
            .collect();
        if let Err(e) = stage_sources(&self.document, edges) {
            self.push_log(format!("Error: {e}"));
            return;
        }
        let sources = targets
            .iter()
            .map(|&t| {
                let object = self
                    .document
                    .object(t.object())
                    .filter(|_| self.document.is_object_selectable(t.object()))
                    .ok_or("Length input is unavailable")?;
                if matches!(t, Target::Curve(_)) && object.geometry().curve_ref().is_none() {
                    return Err("Select a curve or surface edge");
                }
                Ok((t.object(), object.geometry_snapshot().clone()))
            })
            .collect::<Result<Vec<_>, _>>();
        let sources = match sources {
            Ok(s) => s,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return;
            }
        };
        if targets.len() > 1 {
            let labels = targets
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let kind = match t {
                        Target::Curve(_) => "curve".to_owned(),
                        Target::Edge(_, e) => format!("surface edge {}", e + 1),
                    };
                    let name = self
                        .document
                        .object(t.object())
                        .and_then(|o| o.attributes().name());
                    format!(
                        "{}: {}",
                        i + 1,
                        name.map_or(kind.clone(), |n| format!("{n} ({kind})"))
                    )
                })
                .collect::<Vec<_>>();
            self.push_log(format!("Length: choose {}", labels.join("; ")));
            self.length_choices = Some(Choices {
                targets,
                sources,
                mode,
                tolerance: self.document.tolerance(),
            });
            self.command_input.clear();
            return;
        }
        let result = match targets[0] {
            Target::Curve(id) => {
                self.select_prompt_objects([id], mode);
                Ok(())
            }
            Target::Edge(object, index) => self
                .commands
                .component_selection()
                .apply(
                    &self.document,
                    vec![ComponentPick {
                        object,
                        index,
                        kind: viboceros_command::ComponentSelectionKind::BrepEdge,
                    }],
                    match mode {
                        SelectionMode::Remove => ComponentAction::Remove,
                        SelectionMode::Toggle => ComponentAction::Toggle,
                        _ => ComponentAction::Add,
                    },
                )
                .map_err(viboceros_command::CommandError::Usage),
        };
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        }
        self.command_input.clear();
    }
    pub(super) fn continue_length_choices(&mut self, input: &str) -> bool {
        if self.length_choices.is_none() {
            return false;
        }
        self.validate_length_choices();
        let Some(p) = &self.length_choices else {
            return true;
        };
        if input.trim().is_empty() {
            self.length_choices = None;
            self.command_input.clear();
            return true;
        }
        if let Ok(n) = input.trim().parse::<usize>() {
            if let Some(&target) = n.checked_sub(1).and_then(|i| p.targets.get(i)) {
                let mode = p.mode;
                self.pick_length_inputs(vec![target], mode);
            } else {
                self.push_log(format!(
                    "Choose a Length input from 1 to {}",
                    p.targets.len()
                ));
            }
            self.command_input.clear();
            return true;
        }
        self.length_choices = None;
        false
    }
    pub(super) fn continue_length_units(&mut self, input: &str) -> bool {
        if !self.length_chain_available() {
            return false;
        }
        let input = input.trim().trim_start_matches('_');
        if self.length_units_pending {
            if input.is_empty() {
                self.length_units_pending = false;
            } else {
                match viboceros_command::distance_display_units(input) {
                    Ok(units) => {
                        self.object_prompt
                            .as_mut()
                            .unwrap()
                            .measurement_display_units = units;
                        self.length_units_pending = false;
                        self.log_object_prompt();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        if input.eq_ignore_ascii_case("Units") {
            self.length_units_pending = true;
            self.command_input.clear();
            self.push_log("Length units: enter a unit name or Model_Units".into());
            return true;
        }
        false
    }
}

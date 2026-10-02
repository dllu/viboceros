//! Shared Untrim/UntrimHoles component prompts and command-local Undo.
use super::*;
use crate::viewport::EdgePick;
use std::collections::BTreeMap;
use viboceros_command::{
    UntrimHolesComponent, UntrimHolesOptions, UntrimHolesSelection, UntrimOptions, UntrimSelection,
};
use viboceros_document::HistoryGroup;

#[derive(Debug)]
struct Candidates {
    picks: Vec<EdgePick>,
    hover: Option<EdgePick>,
    sources: BTreeMap<viboceros_document::ObjectId, viboceros_document::GeometrySnapshot>,
    tolerance: Tolerance,
}

#[derive(Debug)]
pub(super) struct HolePrompt {
    pub(super) options: UntrimHolesOptions,
    general: bool,
    group: HistoryGroup,
    question: Option<&'static str>,
    candidates: Option<Candidates>,
}

impl HolePrompt {
    pub(super) fn name(&self) -> &'static str {
        if self.general {
            "Untrim"
        } else {
            "UntrimHoles"
        }
    }
    fn command_line(&self, options: UntrimHolesOptions) -> String {
        if self.general {
            general_options(options).command_line()
        } else {
            options.command_line()
        }
    }
    fn updated(&self, input: &str) -> Result<UntrimHolesOptions, viboceros_command::CommandError> {
        boundary_options(self.general, input, self.options)
    }
    pub(super) fn picking_edges(&self) -> bool {
        (self.general || !self.options.all) && self.question.is_none()
    }
    pub(super) fn picking_faces(&self) -> bool {
        !self.general && self.options.all && self.question.is_none()
    }
    pub(super) fn hint(&self) -> &'static str {
        match self.question {
            Some("MaximumEdgeLength") => {
                "Type a nonnegative hole perimeter limit; zero disables filtering"
            }
            Some(_) => "Yes / No; Enter keeps the current value",
            None if self.candidates.is_some() => "Choose an edge by number, or pick again",
            None if self.general => {
                "Pick an edge to untrim; Undo reverses the last pick; Enter or Esc finishes"
            }
            None if self.options.all => {
                "Pick a face; Undo reverses the last pick; Enter or Esc finishes"
            }
            None => "Pick a hole edge; Undo reverses the last pick; Enter or Esc finishes",
        }
    }
    pub(super) fn highlights(&self) -> Vec<EdgePick> {
        self.candidates.as_ref().map_or_else(Vec::new, |c| {
            c.hover.map_or_else(|| c.picks.clone(), |pick| vec![pick])
        })
    }
}

fn general_options(options: UntrimHolesOptions) -> UntrimOptions {
    UntrimOptions {
        all_similar: options.all,
        keep_trim_objects: options.keep_trim_objects,
    }
}
fn boundary_options(
    general: bool,
    input: &str,
    previous: UntrimHolesOptions,
) -> Result<UntrimHolesOptions, viboceros_command::CommandError> {
    if !general {
        return previous.updated(input);
    }
    let options = general_options(previous).updated(input)?;
    Ok(UntrimHolesOptions {
        all: options.all_similar,
        keep_trim_objects: options.keep_trim_objects,
        maximum_edge_length: 0.,
    })
}

impl VibocerosApp {
    pub(super) fn try_start_hole_command(&mut self, input: &str) -> bool {
        if !input.split_whitespace().next().is_some_and(|name| {
            ["UntrimHoles", "Untrim"].iter().any(|command| {
                name.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case(command)
            })
        }) {
            return false;
        }
        let descriptor = match self.commands.component_selection_prompt(input) {
            Ok(Some(prompt)) => prompt,
            Ok(None) => return false,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let line = descriptor.command_line();
        let general = descriptor.command == "Untrim";
        let options = boundary_options(
            general,
            line.split_once(' ').unwrap().1,
            UntrimHolesOptions::default(),
        )
        .expect("validated command-owned options");
        let remembered = self
            .commands
            .component_selection_prompt(descriptor.command)
            .expect("built-in prompt")
            .expect("component prompt")
            .command_line();
        let remembered = boundary_options(
            general,
            remembered.split_once(' ').unwrap().1,
            UntrimHolesOptions::default(),
        )
        .expect("remembered options");
        let picks = self
            .component_selection
            .valid_picks(&self.document)
            .into_iter()
            .map(|pick| {
                (
                    pick.object,
                    match pick.kind {
                        viboceros_command::ComponentSelectionKind::BrepEdge => {
                            UntrimHolesComponent::Edge(pick.index)
                        }
                        viboceros_command::ComponentSelectionKind::BrepFace => {
                            UntrimHolesComponent::Face(pick.index)
                        }
                    },
                )
            });
        // Rhino handles preselection before the command's option tokens.
        let prepared = if general {
            Ok(None)
        } else {
            UntrimHolesSelection::prepare_preselected(&self.document, picks, remembered)
        };
        self.cancel_interactive_command(false);
        self.component_selection.clear();
        self.document.clear_selection();
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        match self.document.begin_history_group(descriptor.command) {
            Ok(mut group) => {
                if let Some(prepared) = prepared {
                    match prepared.commit_in_group(&mut self.document, &mut group) {
                        Ok(result) => self.log_hole_result(result),
                        Err(error) => {
                            self.push_log(format!("Error: {error}"));
                            self.command_input.clear();
                            return true;
                        }
                    }
                }
                self.commands
                    .accept_object_selection_input(&line)
                    .expect("validated prompt");
                self.hole_prompt = Some(HolePrompt {
                    options,
                    general,
                    group,
                    question: None,
                    candidates: None,
                });
                self.push_log(format!(
                    "{}: {}",
                    descriptor.command,
                    self.hole_prompt.as_ref().unwrap().hint()
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }

    pub(super) fn try_continue_hole_command(&mut self, input: &str) -> bool {
        let Some(prompt) = &self.hole_prompt else {
            return false;
        };
        let input = input.trim();
        let name = input.trim_start_matches('_');
        if name.eq_ignore_ascii_case("Cancel") || name.eq_ignore_ascii_case("None") {
            self.finish_hole_command(true);
            return true;
        }
        if input.is_empty() {
            if prompt.question.is_some() {
                self.hole_prompt.as_mut().unwrap().question = None;
            } else {
                self.finish_hole_command(true);
            }
            self.command_input.clear();
            return true;
        }
        if name.eq_ignore_ascii_case("Undo") {
            let prompt = self.hole_prompt.as_mut().unwrap();
            prompt.candidates = None;
            prompt.question = None;
            match self.document.undo_history_group(&mut prompt.group) {
                Ok(true) => {
                    let name = prompt.name();
                    self.push_log(format!("Undid the last {name} pick"));
                }
                Ok(false) => self.push_log("No picks to undo".into()),
                Err(error) => {
                    self.finish_hole_command(false);
                    self.push_log(format!("Error: {error}"));
                }
            }
            self.command_input.clear();
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.finish_hole_command(false);
            return false;
        }
        let prompt = self.hole_prompt.as_mut().unwrap();
        if prompt.question.is_none() {
            let choices = if prompt.general {
                &["AllSimilar", "KeepTrimObjects"][..]
            } else {
                &["All", "KeepTrimObjects", "MaximumEdgeLength"][..]
            };
            if let Some(option) = choices
                .iter()
                .copied()
                .into_iter()
                .find(|option| name.eq_ignore_ascii_case(option))
            {
                prompt.question = Some(option);
                prompt.candidates = None;
                self.command_input.clear();
                return true;
            }
            if let Some(candidates) = &prompt.candidates {
                let selected = input
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|n| candidates.picks.get(n))
                    .copied();
                if let Some(pick) = selected {
                    if self.document.tolerance() != candidates.tolerance
                        || self.document.object(pick.object).is_none_or(|object| {
                            candidates.sources.get(&pick.object) != Some(object.geometry_snapshot())
                        })
                    {
                        prompt.candidates = None;
                        self.push_log("Source changed; pick the edge again".into());
                    } else {
                        self.accept_hole_component(
                            pick.object,
                            UntrimHolesComponent::Edge(pick.edge),
                        );
                    }
                    self.command_input.clear();
                    return true;
                }
            }
        }
        let input = prompt
            .question
            .map_or_else(|| input.to_owned(), |option| format!("{option}={input}"));
        match prompt.updated(&input) {
            Ok(options) => {
                if let Err(error) = self
                    .commands
                    .accept_object_selection_input(&prompt.command_line(options))
                {
                    self.push_log(format!("Error: {error}"));
                } else {
                    let prompt = self.hole_prompt.as_mut().unwrap();
                    prompt.options = options;
                    prompt.question = None;
                    prompt.candidates = None;
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_hole_edges(&mut self, picks: Vec<EdgePick>) {
        if !self
            .hole_prompt
            .as_ref()
            .is_some_and(HolePrompt::picking_edges)
        {
            return;
        }
        match picks.as_slice() {
            [] => {}
            &[pick] => {
                self.accept_hole_component(pick.object, UntrimHolesComponent::Edge(pick.edge))
            }
            _ => {
                let mut sources = BTreeMap::new();
                for (index, pick) in picks.iter().enumerate() {
                    if let Some(object) = self.document.object(pick.object) {
                        sources
                            .entry(pick.object)
                            .or_insert_with(|| object.geometry_snapshot().clone());
                    }
                    self.push_log(format!(
                        "{}: object {} edge {}",
                        index + 1,
                        pick.object,
                        pick.edge
                    ));
                }
                self.hole_prompt.as_mut().unwrap().candidates = Some(Candidates {
                    picks,
                    sources,
                    hover: None,
                    tolerance: self.document.tolerance(),
                });
            }
        }
    }

    pub(super) fn accept_hole_component(
        &mut self,
        object: viboceros_document::ObjectId,
        component: UntrimHolesComponent,
    ) {
        let Some(prompt) = &mut self.hole_prompt else {
            return;
        };
        if !self.document.history_group_is_current(&prompt.group) {
            let name = prompt.name();
            self.finish_hole_command(false);
            self.push_log(format!("History changed; start {name} again"));
            return;
        }
        prompt.candidates = None;
        if prompt.general {
            let UntrimHolesComponent::Edge(edge) = component else {
                return;
            };
            match UntrimSelection::prepare(&self.document,object,edge,general_options(prompt.options))
                .and_then(|selection|selection.commit_in_group(&mut self.document,&mut prompt.group)) {
                Ok(result) => self.push_log(format!("Restored {} boundary loop(s) and removed {} wall face(s); retained {} trim object(s)",result.restored_boundaries,result.removed_faces,result.retained.len())),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return;
        }
        match UntrimHolesSelection::prepare(&self.document, object, component, prompt.options)
            .and_then(|selection| selection.commit_in_group(&mut self.document, &mut prompt.group))
        {
            Ok(result) => self.log_hole_result(result),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn accept_hole_rectangle(&mut self, picks: Vec<crate::viewport::ComponentPick>) {
        let Some(prompt) = &mut self.hole_prompt else {
            return;
        };
        if prompt.question.is_some() {
            return;
        }
        if prompt.general {
            if let [pick] = picks.as_slice()
                && pick.kind == viboceros_command::ComponentSelectionKind::BrepEdge
            {
                self.accept_hole_component(pick.object, UntrimHolesComponent::Edge(pick.index));
            } else if !picks.is_empty() {
                self.push_log("Untrim requires a single edge pick".into());
            }
            return;
        }
        if !self.document.history_group_is_current(&prompt.group) {
            self.finish_hole_command(false);
            self.push_log("History changed; start UntrimHoles again".into());
            return;
        }
        prompt.candidates = None;
        let picks = picks.into_iter().map(|pick| {
            (
                pick.object,
                match pick.kind {
                    viboceros_command::ComponentSelectionKind::BrepEdge => {
                        UntrimHolesComponent::Edge(pick.index)
                    }
                    viboceros_command::ComponentSelectionKind::BrepFace => {
                        UntrimHolesComponent::Face(pick.index)
                    }
                },
            )
        });
        match UntrimHolesSelection::prepare_preselected(&self.document, picks, prompt.options) {
            Ok(Some(selection)) => {
                match selection.commit_in_group(&mut self.document, &mut prompt.group) {
                    Ok(result) => self.log_hole_result(result),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            Ok(None) => {}
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn log_hole_result(&mut self, result: viboceros_command::UntrimHolesResult) {
        self.push_log(format!(
            "Removed {} hole opening(s) and {} wall face(s); retained {} trim object(s)",
            result.removed_openings,
            result.removed_faces,
            result.retained.len()
        ));
    }

    pub(super) fn finish_hole_command(&mut self, announce: bool) {
        if let Some(prompt) = self.hole_prompt.take() {
            self.command_input.clear();
            if announce {
                self.push_log(format!("Finished {}", prompt.name()));
            }
        }
    }

    pub(super) fn show_hole_choices(&mut self, ui: &mut egui::Ui) {
        if self.plane_prompt.is_some() || self.set_view_prompt.is_some() {
            return;
        }
        let Some(prompt) = &self.hole_prompt else {
            return;
        };
        let mut chosen = None;
        let mut hover = None;
        ui.horizontal_wrapped(|ui| {
            if let Some(option) = prompt.question {
                if option != "MaximumEdgeLength" {
                    for label in ["Yes", "No"] {
                        if ui.button(label).clicked() {
                            chosen = Some(label.into());
                        }
                    }
                }
            } else {
                for (name, value) in [
                    (
                        if prompt.general { "AllSimilar" } else { "All" },
                        prompt.options.all,
                    ),
                    ("KeepTrimObjects", prompt.options.keep_trim_objects),
                ] {
                    if ui
                        .button(format!("{name}={}", if value { "Yes" } else { "No" }))
                        .clicked()
                    {
                        chosen = Some(format!("{name}={}", if value { "No" } else { "Yes" }));
                    }
                }
                if !prompt.general
                    && ui
                        .button(format!(
                            "MaximumEdgeLength={}",
                            prompt.options.maximum_edge_length
                        ))
                        .clicked()
                {
                    chosen = Some("MaximumEdgeLength".into());
                }
                if let Some(candidates) = &prompt.candidates {
                    for (i, &pick) in candidates.picks.iter().enumerate() {
                        let button = ui.button((i + 1).to_string());
                        if button.hovered() {
                            hover = Some(pick);
                        }
                        if button.clicked() {
                            chosen = Some((i + 1).to_string());
                        }
                    }
                }
                if prompt.group.can_undo() && ui.button("Undo").clicked() {
                    chosen = Some("Undo".into());
                }
            }
            if ui
                .button(if prompt.question.is_some() {
                    "Keep value"
                } else {
                    "Done"
                })
                .clicked()
            {
                chosen = Some(String::new());
            }
        });
        if let Some(candidates) = &mut self.hole_prompt.as_mut().unwrap().candidates {
            candidates.hover = hover;
        }
        if let Some(input) = chosen {
            self.try_continue_hole_command(&input);
        }
    }
}

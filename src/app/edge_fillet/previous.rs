//! Retain previously selected edges without creating model history.
use super::*;
use crate::viewport::ComponentPick;
use std::collections::{BTreeMap, VecDeque};
use viboceros_command::component_selection::{ComponentAction, current, stage_sources};
use viboceros_document::GeometrySnapshot;

const LIMIT: usize = 20;
type EdgeSet = Vec<(ComponentPick, GeometrySnapshot)>;

pub(super) fn argument(word: &str) -> Option<Option<&str>> {
    let (name, value) = word
        .split_once('=')
        .map_or((word, None), |(name, value)| (name, Some(value)));
    name.trim_start_matches('_')
        .eq_ignore_ascii_case("PreviousEdgeSelection")
        .then_some(value)
}

#[derive(Default)]
pub(in crate::app) struct EdgeSelectionHistory {
    commands: BTreeMap<&'static str, VecDeque<EdgeSet>>,
}

impl EdgeSelectionHistory {
    fn remember(&mut self, command: &'static str, edges: EdgeSet) {
        if edges.is_empty() {
            return;
        }
        let sets = self.commands.entry(command).or_default();
        sets.retain(|previous| previous != &edges);
        sets.push_front(edges);
        sets.truncate(LIMIT);
    }

    fn available<'a>(
        &'a self,
        document: &'a Document,
        command: &'static str,
    ) -> impl Iterator<Item = (usize, &'a EdgeSet)> {
        self.commands
            .get(command)
            .into_iter()
            .flat_map(|sets| sets.iter().enumerate())
            .filter_map(move |(index, edges)| {
                edges
                    .chunk_by(|(a, _), (b, _)| a.object == b.object)
                    .all(|group| {
                        let (pick, witness) = &group[0];
                        current(document, *pick, witness)
                    })
                    .then_some((index + 1, edges))
            })
    }
}

impl VibocerosApp {
    pub(super) fn remember_edge_selection(&mut self, prompt: Prompt) {
        let picks = self
            .component_selection
            .valid_picks(&self.document)
            .into_iter()
            .filter(|pick| pick.kind == ComponentSelectionKind::BrepEdge)
            .collect();
        if let Ok(edges) = stage_sources(&self.document, picks) {
            self.edge_selection_history.remember(prompt.name(), edges);
        }
    }

    pub(super) fn recall_edge_selection(&mut self, mut prompt: Prompt, number: Option<&str>) {
        if self.edge_blend_edit_target().is_some() {
            self.push_log("Select edges on the edge blend you are editing.".into());
            self.command_input.clear();
            return;
        }
        self.command_input.clear();
        if self.picking_edge_blend_faces() {
            self.push_log(
                "Press Enter to finish picking faces before recalling an edge set".into(),
            );
            return;
        }
        let number = if let Some(number) = number {
            let index = number
                .trim_start_matches('_')
                .parse::<usize>()
                .ok()
                .filter(|index| (1..=LIMIT).contains(index));
            if index.is_none() {
                self.push_log(format!(
                    "Enter PreviousEdgeSelection with a set number from 1 to {LIMIT}"
                ));
                return;
            }
            index
        } else {
            None
        };
        let available = self
            .edge_selection_history
            .available(&self.document, prompt.name())
            .collect::<Vec<_>>();
        let Some(&(index, edges)) = available
            .iter()
            .find(|(index, _)| number.is_none_or(|n| n == *index))
        else {
            self.push_log(number.map_or_else(
                || "No previous edge sets are available".into(),
                |index| format!("Previous edge set {index} is unavailable"),
            ));
            return;
        };
        let picks = edges.iter().map(|(pick, _)| *pick).collect::<Vec<_>>();
        let count = edges.len();
        let choices = (available.len() > 1).then(|| {
            let numbers = available
                .iter()
                .map(|(index, _)| index.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("Use PreviousEdgeSelection=N to choose another set from {numbers}")
        });
        match self
            .component_selection
            .replace(&self.document, picks.clone())
        {
            Ok(()) => {
                self.retain_edge_blend_picks(&picks);
                self.track_edge_blend_picks(picks, ComponentAction::Add);
                prompt.size_input = None;
                self.edge_fillet_prompt = Some(prompt);
                self.push_log(format!(
                    "Recalled {count} edge(s) from previous set {index}"
                ));
                if let Some(choices) = choices {
                    self.push_log(choices);
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}

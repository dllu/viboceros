//! Edge components are picked in screen space; menus never mutate the document.
use super::*;
use crate::viewport::EdgePick;
use viboceros_command::{MergeEdgeChoice, MergeEdgeSelection};

#[derive(Clone, Debug)]
pub(super) struct EdgeCandidates {
    picks: Vec<EdgePick>,
    hover: Option<EdgePick>,
    sources: std::collections::BTreeMap<viboceros_document::ObjectId, viboceros_document::Geometry>,
    tolerance: Tolerance,
}

#[derive(Clone, Debug)]
pub(super) enum MergeEdgePrompt {
    Pick,
    Ambiguous(EdgeCandidates),
    Choice(Box<MergeEdgeSelection>),
}

impl MergeEdgePrompt {
    pub(super) fn hint(&self) -> &'static str {
        match self {
            Self::Pick => "Pick a surface edge; Enter or Esc cancels",
            Self::Ambiguous(..) => "Choose an edge by number, or pick again; Esc cancels",
            Self::Choice(_) => "Choose a neighboring edge or All; Enter or Esc cancels",
        }
    }
    pub(super) fn highlights(&self) -> Vec<EdgePick> {
        match self {
            Self::Pick => Vec::new(),
            Self::Ambiguous(candidates) => candidates
                .hover
                .map_or_else(|| candidates.picks.clone(), |pick| vec![pick]),
            Self::Choice(selection) => vec![EdgePick {
                object: selection.object(),
                edge: selection.edge(),
            }],
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_start_merge_edge(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("MergeEdge")
        }) || words.next().is_some()
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.merge_edge_prompt = Some(MergeEdgePrompt::Pick);
        self.command_input.clear();
        self.push_log("MergeEdge: pick a surface or polysurface edge".into());
        true
    }

    pub(super) fn try_continue_merge_edge(&mut self, input: &str) -> bool {
        let Some(prompt) = &self.merge_edge_prompt else {
            return false;
        };
        if input.is_empty()
            || matches!(
                input.trim_start_matches('_').to_ascii_lowercase().as_str(),
                "cancel" | "none"
            )
        {
            self.cancel_merge_edge(true);
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_merge_edge(true);
            return false;
        }
        match prompt {
            MergeEdgePrompt::Ambiguous(candidates) => {
                if let Some(pick) = input
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|i| candidates.picks.get(i))
                    .copied()
                {
                    if self.document.tolerance() != candidates.tolerance
                        || self.document.object(pick.object).is_none_or(|o| {
                            candidates.sources.get(&pick.object) != Some(o.geometry())
                        })
                    {
                        self.merge_edge_prompt = Some(MergeEdgePrompt::Pick);
                        self.push_log("Source changed; pick the edge again".into());
                    } else {
                        self.pick_merge_edge(pick);
                    }
                } else {
                    self.push_log("Choose one of the listed edge numbers".into());
                }
            }
            MergeEdgePrompt::Choice(selection) => {
                if let Some(choice) =
                    MergeEdgeChoice::parse(input).filter(|c| selection.choices().contains(c))
                {
                    self.finish_merge_edge(choice);
                } else {
                    self.push_log("Choose one of the listed MergeEdge options".into());
                }
            }
            MergeEdgePrompt::Pick => {
                self.push_log("Pick an edge in a viewport (not a construction-plane point)".into())
            }
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_edge_click(&mut self, picks: Vec<EdgePick>) {
        if self.merge_edge_prompt.is_none() {
            return;
        }
        match picks.as_slice() {
            [] => {}
            &[pick] => self.pick_merge_edge(pick),
            _ => {
                for (i, pick) in picks.iter().enumerate() {
                    self.push_log(format!(
                        "{}: object {} edge {}",
                        i + 1,
                        pick.object,
                        pick.edge
                    ));
                }
                let mut sources = std::collections::BTreeMap::new();
                for pick in &picks {
                    if let Some(object) = self.document.object(pick.object) {
                        sources
                            .entry(pick.object)
                            .or_insert_with(|| object.geometry().clone());
                    }
                }
                self.merge_edge_prompt = Some(MergeEdgePrompt::Ambiguous(EdgeCandidates {
                    picks,
                    hover: None,
                    sources,
                    tolerance: self.document.tolerance(),
                }));
            }
        }
    }

    fn pick_merge_edge(&mut self, pick: EdgePick) {
        match MergeEdgeSelection::prepare(&self.document, pick.object, pick.edge) {
            Ok(selection) if !selection.choices().is_empty() => {
                self.push_log(format!(
                    "MergeEdge: {} (Esc cancels)",
                    selection
                        .choices()
                        .iter()
                        .map(|c| c.name())
                        .collect::<Vec<_>>()
                        .join(" / ")
                ));
                self.merge_edge_prompt = Some(MergeEdgePrompt::Choice(Box::new(selection)));
            }
            Ok(_) => {
                self.merge_edge_prompt = Some(MergeEdgePrompt::Pick);
                self.push_log("No certified mergeable neighbor; pick another edge".into());
            }
            Err(error) => {
                self.merge_edge_prompt = Some(MergeEdgePrompt::Pick);
                self.push_log(format!("Error: {error}"));
            }
        }
    }

    fn finish_merge_edge(&mut self, choice: MergeEdgeChoice) {
        let Some(MergeEdgePrompt::Choice(selection)) = &self.merge_edge_prompt else {
            return;
        };
        match selection.commit(&mut self.document, choice) {
            Ok(report) => {
                self.merge_edge_prompt = None;
                self.command_input.clear();
                self.push_log(report);
            }
            Err(error) => {
                self.merge_edge_prompt = Some(MergeEdgePrompt::Pick);
                self.push_log(format!("Error: {error}"));
            }
        }
    }

    pub(super) fn cancel_merge_edge(&mut self, announce: bool) {
        if self.merge_edge_prompt.take().is_some() {
            self.command_input.clear();
            if announce {
                self.push_log("Cancelled MergeEdge".into());
            }
        }
    }

    pub(super) fn show_merge_edge_choices(&mut self, ui: &mut egui::Ui) {
        if self.plane_prompt.is_some() {
            return;
        }
        let Some(prompt) = &self.merge_edge_prompt else {
            return;
        };
        let labels: Vec<String> = match prompt {
            MergeEdgePrompt::Choice(selection) => selection
                .choices()
                .iter()
                .map(|c| c.name().to_owned())
                .collect(),
            MergeEdgePrompt::Ambiguous(candidates) => (1..=candidates.picks.len())
                .map(|i| i.to_string())
                .collect(),
            MergeEdgePrompt::Pick => Vec::new(),
        };
        let mut chosen = None;
        let mut hover = None;
        ui.horizontal_wrapped(|ui| {
            for (i, label) in labels.into_iter().enumerate() {
                let button = ui.button(&label);
                if button.hovered()
                    && let MergeEdgePrompt::Ambiguous(candidates) = prompt
                {
                    hover = candidates.picks.get(i).copied();
                }
                if button.clicked() {
                    chosen = Some(label);
                }
            }
            if ui.button("Cancel").clicked() {
                chosen = Some("Cancel".to_owned());
            }
        });
        if let Some(MergeEdgePrompt::Ambiguous(candidates)) = &mut self.merge_edge_prompt {
            candidates.hover = hover;
        }
        if let Some(choice) = chosen {
            self.try_continue_merge_edge(&choice);
        }
    }
}

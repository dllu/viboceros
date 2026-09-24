//! Shared component selection; command-specific merge choices and split points.
use super::*;
use crate::viewport::EdgePick;
use viboceros_command::{MergeEdgeChoice, MergeEdgeSelection, SplitEdgeSelection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EdgeCommand {
    Merge,
    Split,
}
impl EdgeCommand {
    fn name(self) -> &'static str {
        match self {
            Self::Merge => "MergeEdge",
            Self::Split => "SplitEdge",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct EdgeCandidates {
    picks: Vec<EdgePick>,
    hover: Option<EdgePick>,
    sources: std::collections::BTreeMap<viboceros_document::ObjectId, viboceros_document::Geometry>,
    tolerance: Tolerance,
}

#[derive(Clone, Debug)]
pub(super) enum EdgePrompt {
    Pick(EdgeCommand),
    Ambiguous(EdgeCommand, EdgeCandidates),
    Choice(Box<MergeEdgeSelection>),
    SplitPoints(Box<SplitEdgeSelection>),
}

impl EdgePrompt {
    fn kind(&self) -> EdgeCommand {
        match self {
            Self::Pick(kind) | Self::Ambiguous(kind, _) => *kind,
            Self::Choice(_) => EdgeCommand::Merge,
            Self::SplitPoints(_) => EdgeCommand::Split,
        }
    }
    pub(super) fn name(&self) -> &'static str {
        self.kind().name()
    }
    pub(super) fn picking_edge(&self) -> bool {
        !matches!(self, Self::SplitPoints(_))
    }
    pub(super) fn split_selection(&self) -> Option<&SplitEdgeSelection> {
        if let Self::SplitPoints(selection) = self {
            Some(selection)
        } else {
            None
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        match self {
            Self::Pick(_) => "Pick a surface edge; Enter or Esc cancels",
            Self::Ambiguous(..) => "Choose an edge by number, or pick again; Esc cancels",
            Self::Choice(_) => "Choose a neighboring edge or All; Enter or Esc cancels",
            Self::SplitPoints(_) => {
                "Pick edge points or type a distance; Enter or Esc applies the batch"
            }
        }
    }
    pub(super) fn highlights(&self) -> Vec<EdgePick> {
        match self {
            Self::Pick(_) => Vec::new(),
            Self::Ambiguous(_, candidates) => candidates
                .hover
                .map_or_else(|| candidates.picks.clone(), |pick| vec![pick]),
            Self::Choice(selection) => vec![EdgePick {
                object: selection.object(),
                edge: selection.edge(),
            }],
            Self::SplitPoints(selection) => vec![EdgePick {
                object: selection.object(),
                edge: selection.edge(),
            }],
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_start_edge_command(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let kind = match words
            .next()
            .map(|name| name.trim_start_matches(['_', '-']).to_ascii_lowercase())
            .as_deref()
        {
            Some("mergeedge") => EdgeCommand::Merge,
            Some("splitedge") => EdgeCommand::Split,
            _ => return false,
        };
        if words.next().is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.edge_prompt = Some(EdgePrompt::Pick(kind));
        self.command_input.clear();
        self.push_log(format!(
            "{}: pick a surface or polysurface edge",
            kind.name()
        ));
        true
    }

    pub(super) fn try_continue_edge_command(&mut self, input: &str) -> bool {
        let Some(prompt) = &self.edge_prompt else {
            return false;
        };
        if input.is_empty()
            || matches!(
                input.trim_start_matches('_').to_ascii_lowercase().as_str(),
                "cancel" | "none"
            )
        {
            self.finish_edge_command(true);
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.finish_edge_command(true);
            return false;
        }
        match prompt {
            EdgePrompt::Ambiguous(kind, candidates) => {
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
                        self.edge_prompt = Some(EdgePrompt::Pick(*kind));
                        self.push_log("Source changed; pick the edge again".into());
                    } else {
                        self.prepare_edge_command(pick);
                    }
                } else {
                    self.push_log("Choose one of the listed edge numbers".into());
                }
            }
            EdgePrompt::Choice(selection) => {
                if let Some(choice) =
                    MergeEdgeChoice::parse(input).filter(|c| selection.choices().contains(c))
                {
                    self.finish_merge_edge(choice);
                } else {
                    self.push_log("Choose one of the listed MergeEdge options".into());
                }
            }
            EdgePrompt::Pick(_) => {
                self.push_log("Pick an edge in a viewport (not a construction-plane point)".into())
            }
            EdgePrompt::SplitPoints(_) => {
                if let Ok(distance) = input.parse::<f64>() {
                    let Some(EdgePrompt::SplitPoints(selection)) = &mut self.edge_prompt else {
                        unreachable!()
                    };
                    match selection
                        .validate_source(&self.document)
                        .and_then(|()| selection.set_distance(distance))
                    {
                        Ok(()) => self.push_log(if distance == 0. {
                            "SplitEdge: distance constraint cleared".into()
                        } else {
                            format!(
                                "SplitEdge: arc distance {}; pick the next point",
                                distance.abs()
                            )
                        }),
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                } else if let Some(point) =
                    viboceros_drafting::PointInput::parse_with_units(input, self.document.units())
                {
                    match point.and_then(|p| {
                        p.resolve(
                            self.viewports[self.active_viewport].construction_plane(),
                            self.last_point,
                        )
                    }) {
                        Ok(point) => self.accept_split_point(point),
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                } else {
                    self.push_log(
                        "Pick a point on the edge, type coordinates, or press Enter".into(),
                    );
                }
            }
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_edge_click(&mut self, picks: Vec<EdgePick>) {
        let Some(prompt) = &self.edge_prompt else {
            return;
        };
        if !prompt.picking_edge() {
            return;
        }
        let kind = prompt.kind();
        match picks.as_slice() {
            [] => {}
            &[pick] => self.prepare_edge_command(pick),
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
                self.edge_prompt = Some(EdgePrompt::Ambiguous(
                    kind,
                    EdgeCandidates {
                        picks,
                        hover: None,
                        sources,
                        tolerance: self.document.tolerance(),
                    },
                ));
            }
        }
    }

    fn prepare_edge_command(&mut self, pick: EdgePick) {
        if self
            .edge_prompt
            .as_ref()
            .is_some_and(|p| p.kind() == EdgeCommand::Split)
        {
            match SplitEdgeSelection::prepare(&self.document, pick.object, pick.edge) {
                Ok(selection) => {
                    self.edge_prompt = Some(EdgePrompt::SplitPoints(Box::new(selection)));
                    self.push_log(
                        "SplitEdge: pick points on the edge; Enter or Esc applies the batch".into(),
                    );
                }
                Err(error) => {
                    self.edge_prompt = Some(EdgePrompt::Pick(EdgeCommand::Split));
                    self.push_log(format!("Error: {error}"));
                }
            }
            return;
        }
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
                self.edge_prompt = Some(EdgePrompt::Choice(Box::new(selection)));
            }
            Ok(_) => {
                self.edge_prompt = Some(EdgePrompt::Pick(EdgeCommand::Merge));
                self.push_log("No certified mergeable neighbor; pick another edge".into());
            }
            Err(error) => {
                self.edge_prompt = Some(EdgePrompt::Pick(EdgeCommand::Merge));
                self.push_log(format!("Error: {error}"));
            }
        }
    }

    fn finish_merge_edge(&mut self, choice: MergeEdgeChoice) {
        let Some(EdgePrompt::Choice(selection)) = &self.edge_prompt else {
            return;
        };
        match selection.commit(&mut self.document, choice) {
            Ok(report) => {
                self.edge_prompt = None;
                self.command_input.clear();
                self.push_log(report);
            }
            Err(error) => {
                self.edge_prompt = Some(EdgePrompt::Pick(EdgeCommand::Merge));
                self.push_log(format!("Error: {error}"));
            }
        }
    }

    pub(super) fn finish_edge_command(&mut self, announce: bool) {
        if let Some(prompt) = self.edge_prompt.take() {
            self.snaps.model_override = None;
            self.command_input.clear();
            if let EdgePrompt::SplitPoints(selection) = prompt {
                match selection.commit(&mut self.document) {
                    Ok(message) => self.push_log(message),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            } else if announce {
                self.push_log(format!("Cancelled {}", prompt.name()));
            }
        }
    }

    fn accept_split_point(&mut self, point: Point3) {
        let Some(EdgePrompt::SplitPoints(selection)) = &mut self.edge_prompt else {
            return;
        };
        let result = selection
            .validate_source(&self.document)
            .and_then(|()| selection.add_point(point));
        self.report_split_point(result);
    }

    pub(super) fn accept_split_parameter(&mut self, parameter: f64) {
        let Some(EdgePrompt::SplitPoints(selection)) = &mut self.edge_prompt else {
            return;
        };
        let result = selection
            .validate_source(&self.document)
            .and_then(|()| selection.add_parameter(parameter));
        self.report_split_point(result);
    }

    fn report_split_point(&mut self, result: Result<(), viboceros_command::CommandError>) {
        let Some(EdgePrompt::SplitPoints(selection)) = &self.edge_prompt else {
            return;
        };
        match result {
            Ok(()) => {
                self.snaps.model_override = None;
                self.last_point = selection
                    .parameters()
                    .last()
                    .and_then(|&t| selection.curve().evaluate(t).ok());
                let count = selection.parameters().len();
                self.push_log(format!(
                    "SplitEdge: {count} point(s); Enter or Esc applies the batch"
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
    }

    pub(super) fn show_edge_choices(&mut self, ui: &mut egui::Ui) {
        if self.plane_prompt.is_some() {
            return;
        }
        let Some(prompt) = &self.edge_prompt else {
            return;
        };
        let labels: Vec<String> = match prompt {
            EdgePrompt::Choice(selection) => selection
                .choices()
                .iter()
                .map(|c| c.name().to_owned())
                .collect(),
            EdgePrompt::Ambiguous(_, candidates) => (1..=candidates.picks.len())
                .map(|i| i.to_string())
                .collect(),
            EdgePrompt::Pick(_) | EdgePrompt::SplitPoints(_) => Vec::new(),
        };
        let mut chosen = None;
        let mut hover = None;
        ui.horizontal_wrapped(|ui| {
            for (i, label) in labels.into_iter().enumerate() {
                let button = ui.button(&label);
                if button.hovered()
                    && let EdgePrompt::Ambiguous(_, candidates) = prompt
                {
                    hover = candidates.picks.get(i).copied();
                }
                if button.clicked() {
                    chosen = Some(label);
                }
            }
            if ui
                .button(if matches!(prompt, EdgePrompt::SplitPoints(_)) {
                    "Done"
                } else {
                    "Cancel"
                })
                .clicked()
            {
                chosen = Some("Cancel".to_owned());
            }
        });
        if let Some(EdgePrompt::Ambiguous(_, candidates)) = &mut self.edge_prompt {
            candidates.hover = hover;
        }
        if let Some(choice) = chosen {
            self.try_continue_edge_command(&choice);
        }
    }
}

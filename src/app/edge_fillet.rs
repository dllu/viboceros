//! Shared original-edge picking and confirmation for native fillets, chamfers and G2 blends.
use super::*;
use nested_measurement::Kind as Measurement;
use viboceros_command::blend_edge::BlendOptions;
use viboceros_command::chamfer_edge::ChamferOptions;
use viboceros_command::component_selection::ComponentAction;
use viboceros_command::{
    ComponentSelectionKind, EdgeBlendKind as Kind, EdgeBlendPlan, FilletOptions,
};

mod adjustment;
pub(super) mod edit;
mod face_edges;
mod preview;
mod previous;
mod size_input;
pub(super) use adjustment::Adjustment;
pub(super) use edit::Edit;
pub(super) use face_edges::FaceEdgeSelection;
pub(super) use preview::Preview;
pub(super) use previous::EdgeSelectionHistory;
use size_input::{NextSize, SizeInput, next_size_option, take_next_size};

pub(super) const FACE_EDGES_HINT: &str = "Select faces to add their sharp boundary edges. Enter keeps the picks. Esc discards these face picks.";
pub(super) const FROM_CURVE_HINT: &str =
    "Pick a curve location or type its coordinates. Esc returns to edge selection.";

#[derive(Clone, Copy, Debug)]
pub(super) struct Prompt {
    radius: f64,
    kind: Kind,
    tolerance: Tolerance,
    size_input: Option<SizeInput>,
    adjusting: bool,
    toggles: Toggles,
}

impl Prompt {
    pub(super) fn name(self) -> &'static str {
        self.kind.command()
    }
    fn parameter(self) -> &'static str {
        if matches!(self.kind, Kind::Chamfer(_)) {
            "Distance"
        } else {
            "Radius"
        }
    }
    fn show_option(self) -> &'static str {
        if matches!(self.kind, Kind::Chamfer(_)) {
            "ShowChamferDistance"
        } else {
            "ShowRadius"
        }
    }
    fn next_option(self) -> &'static str {
        next_size_option(matches!(self.kind, Kind::Chamfer(_)))
    }
    pub(super) fn hint(self) -> &'static str {
        if self.size_input.is_some() {
            if self.size_input == Some(SizeInput::Next) {
                return if matches!(self.kind, Kind::Chamfer(_)) {
                    "Enter a distance for the next chamfer edge"
                } else {
                    "Enter a radius for the next edge"
                };
            }
            if matches!(self.kind, Kind::Chamfer(_)) {
                "Enter chamfer distance with optional units"
            } else {
                "Enter edge radius with optional units"
            }
        } else if self.adjusting {
            "Select a size handle or use AddHandle, CopyHandle, SetAll, LinkHandles, SelectEdges or TrimAndJoin. Enter applies. Esc cancels."
        } else {
            match self.kind {
                Kind::Fillet(_) => {
                    "Select edges, or use ChainEdges, FaceEdges, PreviousEdgeSelection or Edit. Set NextRadius, Corner, TrimAndJoin or Preview."
                }
                Kind::Chamfer(_) => {
                    "Select edges, or use ChainEdges, FaceEdges, PreviousEdgeSelection or Edit. Set NextChamferDistance, RailType, TrimAndJoin or Preview."
                }
                Kind::Curvature(_) => {
                    "Select edges, or use ChainEdges, FaceEdges, PreviousEdgeSelection or Edit. Set NextRadius, RailType, TrimAndJoin or Preview."
                }
            }
        }
    }
    fn options(self) -> String {
        let parameter = if self.adjusting {
            self.parameter()
        } else {
            self.next_option()
        };
        let model = match self.kind {
            Kind::Fillet(corner) => {
                format!("{parameter}={} Corner={}", self.radius, corner.token())
            }
            Kind::Chamfer(rail) | Kind::Curvature(rail) => {
                format!("{}={} RailType={}", parameter, self.radius, rail.token())
            }
        };
        let yes_no = |value| if value { "Yes" } else { "No" };
        format!(
            "{model} TrimAndJoin={} Preview={} {}={} LinkHandles={}",
            yes_no(self.toggles.trim_and_join),
            yes_no(self.toggles.preview),
            self.show_option(),
            yes_no(self.toggles.show_handles),
            yes_no(self.toggles.link_handles)
        )
    }
}

fn size_measurement(word: &str) -> Option<Measurement> {
    let word = word.trim_start_matches('_');
    if word.eq_ignore_ascii_case("FromCurve") {
        Some(Measurement::Radius)
    } else if word.eq_ignore_ascii_case("FromTwoPoints") {
        Some(Measurement::Distance)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct Toggles {
    preview: bool,
    show_handles: bool,
    link_handles: bool,
    trim_and_join: bool,
}

impl Default for Toggles {
    fn default() -> Self {
        Self {
            preview: false,
            show_handles: true,
            link_handles: false,
            trim_and_join: true,
        }
    }
}

fn toggle_options<'a>(
    words: impl IntoIterator<Item = &'a str>,
    mut toggles: Toggles,
    show_option: &'static str,
) -> Result<(Vec<&'a str>, Toggles), &'static str> {
    let mut modeling = Vec::new();
    for word in words {
        let option = word.trim_start_matches('_');
        let (name, value) = option
            .split_once('=')
            .map_or((option, None), |(name, value)| {
                (name, Some(value.trim_start_matches('_')))
            });
        let target = if name.eq_ignore_ascii_case("Preview") {
            &mut toggles.preview
        } else if name.eq_ignore_ascii_case(show_option) {
            &mut toggles.show_handles
        } else if name.eq_ignore_ascii_case("LinkHandles") {
            &mut toggles.link_handles
        } else if name.eq_ignore_ascii_case("TrimAndJoin") {
            &mut toggles.trim_and_join
        } else {
            modeling.push(word);
            continue;
        };
        *target = match value {
            None => !*target,
            Some(v) if v.eq_ignore_ascii_case("Yes") => true,
            Some(v) if v.eq_ignore_ascii_case("No") => false,
            _ => return Err("expected Yes or No for the edge blend option"),
        };
    }
    Ok((modeling, toggles))
}

impl VibocerosApp {
    pub(super) fn try_start_edge_fillet(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(name) = words.next().map(|n| n.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let chamfer = name.eq_ignore_ascii_case("ChamferEdge");
        let curvature = name.eq_ignore_ascii_case("BlendEdge");
        if !chamfer && !curvature && !name.eq_ignore_ascii_case("FilletEdge") {
            return false;
        }
        let show_option = if chamfer {
            "ShowChamferDistance"
        } else {
            "ShowRadius"
        };
        let (mut words, toggles) = match toggle_options(words, Toggles::default(), show_option) {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        let previous = words
            .iter()
            .position(|word| previous::argument(word).is_some())
            .and_then(|index| previous::argument(words.remove(index)));
        let edit = words
            .iter()
            .position(|word| edit::argument(word).is_some())
            .and_then(|index| edit::argument(words.remove(index)));
        let chain_options = words
            .iter()
            .position(|word| {
                word.trim_start_matches('_')
                    .eq_ignore_ascii_case("ChainEdges")
            })
            .map(|index| words.split_off(index)[1..].join(" "));
        let measurement = words
            .iter()
            .position(|word| size_measurement(word).is_some())
            .and_then(|index| size_measurement(words.remove(index)));
        if chain_options.is_some() && measurement.is_some() {
            self.command_input.clear();
            self.push_log("Finish the chain selection before measuring its size".into());
            return true;
        }
        let face_edges = words.iter().position(|word| {
            word.trim_start_matches('_')
                .eq_ignore_ascii_case("FaceEdges")
        });
        if let Some(index) = face_edges {
            words.remove(index);
        }
        let next_size = match take_next_size(&mut words, next_size_option(chamfer), &self.document)
        {
            Ok(size) => size,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        let modeling_input = std::iter::once(name)
            .chain(words.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        let descriptor = match self.commands.component_selection_prompt(&modeling_input) {
            Ok(Some(p)) => p,
            Ok(None) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_input.clear();
                return true;
            }
        };
        let picks = if edit.is_some() {
            Vec::new()
        } else {
            match self.component_selection.checked_picks(&self.document) {
                Ok(picks) => picks,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            }
        };
        if picks
            .iter()
            .any(|p| p.kind != ComponentSelectionKind::BrepEdge)
        {
            self.push_log("Error: select polysurface edge components".into());
            return true;
        }
        let kind = if chamfer {
            let mut o = ChamferOptions::default();
            if let Err(e) = o.update(&words) {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Kind::Chamfer(o.rail)
        } else if curvature {
            let mut o = BlendOptions::default();
            if let Err(e) = o.update(&words) {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Kind::Curvature(o.rail)
        } else {
            let mut options = FilletOptions::default();
            if let Err(error) = options.update(&words) {
                self.push_log(format!("Error: {error}"));
                return true;
            }
            Kind::Fillet(options.corner)
        };
        self.cancel_interactive_command(false);
        let mut prompt = Prompt {
            radius: descriptor.numbers[0].value,
            kind,
            tolerance: self.document.tolerance(),
            size_input: matches!(next_size, Some(NextSize::Ask)).then_some(SizeInput::Next),
            adjusting: false,
            toggles,
        };
        self.edge_fillet_prompt = Some(prompt);
        self.track_edge_blend_picks(picks, ComponentAction::Add);
        if let Some(NextSize::Value(value)) = next_size {
            prompt.radius = value;
            self.edge_fillet_prompt = Some(prompt);
        }
        self.command_input.clear();
        self.document.clear_selection();
        self.log_edge_blend_prompt();
        if let Some(edit) = edit {
            self.begin_edge_blend_edit(edit);
            return true;
        }
        if let Some(number) = previous {
            self.recall_edge_selection(prompt, number);
        }
        if let Some(chain_options) = chain_options {
            self.begin_edge_blend_chain(&chain_options);
        } else if face_edges.is_some() {
            self.begin_edge_blend_faces();
        }
        if let Some(kind) = measurement {
            self.start_edge_blend_measurement(kind);
        }
        true
    }
    pub(super) fn try_continue_edge_fillet(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt else {
            return false;
        };
        let input = input.trim();
        let keyword = input.trim_start_matches('_');
        let (name, options) = keyword
            .split_once(char::is_whitespace)
            .unwrap_or((keyword, ""));
        let (option, inline_value) = name.split_once('=').unwrap_or((name, options));
        if self.picking_edge_blend_edit() {
            if input.is_empty() || keyword.eq_ignore_ascii_case("Enter") {
                self.confirm_edge_blend_edit_object();
            } else if keyword.eq_ignore_ascii_case("Cancel") {
                self.finish_edge_fillet(false);
            } else if let Ok(id) = input.parse::<ObjectId>() {
                self.select_edge_blend_edit_object(id);
            } else if input
                .split_whitespace()
                .next()
                .is_some_and(|n| self.commands.recognizes(n))
            {
                self.finish_edge_fillet(false);
                return false;
            } else {
                self.log_edge_blend_prompt();
            }
            self.command_input.clear();
            return true;
        }
        if !prompt.adjusting
            && self.edge_blend_edit.is_none()
            && let Some(edit) = edit::argument(name)
        {
            self.begin_edge_blend_edit(edit.or_else(|| (!options.is_empty()).then_some(options)));
            return true;
        }
        let set_all = option.eq_ignore_ascii_case("SetAll") && prompt.adjusting;
        let current_chamfer = matches!(prompt.kind, Kind::Chamfer(_))
            && prompt.adjusting
            && option.eq_ignore_ascii_case("CurrentChamferDistance");
        if current_chamfer && self.edge_blend_adjustment.active.is_none() {
            self.command_input.clear();
            self.push_log("Select a chamfer distance handle first.".into());
            return true;
        }
        if prompt.adjusting && self.start_edge_handle_operation(prompt, keyword) {
            return true;
        }
        if prompt.adjusting && keyword.eq_ignore_ascii_case("SelectEdges") {
            self.reopen_edge_blend_selection(prompt);
            return true;
        }
        if prompt.adjusting
            && (name.eq_ignore_ascii_case("ChainEdges")
                || keyword.eq_ignore_ascii_case("FaceEdges")
                || previous::argument(name).is_some())
        {
            self.push_log("Use SelectEdges to return to edge selection".into());
            self.command_input.clear();
            return true;
        }
        if let Some(number) = previous::argument(name) {
            let number = number.or_else(|| (!options.trim().is_empty()).then_some(options.trim()));
            self.recall_edge_selection(prompt, number);
            return true;
        }
        if name.eq_ignore_ascii_case("ChainEdges") {
            self.begin_edge_blend_chain(options);
            return true;
        }
        if keyword.eq_ignore_ascii_case("FaceEdges") {
            self.begin_edge_blend_faces();
            return true;
        }
        if input.is_empty() || keyword.eq_ignore_ascii_case("Enter") {
            if self.finish_edge_blend_faces(true) {
                return true;
            }
            if !self.dismiss_edge_handle_operation() && !self.dismiss_edge_size_input() {
                if prompt.adjusting {
                    self.finish_edge_fillet(true);
                } else {
                    self.begin_edge_blend_adjustment(prompt);
                }
            }
            return true;
        }
        if keyword.eq_ignore_ascii_case("Cancel") {
            if self.finish_edge_blend_faces(false) {
                return true;
            }
            if !self.dismiss_edge_handle_operation() && !self.dismiss_edge_size_input() {
                self.finish_edge_fillet(false);
            }
            return true;
        }
        if let Some(kind) = size_measurement(input) {
            return self.start_edge_blend_measurement(kind);
        }
        if keyword.eq_ignore_ascii_case(prompt.parameter())
            || ((set_all || current_chamfer) && inline_value.trim().is_empty())
        {
            if set_all {
                self.edge_blend_adjustment.active = None;
                self.edge_blend_adjustment.drag_size = None;
            }
            prompt.size_input = Some(if set_all {
                SizeInput::All
            } else {
                SizeInput::Edit
            });
            self.edge_fillet_prompt = Some(prompt);
            self.component_selection.clear_choices();
            self.command_input.clear();
            self.push_log(format!(
                "{} {} (model units or unit suffix)",
                prompt.name(),
                prompt.parameter()
            ));
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|n| self.commands.recognizes(n))
        {
            self.finish_edge_fillet(false);
            return false;
        }
        let input = if set_all || current_chamfer {
            inline_value.trim()
        } else {
            input
        };
        let (mut modeling, toggles) = match toggle_options(
            input.split_whitespace(),
            prompt.toggles,
            prompt.show_option(),
        ) {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        if self.try_next_edge_blend_size(prompt, &mut modeling, toggles) {
            return true;
        }
        let mut words = Vec::new();
        for word in modeling {
            let (key, value) = word
                .trim_start_matches('_')
                .split_once('=')
                .unwrap_or((prompt.parameter(), word));
            if key.eq_ignore_ascii_case(prompt.parameter()) {
                match viboceros_drafting::PointInput::parse_length_with_units(
                    value,
                    self.document.units(),
                ) {
                    Some(Ok(n)) => words.push(format!("{}={n}", prompt.parameter())),
                    Some(Err(e)) => {
                        self.push_log(format!("Error: {e}"));
                        self.command_input.clear();
                        return true;
                    }
                    None => words.push(word.to_owned()),
                }
            } else {
                words.push(word.to_owned());
            }
        }
        let words = words.iter().map(String::as_str).collect::<Vec<_>>();
        let edits_size = words.iter().any(|word| {
            word.split_once('=')
                .is_some_and(|(name, _)| name.eq_ignore_ascii_case(prompt.parameter()))
        });
        let default = prompt.radius;
        let result = match prompt.kind {
            Kind::Fillet(corner) => {
                let mut options = FilletOptions {
                    radius: prompt.radius,
                    corner,
                };
                options.update(&words).map(|()| {
                    prompt.radius = options.radius;
                    prompt.kind = Kind::Fillet(options.corner);
                })
            }
            Kind::Chamfer(rail) => {
                let mut options = ChamferOptions {
                    distance: prompt.radius,
                    rail,
                };
                options.update(&words).map(|()| {
                    prompt.radius = options.distance;
                    prompt.kind = Kind::Chamfer(options.rail);
                })
            }
            Kind::Curvature(rail) => {
                let mut options = BlendOptions {
                    radius: prompt.radius,
                    rail,
                };
                options.update(&words).map(|()| {
                    prompt.radius = options.radius;
                    prompt.kind = Kind::Curvature(options.rail);
                })
            }
        };
        match result {
            Ok(()) => {
                prompt.toggles = toggles;
                if edits_size {
                    if let Err(error) = self.store_edge_blend_size(&mut prompt, default, set_all) {
                        self.push_log(format!("Error: {error}"));
                        self.command_input.clear();
                        return true;
                    }
                }
                if !words.is_empty() {
                    prompt.size_input = None;
                }
                self.edge_fillet_prompt = Some(prompt);
                self.log_edge_blend_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        self.command_input.clear();
        true
    }

    pub(super) fn log_edge_blend_prompt(&mut self) {
        if let Some(prompt) = self.edge_fillet_prompt {
            let hint = if self.picking_edge_blend_edit() {
                "Select an object with an editable edge blend."
            } else if self.picking_edge_blend_faces() {
                FACE_EDGES_HINT
            } else {
                self.edge_handle_operation_hint()
                    .unwrap_or_else(|| prompt.hint())
            };
            let remove = if prompt.adjusting && self.has_added_edge_handles() {
                " Use RemoveHandle to delete an added handle."
            } else {
                ""
            };
            self.push_log(format!(
                "{} {}. {hint}{remove}",
                prompt.name(),
                prompt.options()
            ));
        }
    }

    fn capture_edge_blend(
        &self,
        prompt: Prompt,
        trial_size: Option<f64>,
    ) -> Result<EdgeBlendPlan, String> {
        if prompt.tolerance != self.document.tolerance() {
            return Err("tolerance changed; select edges again".into());
        }
        if matches!(&self.edge_blend_edit, Some(Edit::Active(session)) if !session.is_current(&self.document))
        {
            return Err("The edge blend changed. Reopen Edit to continue.".into());
        }
        let document = self.edge_blend_source_document();
        let edit = self.edge_blend_trial(prompt, trial_size);
        let picks = self
            .component_selection
            .checked_picks(document)?
            .into_iter()
            .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
            .map(|p| (p.object, p.index, self.edge_blend_sizes(prompt, p, edit)));
        EdgeBlendPlan::capture_profiles(document, picks, prompt.kind)
            .map(|plan| plan.with_trim_and_join(prompt.toggles.trim_and_join))
            .map_err(|error| error.to_string())
    }

    pub(super) fn finish_edge_fillet(&mut self, apply: bool) {
        let Some(prompt) = self.edge_fillet_prompt.take() else {
            return;
        };
        self.edge_blend_faces = None;
        self.command_input.clear();
        if !apply {
            match self.edge_blend_edit.take() {
                None => self.remember_edge_selection(prompt),
                Some(Edit::Picking(_)) => {
                    self.document.clear_selection();
                }
                Some(Edit::Active(_)) => {}
            }
            self.edge_blend_adjustment = Default::default();
            self.component_selection.clear();
            self.edge_blend_preview.clear();
            self.push_log(format!("Cancelled {}", prompt.name()));
            return;
        }
        let result = self
            .capture_edge_blend(prompt, None)
            .and_then(|plan| self.edge_blend_preview.selection(plan))
            .and_then(|s| {
                match &self.edge_blend_edit {
                    Some(Edit::Active(edit)) => {
                        s.commit_edit(&mut self.document, edit.object, &edit.live)
                    }
                    _ => s.commit(&mut self.document),
                }
                .map_err(|e| e.to_string())
            });
        match result {
            Ok(n) => {
                self.edge_blend_adjustment = Default::default();
                self.edge_blend_edit = None;
                self.component_selection.clear();
                self.edge_blend_preview.clear();
                self.push_log(if prompt.toggles.trim_and_join {
                    format!("{} applied to {n} object(s)", prompt.name())
                } else {
                    format!("{} created {n} surface object(s)", prompt.name())
                });
            }
            Err(e) => {
                self.edge_fillet_prompt = Some(prompt);
                self.push_log(format!("Error: {e}"));
            }
        }
    }
}

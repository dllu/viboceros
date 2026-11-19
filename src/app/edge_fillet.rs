//! Shared original-edge picking and confirmation for native fillets, chamfers and G2 blends.
use super::*;
use viboceros_command::blend_edge::BlendOptions;
use viboceros_command::chamfer_edge::ChamferOptions;
use viboceros_command::{
    ComponentSelectionKind, EdgeBlendKind as Kind, EdgeBlendPlan, FilletOptions,
};

mod face_edges;
mod preview;
pub(super) use face_edges::FaceEdgeSelection;
pub(super) use preview::Preview;

pub(super) const FACE_EDGES_HINT: &str = "Select faces to add their sharp boundary edges. Enter keeps the picks. Esc discards these face picks.";

#[derive(Clone, Copy, Debug)]
pub(super) struct Prompt {
    radius: f64,
    kind: Kind,
    tolerance: Tolerance,
    awaiting_value: bool,
    preview: bool,
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
    pub(super) fn hint(self) -> &'static str {
        if self.awaiting_value {
            if matches!(self.kind, Kind::Chamfer(_)) {
                "Enter chamfer distance with optional units"
            } else {
                "Enter edge radius with optional units"
            }
        } else {
            match self.kind {
                Kind::Fillet(_) => {
                    "Select edges, or use FaceEdges. Set Radius=value, FromTwoPoints, Corner=Automatic|Ball or Preview."
                }
                Kind::Chamfer(_) => {
                    "Select edges, or use FaceEdges. Set Distance=value, FromTwoPoints, RailType or Preview."
                }
                Kind::Curvature(_) => {
                    "Select edges, or use FaceEdges. Set Radius=value, FromTwoPoints, RailType or Preview."
                }
            }
        }
    }
    fn options(self) -> String {
        let model = match self.kind {
            Kind::Fillet(corner) => format!("Radius={} Corner={}", self.radius, corner.token()),
            Kind::Chamfer(rail) | Kind::Curvature(rail) => format!(
                "{}={} RailType={}",
                self.parameter(),
                self.radius,
                rail.token()
            ),
        };
        let preview = if self.preview { "Yes" } else { "No" };
        format!("{model} Preview={preview}")
    }
}

fn preview_options<'a>(
    words: impl IntoIterator<Item = &'a str>,
    mut preview: bool,
) -> Result<(Vec<&'a str>, bool), &'static str> {
    let mut modeling = Vec::new();
    for word in words {
        let option = word.trim_start_matches('_');
        let (name, value) = option
            .split_once('=')
            .map_or((option, None), |(name, value)| {
                (name, Some(value.trim_start_matches('_')))
            });
        if !name.eq_ignore_ascii_case("Preview") {
            modeling.push(word);
        } else {
            preview = match value {
                None => !preview,
                Some(v) if v.eq_ignore_ascii_case("Yes") => true,
                Some(v) if v.eq_ignore_ascii_case("No") => false,
                _ => return Err("expected Preview=Yes or Preview=No"),
            };
        }
    }
    Ok((modeling, preview))
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
        let (mut words, preview) = match preview_options(words, false) {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        let face_edges = words.iter().position(|word| {
            word.trim_start_matches('_')
                .eq_ignore_ascii_case("FaceEdges")
        });
        if let Some(index) = face_edges {
            words.remove(index);
        }
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
        let picks = match self.component_selection.checked_picks(&self.document) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
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
        let prompt = Prompt {
            radius: descriptor.numbers[0].value,
            kind,
            tolerance: self.document.tolerance(),
            awaiting_value: false,
            preview,
        };
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.document.clear_selection();
        self.push_log(format!(
            "{}: select edges; {}; Enter applies, Esc cancels",
            prompt.name(),
            prompt.options()
        ));
        if face_edges.is_some() {
            self.begin_edge_blend_faces();
        }
        true
    }
    pub(super) fn try_continue_edge_fillet(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt else {
            return false;
        };
        let input = input.trim();
        let keyword = input.trim_start_matches('_');
        if keyword.eq_ignore_ascii_case("FaceEdges") {
            self.begin_edge_blend_faces();
            return true;
        }
        if input.is_empty() || keyword.eq_ignore_ascii_case("Enter") {
            if self.finish_edge_blend_faces(true) {
                return true;
            }
            self.finish_edge_fillet(true);
            return true;
        }
        if keyword.eq_ignore_ascii_case("Cancel") {
            if self.finish_edge_blend_faces(false) {
                return true;
            }
            self.finish_edge_fillet(false);
            return true;
        }
        if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("FromTwoPoints")
        {
            if !self.start_edge_blend_distance() {
                return false;
            }
            self.component_selection.clear_choices();
            return true;
        }
        if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case(prompt.parameter())
        {
            prompt.awaiting_value = true;
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
        let (modeling, preview) = match preview_options(input.split_whitespace(), prompt.preview) {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
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
                if !words.is_empty() {
                    prompt.awaiting_value = false;
                }
                prompt.preview = preview;
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
            self.push_log(format!(
                "{} {}. {}",
                prompt.name(),
                prompt.options(),
                if self.picking_edge_blend_faces() {
                    FACE_EDGES_HINT
                } else {
                    prompt.hint()
                }
            ));
        }
    }

    fn capture_edge_blend(&self, prompt: Prompt) -> Result<EdgeBlendPlan, String> {
        if prompt.tolerance != self.document.tolerance() {
            return Err("tolerance changed; select edges again".into());
        }
        let picks = self
            .component_selection
            .checked_picks(&self.document)?
            .into_iter()
            .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
            .map(|p| (p.object, p.index));
        EdgeBlendPlan::capture(&self.document, picks, prompt.radius, prompt.kind)
            .map_err(|error| error.to_string())
    }

    pub(super) fn finish_edge_fillet(&mut self, apply: bool) {
        let Some(prompt) = self.edge_fillet_prompt.take() else {
            return;
        };
        self.edge_blend_faces = None;
        self.command_input.clear();
        if !apply {
            self.component_selection.clear();
            self.edge_blend_preview.clear();
            self.push_log(format!("Cancelled {}", prompt.name()));
            return;
        }
        let result = self
            .capture_edge_blend(prompt)
            .and_then(|plan| self.edge_blend_preview.selection(plan))
            .and_then(|s| s.commit(&mut self.document).map_err(|e| e.to_string()));
        match result {
            Ok(n) => {
                self.component_selection.clear();
                self.edge_blend_preview.clear();
                self.push_log(format!("{} applied to {n} object(s)", prompt.name()));
            }
            Err(e) => {
                self.edge_fillet_prompt = Some(prompt);
                self.push_log(format!("Error: {e}"));
            }
        }
    }
}

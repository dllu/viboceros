//! Shared original-edge picking and confirmation for native fillets and chamfers.
use super::*;
use viboceros_command::chamfer_edge::{ChamferOptions, ChamferRailType};
use viboceros_command::{ComponentSelectionKind, EdgeBlendSelection};
#[derive(Clone, Copy, Debug)]
pub(super) struct Prompt {
    radius: f64,
    chamfer: Option<ChamferRailType>,
    tolerance: Tolerance,
    awaiting_value: bool,
}
impl Prompt {
    pub(super) fn name(self) -> &'static str {
        if self.chamfer.is_some() {
            "ChamferEdge"
        } else {
            "FilletEdge"
        }
    }
    fn parameter(self) -> &'static str {
        if self.chamfer.is_some() {
            "Distance"
        } else {
            "Radius"
        }
    }
    pub(super) fn hint(self) -> &'static str {
        if self.awaiting_value {
            if self.chamfer.is_some() {
                "Enter chamfer distance with optional units"
            } else {
                "Enter fillet radius with optional units"
            }
        } else if self.chamfer.is_some() {
            "Select edges; Distance=value / RailType=DistBetweenRails|RollingBall; Enter applies, Esc cancels"
        } else {
            "Select edges; Radius=value changes the radius; Enter applies, Esc cancels"
        }
    }
    fn options(self) -> String {
        match self.chamfer {
            Some(rail) => format!("Distance={} RailType={}", self.radius, rail.token()),
            None => format!("Radius={}", self.radius),
        }
    }
}
impl VibocerosApp {
    pub(super) fn try_start_edge_fillet(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(name) = words.next().map(|n| n.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let chamfer = name.eq_ignore_ascii_case("ChamferEdge");
        if !chamfer && !name.eq_ignore_ascii_case("FilletEdge") {
            return false;
        }
        let descriptor = match self.commands.component_selection_prompt(input) {
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
        let rail = if chamfer {
            let mut o = ChamferOptions::default();
            if let Err(e) = o.update(&words.collect::<Vec<_>>()) {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Some(o.rail)
        } else {
            None
        };
        self.cancel_interactive_command(false);
        let prompt = Prompt {
            radius: descriptor.numbers[0].value,
            chamfer: rail,
            tolerance: self.document.tolerance(),
            awaiting_value: false,
        };
        self.edge_fillet_prompt = Some(prompt);
        self.command_input.clear();
        self.document.clear_selection();
        self.push_log(format!(
            "{}: select edges; {}; Enter applies, Esc cancels",
            prompt.name(),
            prompt.options()
        ));
        true
    }
    pub(super) fn try_continue_edge_fillet(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.edge_fillet_prompt else {
            return false;
        };
        let input = input.trim();
        if input.is_empty() {
            self.finish_edge_fillet(true);
            return true;
        }
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.finish_edge_fillet(false);
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
        let mut words = Vec::new();
        for word in input.split_whitespace() {
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
        let result = if let Some(rail) = prompt.chamfer {
            let mut options = ChamferOptions {
                distance: prompt.radius,
                rail,
            };
            options.update(&words).map(|()| {
                prompt.radius = options.distance;
                prompt.chamfer = Some(options.rail);
            })
        } else {
            self.commands
                .component_selection_prompt(&format!("FilletEdge {}", words.join(" ")))
                .and_then(|p| {
                    p.ok_or(viboceros_command::CommandError::Usage(
                        "FilletEdge Radius=value",
                    ))
                })
                .map(|p| prompt.radius = p.numbers[0].value)
        };
        match result {
            Ok(()) => {
                prompt.awaiting_value = false;
                self.edge_fillet_prompt = Some(prompt);
                self.push_log(format!(
                    "{} {}; select edges or Enter applies",
                    prompt.name(),
                    prompt.options()
                ));
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        self.command_input.clear();
        true
    }
    pub(super) fn finish_edge_fillet(&mut self, apply: bool) {
        let Some(prompt) = self.edge_fillet_prompt.take() else {
            return;
        };
        self.command_input.clear();
        if !apply {
            self.component_selection.clear();
            self.push_log(format!("Cancelled {}", prompt.name()));
            return;
        }
        if prompt.tolerance != self.document.tolerance() {
            self.edge_fillet_prompt = Some(prompt);
            self.push_log("Error: tolerance changed; select edges again".into());
            return;
        }
        let result = self
            .component_selection
            .checked_picks(&self.document)
            .map_err(str::to_owned)
            .and_then(|picks| {
                let picks = picks
                    .into_iter()
                    .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
                    .map(|p| (p.object, p.index));
                match prompt.chamfer {
                    Some(rail) => EdgeBlendSelection::prepare_chamfer(
                        &self.document,
                        picks,
                        prompt.radius,
                        rail,
                    ),
                    None => EdgeBlendSelection::prepare(&self.document, picks, prompt.radius),
                }
                .map_err(|e| e.to_string())
            })
            .and_then(|s| s.commit(&mut self.document).map_err(|e| e.to_string()));
        match result {
            Ok(n) => {
                self.component_selection.clear();
                self.push_log(format!("{} applied to {n} object(s)", prompt.name()));
            }
            Err(e) => {
                self.edge_fillet_prompt = Some(prompt);
                self.push_log(format!("Error: {e}"));
            }
        }
    }
}

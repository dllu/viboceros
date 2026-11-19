//! Numeric input for the default size assigned as edges are picked.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SizeInput {
    Edit,
    All,
    Next,
}

#[derive(Clone, Copy)]
pub(super) enum NextSize {
    Ask,
    Value(f64),
}

pub(super) fn next_size_option(chamfer: bool) -> &'static str {
    if chamfer {
        "NextChamferDistance"
    } else {
        "NextRadius"
    }
}

fn parse_size(input: &str, document: &Document) -> Result<f64, String> {
    let size = viboceros_drafting::PointInput::parse_length_with_units(
        input.trim_start_matches('_'),
        document.units(),
    )
    .ok_or("Enter a positive size with optional units")?
    .map_err(|error| error.to_string())?;
    if size.is_finite() && size > 0. {
        Ok(size)
    } else {
        Err("Enter a positive size with optional units".into())
    }
}

pub(super) fn take_next_size(
    words: &mut Vec<&str>,
    name: &str,
    document: &Document,
) -> Result<Option<NextSize>, String> {
    let matches = |word: &str| {
        word.trim_start_matches('_')
            .split('=')
            .next()
            .is_some_and(|key| key.eq_ignore_ascii_case(name))
    };
    let Some(index) = words.iter().position(|word| matches(word)) else {
        return Ok(None);
    };
    let token = words.remove(index);
    if words.iter().any(|word| matches(word)) {
        return Err(format!("Enter {name} once"));
    }
    let value = if let Some((_, value)) = token.split_once('=') {
        Some(value)
    } else if words.get(index).is_some_and(|word| !word.contains('=')) {
        Some(words.remove(index))
    } else {
        None
    };
    value
        .map(|value| parse_size(value, document).map(NextSize::Value))
        .transpose()
        .map(|size| Some(size.unwrap_or(NextSize::Ask)))
}

impl VibocerosApp {
    pub(in crate::app) fn edge_blend_waiting_for_next_size(&self) -> bool {
        self.edge_fillet_prompt
            .is_some_and(|prompt| prompt.size_input == Some(SizeInput::Next))
    }

    pub(super) fn try_next_edge_blend_size(
        &mut self,
        mut prompt: Prompt,
        modeling: &mut Vec<&str>,
        toggles: Toggles,
    ) -> bool {
        let next = match take_next_size(modeling, prompt.next_option(), &self.document) {
            Ok(next) => next,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        if next.is_some() || prompt.size_input == Some(SizeInput::Next) && !modeling.is_empty() {
            let next = match (next, modeling.as_slice()) {
                (Some(next), []) => Ok(next),
                (None, [value]) => parse_size(value, &self.document).map(NextSize::Value),
                _ => Err(format!("Enter one value for {}", prompt.next_option())),
            };
            self.command_input.clear();
            if self.picking_edge_blend_faces() {
                self.push_log(
                    "Press Enter to finish picking faces before setting the next size".into(),
                );
            } else if prompt.adjusting {
                self.push_log(format!("Use SelectEdges to set {}", prompt.next_option()));
            } else {
                match next {
                    Ok(next) => {
                        prompt.size_input = match next {
                            NextSize::Ask => Some(SizeInput::Next),
                            NextSize::Value(size) => {
                                prompt.radius = size;
                                None
                            }
                        };
                        prompt.toggles = toggles;
                        self.edge_fillet_prompt = Some(prompt);
                        self.component_selection.clear_choices();
                        self.log_edge_blend_prompt();
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            return true;
        }
        false
    }
}

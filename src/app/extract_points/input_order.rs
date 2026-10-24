//! Keep whole picks and completed temporary references in one getter sequence.
use super::*;
use crate::app::uv_subcurve_input::CurveRange;
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
enum Input {
    Object(ObjectId),
    Range(usize),
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct InputOrder {
    inputs: Vec<Input>,
    wholes: BTreeSet<ObjectId>,
}

impl InputOrder {
    pub(in crate::app) fn sync(&mut self, selected: impl IntoIterator<Item = ObjectId>) {
        let selected = selected.into_iter().collect::<Vec<_>>();
        let present = selected.iter().copied().collect::<BTreeSet<_>>();
        if present == self.wholes {
            return;
        }
        // Adding portions must not rescan every earlier portion. Only a
        // removal needs to prune recorded whole picks from the sequence.
        if self.wholes.difference(&present).next().is_some() {
            self.inputs.retain(|input| match input {
                Input::Object(id) => present.contains(id),
                Input::Range(_) => true,
            });
            self.wholes.retain(|id| present.contains(id));
        }
        for id in selected {
            if self.wholes.insert(id) {
                self.inputs.push(Input::Object(id));
            }
        }
    }

    pub(in crate::app) fn push_range(
        &mut self,
        index: usize,
        selected: impl IntoIterator<Item = ObjectId>,
    ) {
        self.sync(selected);
        self.inputs.push(Input::Range(index));
    }

    pub(in crate::app) fn arguments(&self, ranges: &[CurveRange]) -> Result<String, String> {
        let mut result = String::new();
        let mut seen = vec![false; ranges.len()];
        for input in &self.inputs {
            match input {
                Input::Object(id) => result.push_str(&format!(" Object={id}")),
                Input::Range(index) => {
                    let range = ranges
                        .get(*index)
                        .filter(|_| !seen[*index])
                        .ok_or_else(|| "Temporary curve input order changed".to_owned())?;
                    seen[*index] = true;
                    result.push_str(&range.argument());
                }
            }
        }
        if seen.iter().any(|seen| !seen) {
            return Err("Temporary curve input order changed".into());
        }
        Ok(result)
    }
}

impl VibocerosApp {
    pub(in crate::app) fn sync_extract_point_input_order(&mut self) {
        if let Some(order) = self
            .intersection_prompt
            .as_mut()
            .and_then(|p| p.uv_subcurves.extraction_order.as_mut())
        {
            order.sync(self.document.selected_object_ids());
        }
    }
}

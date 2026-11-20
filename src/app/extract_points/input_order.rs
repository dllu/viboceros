//! Format whole picks and temporary references for ExtractPt.
use super::*;
use crate::app::curve_portion::OrderedInput;
use crate::app::uv_subcurve_input::CurveRange;

impl InputOrder {
    pub(in crate::app) fn arguments(&self, ranges: &[CurveRange]) -> Result<String, String> {
        let mut result = String::new();
        let mut seen = vec![false; ranges.len()];
        for input in self.ordered() {
            match input {
                OrderedInput::Object(id) => result.push_str(&format!(" Object={id}")),
                OrderedInput::Range(index) => {
                    let range = ranges
                        .get(index)
                        .filter(|_| !seen[index])
                        .ok_or_else(|| "Temporary curve input order changed".to_owned())?;
                    seen[index] = true;
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

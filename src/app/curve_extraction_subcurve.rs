//! Reuse the shared temporary-curve getter; no temporary document objects.
use super::*;
use crate::app::intersect_two_sets::TwoSetsPrompt;
use crate::app::uv_subcurve_input::{PendingSubcurve, SubcurveInputs};
use viboceros_command::extract_control_polygon::{self, Selection, Target};
impl VibocerosApp {
    pub(in crate::app) fn try_start_curve_extraction_subcurve(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtractControlPolygon")
                || n.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("ExtractPt")
        }) {
            return false;
        }
        if words
            .iter()
            .filter(|w| w.trim_start_matches('_').eq_ignore_ascii_case("SubCrv"))
            .count()
            != 1
        {
            return false;
        }
        let base = words
            .into_iter()
            .filter(|w| !w.trim_start_matches('_').eq_ignore_ascii_case("SubCrv"))
            .collect::<Vec<_>>()
            .join(" ");
        if !self
            .commands
            .object_selection_prompt(&base)
            .is_ok_and(|p| p.is_some())
        {
            return false;
        }
        self.component_selection.clear();
        self.begin_curve_extraction_subcurve(base);
        true
    }
    pub(in crate::app) fn promote_curve_extraction_subcurve(&mut self, input: &str) -> bool {
        if !input.trim_start_matches('_').eq_ignore_ascii_case("SubCrv")
            || !(self.picking_control_polygons() || self.picking_extract_points())
            || self.object_prompt.is_none()
        {
            return false;
        }
        let base = self
            .object_prompt
            .take()
            .unwrap()
            .description
            .command_line();
        self.begin_curve_extraction_subcurve(base);
        true
    }
    fn begin_curve_extraction_subcurve(&mut self, base: String) {
        let original_selection = self.document.selected_object_ids().collect();
        let components = std::mem::take(&mut self.component_selection);
        self.cancel_interactive_command(false);
        self.component_selection = components;
        self.intersection_prompt = Some(TwoSetsPrompt {
            curve_extraction: Some(base),
            curve_deviation: None,
            deviation: None,
            first: None,
            output_layer: "Current",
            original_selection,
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: SubcurveInputs {
                ranges: vec![],
                pending: Some(PendingSubcurve::default()),
            },
            projection: None,
        });
        self.command_input.clear();
        self.log_intersection_prompt();
    }
    pub(in crate::app) fn continue_curve_extraction_sources(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if self.try_select_prompt_points(input) {
            return true;
        }
        if self.try_continue_uv_subcurve(&prompt, input) {
            return true;
        }
        if input.is_empty() {
            let no_components = prompt.name() != "ExtractPt"
                || self
                    .extract_point_component_arguments()
                    .is_ok_and(|a| a.is_empty());
            if prompt.uv_subcurves.ranges.is_empty()
                && no_components
                && self.document.selected_control_points().next().is_none()
                && !self
                    .document
                    .selected_objects()
                    .any(|o| prompt.filter().accepts_object(o))
            {
                self.cancel_intersection_prompt(false);
                return true;
            }
            let result = (|| -> Result<_, String> {
                if prompt
                    .uv_subcurves
                    .ranges
                    .iter()
                    .any(|r| !r.current(&self.document))
                {
                    return Err(if prompt.name() == "ExtractPt" {
                        viboceros_command::CommandError::SubcurveInputStale
                    } else {
                        viboceros_command::CommandError::ControlPolygonStale
                    }
                    .to_string());
                }
                if prompt.name() == "ExtractPt" {
                    let args = self
                        .extract_point_component_arguments()
                        .map_err(str::to_owned)?;
                    let input = format!(
                        "{}{}{}",
                        prompt.curve_extraction.as_ref().unwrap(),
                        prompt.uv_subcurves.arguments(),
                        args
                    );
                    let context = viboceros_command::CommandContext {
                        construction_plane: self.viewports[self.active_viewport]
                            .construction_plane(),
                    };
                    return self
                        .commands
                        .execute_postselected(&mut self.document, &input, context)
                        .map_err(|e| e.to_string());
                }
                let mut targets = self.control_polygon_targets();
                targets.extend(
                    prompt
                        .uv_subcurves
                        .ranges
                        .iter()
                        .map(|r| Target::CurveRange(r.object, r.parameters)),
                );
                let base = prompt.curve_extraction.as_ref().unwrap();
                let parsed = extract_control_polygon::parse(
                    &base.split_whitespace().skip(1).collect::<Vec<_>>(),
                )
                .map_err(|e| e.to_string())?;
                Selection::prepare(&self.document, targets, parsed.options)
                    .and_then(|s| s.commit(&mut self.document))
                    .map_err(|e| e.to_string())
            })();
            match result {
                Ok(message) => {
                    self.intersection_prompt = None;
                    self.component_selection.clear();
                    self.push_log(message);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        } else if ["SelNone", "None"]
            .iter()
            .any(|n| input.trim_start_matches('_').eq_ignore_ascii_case(n))
        {
            self.document.clear_selection();
            self.component_selection.clear();
            prompt.uv_subcurves.ranges.clear();
            self.intersection_prompt = Some(prompt);
        } else if input.trim_start_matches('_').eq_ignore_ascii_case("SelAll") {
            let ids = self
                .document
                .selectable_objects()
                .filter(|o| prompt.filter().accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
        } else if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_interactive_command(true);
            return false;
        } else {
            let base = prompt.curve_extraction.as_ref().unwrap();
            let mut description = self
                .commands
                .object_selection_prompt(base)
                .unwrap()
                .unwrap();
            if description.update_options(input).is_ok() {
                prompt.curve_extraction = Some(description.command_line());
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            } else if let Ok(ids) = input
                .split(',')
                .map(str::parse::<ObjectId>)
                .collect::<Result<Vec<_>, _>>()
            {
                self.select_intersection_prompt_objects(ids, SelectionMode::Add);
            } else {
                self.push_log(prompt.hint().into());
            }
        }
        self.command_input.clear();
        true
    }
}

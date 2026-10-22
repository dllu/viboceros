//! Reuse the shared temporary-curve getter; no temporary document objects.
use super::*;
use crate::app::intersect_two_sets::TwoSetsPrompt;
use crate::app::uv_subcurve_input::{PendingSubcurve, SubcurveInputs};
impl VibocerosApp {
    pub(in crate::app) fn try_start_control_polygon_subcurve(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtractControlPolygon")
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
        if !extract_control_polygon::parse(&base.split_whitespace().skip(1).collect::<Vec<_>>())
            .is_ok_and(|p| p.targets.is_none())
        {
            return false;
        }
        self.component_selection.clear();
        self.begin_control_polygon_subcurve(base);
        true
    }
    pub(in crate::app) fn promote_control_polygon_subcurve(&mut self, input: &str) -> bool {
        if !input.trim_start_matches('_').eq_ignore_ascii_case("SubCrv")
            || !self.picking_control_polygons()
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
        self.begin_control_polygon_subcurve(base);
        true
    }
    fn begin_control_polygon_subcurve(&mut self, base: String) {
        let original_selection = self.document.selected_object_ids().collect();
        let components = std::mem::take(&mut self.component_selection);
        self.cancel_interactive_command(false);
        self.component_selection = components;
        self.intersection_prompt = Some(TwoSetsPrompt {
            control_polygon: Some(base),
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
    pub(in crate::app) fn continue_control_polygon_sources(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if self.try_continue_uv_subcurve(&prompt, input) {
            return true;
        }
        if input.is_empty() {
            let result = (|| -> Result<_, String> {
                if prompt.uv_subcurves.ranges.iter().any(|r| {
                    self.document
                        .object(r.object)
                        .is_none_or(|o| o.geometry_snapshot() != &r.source)
                }) {
                    return Err(viboceros_command::CommandError::ControlPolygonStale.to_string());
                }
                let mut targets = self.control_polygon_targets();
                targets.extend(
                    prompt
                        .uv_subcurves
                        .ranges
                        .iter()
                        .map(|r| Target::CurveRange(r.object, r.parameters)),
                );
                let base = prompt.control_polygon.as_ref().unwrap();
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
                .filter(|o| {
                    viboceros_command::ObjectSelectionFilter::ControlPolygons.accepts_object(o)
                })
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
            let base = prompt.control_polygon.as_ref().unwrap();
            let mut description = extract_control_polygon::parse(
                &base.split_whitespace().skip(1).collect::<Vec<_>>(),
            )
            .unwrap()
            .options
            .prompt();
            if description.update_options(input).is_ok() {
                prompt.control_polygon = Some(description.command_line());
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

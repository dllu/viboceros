//! Control-net getters share an atomic plan with typed temporary ranges.
use super::object_selection::ObjectPromptPhase;
use super::*;
#[cfg(test)]
#[path = "../../crates/viboceros-command/tests/support/control_polygon.rs"]
pub(super) mod native;
use viboceros_command::extract_control_polygon::{self, Target};

impl VibocerosApp {
    pub(super) fn replaying_control_polygon_selection(&self, input: &str) -> bool {
        let name = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches(['_', '-']);
        (name.eq_ignore_ascii_case("Undo")
            && self.document.undo_label() == Some("ExtractControlPolygon"))
            || (name.eq_ignore_ascii_case("Redo")
                && self.document.redo_label() == Some("ExtractControlPolygon"))
    }
    pub(super) fn picking_control_polygons(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.description.command == "ExtractControlPolygon"
                && p.phase == ObjectPromptPhase::Selecting
        }) || self.intersection_prompt.as_ref().is_some_and(|p| {
            p.curve_extraction.is_some()
                && p.name() == "ExtractControlPolygon"
                && p.uv_subcurves.pending.is_none()
        })
    }
    pub(super) fn control_polygon_targets(&self) -> Vec<Target> {
        self.document
            .selected_objects()
            .map(|o| Target::Whole(o.id()))
            .collect()
    }
    pub(super) fn try_start_control_polygon_components(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtractControlPolygon")
        }) {
            return false;
        }
        let Ok(parsed) = extract_control_polygon::parse(&words[1..]) else {
            return false;
        };
        if parsed.targets.is_some() {
            return false;
        }
        // Native GetObject ignores preselected components. Whole-object
        // preselection can complete while retaining unrelated component picks.
        if !self
            .document
            .selected_objects()
            .any(|o| viboceros_command::ObjectSelectionFilter::ControlPolygons.accepts_object(o))
        {
            self.component_selection.clear();
        }
        false
    }
}

#[cfg(test)]
mod tests;

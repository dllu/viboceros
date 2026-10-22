//! Standalone plane intersections share the object selection and plane getter.
use super::intersect_two_sets::TwoSetsPrompt;
use super::project::{PlaneOperation, ProjectOptions, ProjectSession, project_ids};
use super::*;
use viboceros_command::ObjectSelectionFilter;
impl VibocerosApp {
    pub(super) fn try_start_iplane_prompt(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            matches!(
                w.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "iplane" | "infiniteplane" | "ip"
            )
        }) || words.next().is_some()
        {
            return false;
        }
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let first = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Section.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        let ready = !first.is_empty();
        self.intersection_prompt = Some(TwoSetsPrompt {
            control_polygon: None,
            curve_deviation: None,
            deviation: None,
            first: ready.then_some(first),
            original_selection,
            output_layer: "Current",
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            projection: Some(ProjectSession {
                operation: PlaneOperation::IPlane,
                options: ProjectOptions::default(),
                custom_requested: false,
                finish_targets: None,
                planes: Vec::new(),
                plane_getter: None,
            }),
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        if ready {
            self.begin_iplane_getter();
        } else {
            self.log_intersection_prompt();
        }
        true
    }
    pub(super) fn finish_iplane(&mut self, definition: String) {
        let prompt = self.intersection_prompt.clone().unwrap();
        self.clear_project_plane_state();
        let command = format!(
            "IPlane Objects={} Plane={definition}",
            project_ids(prompt.first.as_ref().unwrap())
        );
        if !self.try_execute_command(&command) {
            self.intersection_prompt = Some(prompt);
            self.begin_iplane_getter();
        }
        self.command_input.clear();
    }
}

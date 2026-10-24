//! Cutter selection followed by incrementally accepted mesh-region picks.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn picking_mesh_trim_target(&self) -> bool {
        self.intersection_prompt.as_ref().is_some_and(|p| {
            p.first.is_some()
                && p.boolean.as_ref().is_some_and(|b| b.kind.is_mesh_trim())
                && p.plane_getter().is_none()
        })
    }
    pub(in crate::app) fn begin_mesh_trim_history(&mut self) {
        match self.document.begin_history_group("MeshTrim") {
            Ok(group) => {
                self.intersection_prompt
                    .as_mut()
                    .unwrap()
                    .boolean
                    .as_mut()
                    .unwrap()
                    .partition
                    .as_mut()
                    .unwrap()
                    .trim_history = Some(std::rc::Rc::new(std::cell::RefCell::new(group)))
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
    pub(in crate::app) fn continue_mesh_trim(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") || input.is_empty() && prompt.first.is_some() {
            self.intersection_prompt = None;
            self.document.clear_selection();
            self.clear_project_plane_state();
            self.command_input.clear();
            self.push_log("Finished MeshTrim".into());
            return true;
        }
        if prompt.plane_getter().is_some()
            || ["IP", "IPlane", "InfinitePlane"]
                .iter()
                .any(|v| word.eq_ignore_ascii_case(v))
        {
            return self.continue_project_plane(&prompt, input);
        }
        if word.eq_ignore_ascii_case("Undo") && prompt.first.is_some() {
            let group = prompt
                .boolean
                .as_ref()
                .unwrap()
                .partition
                .as_ref()
                .unwrap()
                .trim_history
                .clone()
                .unwrap();
            if let Err(error) = self.document.undo_history_group(&mut group.borrow_mut()) {
                self.push_log(format!("Error: {error}"));
            }
            self.command_input.clear();
            return true;
        }
        if input.is_empty() {
            let cutters = self
                .document
                .selected_objects()
                .filter(|o| {
                    viboceros_command::ObjectSelectionFilter::MeshSplitCutters.accepts_object(o)
                })
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if cutters.is_empty()
                && prompt
                    .boolean
                    .as_ref()
                    .unwrap()
                    .partition
                    .as_ref()
                    .unwrap()
                    .planes
                    .is_empty()
            {
                self.push_log("Select cutting objects or define an infinite plane".into());
            } else {
                prompt.first = Some(cutters);
                self.document.clear_selection();
                self.intersection_prompt = Some(prompt);
                self.begin_mesh_trim_history();
                self.log_intersection_prompt();
            }
        } else if prompt.first.is_none() {
            if word.eq_ignore_ascii_case("SelNone") {
                self.document.clear_selection();
            } else if word.eq_ignore_ascii_case("SelAll") {
                let ids = self
                    .document
                    .selectable_objects()
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                self.select_boolean_solids_objects(ids, SelectionMode::Add);
            } else {
                match input
                    .split(',')
                    .map(str::parse::<ObjectId>)
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(ids) => self.select_boolean_solids_objects(ids, SelectionMode::Add),
                    Err(_) => self.push_log("Select cutting objects or IP".into()),
                }
            }
        } else {
            self.push_log("Pick a mesh region in a viewport".into());
        }
        self.command_input.clear();
        true
    }
    pub(in crate::app) fn pick_mesh_trim_region(
        &mut self,
        id: ObjectId,
        point: Option<Point3>,
    ) -> bool {
        if !self.picking_mesh_trim_target() {
            return false;
        }
        let Some(point) = point else {
            return true;
        };
        let prompt = self.intersection_prompt.as_ref().unwrap();
        let options = prompt.boolean.as_ref().unwrap();
        let first = prompt.first.as_ref().unwrap();
        let mut command = options.command_line(true);
        if !first.is_empty() {
            command.push_str(&format!(
                " Cutters={}",
                first
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        command.push_str(&format!(" Target={id} Point={}", format_model_point(point)));
        let group = options
            .partition
            .as_ref()
            .unwrap()
            .trim_history
            .clone()
            .unwrap();
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let result = self.commands.execute_in_history_group(
            &mut self.document,
            &command,
            context,
            &mut group.borrow_mut(),
        );
        match result {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        };
        self.log_intersection_prompt();
        true
    }
}

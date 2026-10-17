//! Arc Extension keeps a captured source and adds only the final independent arc.
use super::*;
use viboceros_command::arc_extension_input::Input;
pub(super) struct Session {
    pub input: Input,
    pub pointer: Option<egui::Pos2>,
    plane: Frame3,
}
impl VibocerosApp {
    pub(super) fn picking_arc_extension_curve(&self) -> bool {
        self.arc_extension_session
            .as_ref()
            .is_some_and(|s| s.input.wants_curve())
    }
    pub(super) fn start_arc_extension(&mut self, from_prompt: bool) {
        let output = from_prompt.then_some(self.arc_deform_session).flatten();
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        self.cancel_interactive_command(false);
        self.arc_deform_session = output;
        self.arc_extension_session = Some(Session {
            input: Input::new(self.document.tolerance()),
            pointer: None,
            plane,
        });
        self.arc_extension_prompt();
    }
    pub(super) fn try_start_arc_extension(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        let [command, mode] = words.as_slice() else {
            return false;
        };
        if !["Arc", "A"].iter().any(|k| {
            command
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case(k)
        }) || !mode
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Extension")
        {
            return false;
        }
        self.start_arc_extension(false);
        true
    }
    fn arc_extension_prompt(&mut self) {
        let s = self.arc_extension_session.as_ref().unwrap();
        self.active_command = Some(InteractiveCommand::ArcExtension {
            anchor: s.input.anchor(),
        });
        self.drafting_plane =
            if s.input.wants_center() || s.input.sweep().is_some_and(|s| s.requests_length()) {
                None
            } else {
                s.input.sweep().and_then(|s| s.frame()).or(Some(s.plane))
            };
        self.push_log(format!("Arc Extension: {}; Esc cancels", s.input.prompt()));
    }
    pub(super) fn release_arc_extension_pick(&mut self) {
        if let Some(id) = self
            .arc_extension_session
            .as_ref()
            .and_then(|s| s.input.source_id())
        {
            let _ = self.document.select_object(id, SelectionMode::Remove);
        }
    }
    pub(super) fn validate_arc_extension_source(&mut self) {
        if self
            .arc_extension_session
            .as_ref()
            .is_some_and(|s| !s.input.current(&self.document))
        {
            self.release_arc_extension_pick();
            self.arc_extension_session.as_mut().unwrap().input =
                Input::new(self.document.tolerance());
            self.point_filter = None;
            self.point_constraint = None;
            self.component_selection.clear_choices();
            self.push_log("Arc Extension source changed or became unavailable; pick again".into());
            self.arc_extension_prompt();
        }
    }
    pub(super) fn pick_arc_extension_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        if !self.picking_arc_extension_curve() {
            return false;
        }
        self.validate_arc_extension_source();
        let result = self.arc_extension_session.as_mut().unwrap().input.curve(
            &self.document,
            id,
            edge,
            point,
        );
        self.component_selection.clear_choices();
        match result {
            Ok(()) => {
                self.last_point = self.arc_extension_session.as_ref().unwrap().input.anchor();
                let _ = self.document.select_object(id, SelectionMode::Replace);
                self.arc_extension_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn pick_arc_extension_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.arc_extension_session.is_none() {
            return false;
        }
        if self.picking_arc_extension_curve() {
            let point = id.and_then(|id| {
                let s = self.arc_extension_session.as_ref()?;
                self.viewports[self.active_viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    edge,
                    s.pointer?,
                    self.document.tolerance(),
                )
            });
            if let (Some(id), Some(p)) = (id, point) {
                self.pick_arc_extension_at(id, edge, p);
            } else {
                self.push_log("Arc Extension: pick a curve near an end".into());
            }
        }
        true
    }
    fn arc_extension_result(
        &mut self,
        result: Result<Option<viboceros_geometry::CircularArc3>, viboceros_command::CommandError>,
    ) -> bool {
        match result {
            Ok(Some(_)) => {
                let s = self.arc_extension_session.as_ref().unwrap();
                let cmd = s.input.command();
                self.finish_arc_command(&cmd, Some(s.plane))
            }
            Ok(None) => {
                self.arc_extension_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_arc_extension_point(&mut self, point: Point3) -> Option<bool> {
        self.arc_extension_session.as_ref()?;
        self.validate_arc_extension_source();
        let result = self
            .arc_extension_session
            .as_mut()
            .unwrap()
            .input
            .point(point);
        Some(self.arc_extension_result(result))
    }
    pub(super) fn try_continue_arc_extension(&mut self, text: &str) -> bool {
        if self.arc_extension_session.is_none() {
            return false;
        }
        self.validate_arc_extension_source();
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.is_empty()
            || ["Cancel", "Enter"]
                .iter()
                .any(|k| word.eq_ignore_ascii_case(k))
        {
            self.cancel_interactive_command(true);
        } else if word.eq_ignore_ascii_case("Length") || word.starts_with("Length=") {
            self.push_log("Arc Extension Center accepts an endpoint or angle".into());
        } else if self
            .arc_extension_session
            .as_mut()
            .unwrap()
            .input
            .option(word)
        {
            self.arc_extension_prompt();
        } else {
            let Ok(number) = text.trim().parse::<f64>() else {
                return false;
            };
            let mut input = self.arc_extension_session.as_ref().unwrap().input.clone();
            let result = input.number(number);
            if result.is_ok() {
                self.arc_extension_session.as_mut().unwrap().input = input;
            }
            self.arc_extension_result(result);
        }
        self.command_input.clear();
        true
    }
}

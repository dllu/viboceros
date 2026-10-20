//! Surface and two point getters with an immutable cached path preview.
use super::*;
use viboceros_command::{
    orient_surface::Target,
    short_path::{self as solver, Plan},
};
use viboceros_geometry::NurbsCurve;

pub(super) struct Session {
    target: Option<Target>,
    start: Option<Point3>,
    cursor: Option<Point3>,
    plan: Option<Plan>,
    group: viboceros_document::HistoryGroup,
}
impl Session {
    pub(super) fn cursor_surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let t = self.target.as_ref()?;
        Some((t.id, Some(t.face), true))
    }
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        self.plan
            .as_ref()
            .map_or(&[], |p| std::slice::from_ref(&p.path.curve))
    }
}
impl VibocerosApp {
    pub(super) fn start_short_path(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ShortPath")
        }) {
            return false;
        }
        let Ok(input) = solver::input(&words[1..]) else {
            return false;
        };
        if !input.points.is_empty() {
            return false;
        }
        let id = input
            .surface
            .or_else(|| solver::selected_target(&self.document));
        let target = match id
            .map(|id| Target::capture(&self.document, id, input.face))
            .transpose()
        {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("ShortPath") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.short_path_session = Some(Session {
            target,
            start: None,
            cursor: None,
            plan: None,
            group,
        });
        self.active_command = Some(InteractiveCommand::ShortPath { start: None });
        self.command_input.clear();
        self.short_path_prompt();
        true
    }
    fn short_path_prompt(&mut self) {
        if let Some(s) = &self.short_path_session {
            self.push_log(format!(
                "ShortPath: {}",
                if s.target.is_none() {
                    "Select a surface or face"
                } else if s.start.is_none() {
                    "Start of path on surface"
                } else {
                    "End of path on surface"
                }
            ));
        }
    }
    pub(super) fn picking_short_path_face(&self) -> bool {
        self.short_path_session
            .as_ref()
            .is_some_and(|s| s.target.is_none())
    }
    pub(super) fn pick_short_path_face(
        &mut self,
        id: Option<ObjectId>,
        face: Option<usize>,
    ) -> bool {
        if !self.picking_short_path_face() {
            return false;
        }
        if !self.validate_short_path() {
            return true;
        }
        let Some(id) = id else {
            return true;
        };
        match Target::capture(&self.document, id, face) {
            Ok(t) => {
                self.short_path_session.as_mut().unwrap().target = Some(t);
                self.component_selection.clear_choices();
                self.short_path_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn continue_short_path(&mut self, input: &str) -> bool {
        if self.short_path_session.is_none() {
            return false;
        }
        if input.is_empty() || input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(false);
            return true;
        }
        if !self.validate_short_path() {
            return true;
        }
        if self.picking_short_path_face() {
            if let Ok(id) = input.parse() {
                self.pick_short_path_face(Some(id), None);
            } else {
                self.short_path_prompt();
            }
            self.command_input.clear();
            return true;
        }
        false // Shared drafting point input handles coordinates and units.
    }
    pub(super) fn validate_short_path(&mut self) -> bool {
        let valid = self.short_path_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.target.as_ref().is_none_or(|t| t.current(&self.document))
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log("ShortPath source or history changed; select the surface again".into());
        }
        valid
    }
    fn prepare_short_path(&mut self, point: Point3) -> Result<(), viboceros_command::CommandError> {
        let s = self.short_path_session.as_mut().unwrap();
        if s.cursor == Some(point) && s.plan.is_some() {
            return Ok(());
        }
        s.cursor = Some(point);
        s.plan = None;
        let Some(start) = s.start else {
            return Ok(());
        };
        let Some(target) = &s.target else {
            return Ok(());
        };
        s.plan = Some(Plan::prepare(&self.document, target.clone(), start, point)?);
        Ok(())
    }
    pub(super) fn update_short_path_hover(&mut self, point: Point3) {
        if self.short_path_session.is_some() && self.validate_short_path() {
            let _ = self.prepare_short_path(point);
        }
    }
    pub(super) fn accept_short_path_point(&mut self, point: Point3) -> Option<bool> {
        self.short_path_session.as_ref()?;
        if !self.validate_short_path() {
            return Some(false);
        }
        let s = self.short_path_session.as_mut().unwrap();
        let Some(target) = &s.target else {
            return Some(false);
        };
        if s.start.is_none() {
            match target.location(point, true) {
                Ok(location) => {
                    s.start = Some(location.point);
                    self.active_command = Some(InteractiveCommand::ShortPath { start: s.start });
                    self.short_path_prompt();
                    return Some(true);
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            }
        }
        if let Err(e) = self.prepare_short_path(point) {
            self.push_log(format!("Error: {e}"));
            return Some(false);
        }
        let s = self.short_path_session.as_mut().unwrap();
        match s
            .plan
            .as_ref()
            .unwrap()
            .commit(&mut self.document, &mut s.group)
        {
            Ok(message) => {
                self.push_log(message);
                self.cancel_interactive_command(false);
                Some(true)
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                Some(false)
            }
        }
    }
    pub(super) fn finish_short_path(&mut self) -> bool {
        self.short_path_session
            .take()
            .is_some_and(|s| s.group.can_undo())
    }
}

//! Mesh-constrained polyline drafts with cached accepted sections.
use super::*;
use viboceros_command::polyline_on_mesh::{self as solver, Plan, Target};
use viboceros_geometry::{MeshProjectionIndex, NurbsCurve};
pub(super) struct Session {
    target: Option<Target>,
    points: Vec<Point3>,
    cursor: Option<Point3>,
    plan: Option<Plan>,
    accepted: Option<Plan>,
    closed: bool,
    committed: bool,
    group: viboceros_document::HistoryGroup,
}
impl Session {
    pub(super) fn index(&self) -> Option<&MeshProjectionIndex<'static>> {
        Some(&self.target.as_ref()?.index)
    }
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        self.plan
            .as_ref()
            .map_or(&[], |p| std::slice::from_ref(&p.preview))
    }
    pub(super) fn points(&self) -> &[Point3] {
        &self.points
    }
    pub(super) fn close_point(&self) -> Option<Point3> {
        (self.points.len() >= 3).then(|| self.points[0])
    }
}
impl VibocerosApp {
    pub(super) fn start_mesh_polyline(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("PolylineOnMesh")
        }) {
            return false;
        }
        let Ok(input) = solver::input(&words[1..]) else {
            return false;
        };
        if !input.points.is_empty() {
            return false;
        }
        let target = match input
            .mesh
            .or_else(|| solver::selected_target(&self.document))
            .map(|id| Target::capture(&self.document, id))
            .transpose()
        {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("PolylineOnMesh") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.mesh_polyline_session = Some(Session {
            target,
            points: Vec::new(),
            cursor: None,
            plan: None,
            accepted: None,
            closed: input.close,
            committed: false,
            group,
        });
        self.active_command = Some(InteractiveCommand::PolylineOnMesh { anchor: None });
        self.command_input.clear();
        self.mesh_polyline_prompt();
        true
    }
    fn mesh_polyline_prompt(&mut self) {
        if let Some(s) = &self.mesh_polyline_session {
            self.push_log(format!(
                "PolylineOnMesh: {}",
                if s.target.is_none() {
                    "Select a polygon mesh"
                } else if s.points.is_empty() {
                    "Start of polyline on mesh"
                } else {
                    "Next point; Enter finishes (Close Undo)"
                }
            ));
        }
    }
    pub(super) fn picking_mesh_polyline(&self) -> bool {
        self.mesh_polyline_session
            .as_ref()
            .is_some_and(|s| s.target.is_none())
    }
    pub(super) fn pick_mesh_polyline(&mut self, id: Option<ObjectId>) -> bool {
        if !self.picking_mesh_polyline() {
            return false;
        }
        if !self.validate_mesh_polyline() {
            return true;
        }
        if let Some(id) = id {
            match Target::capture(&self.document, id) {
                Ok(t) => {
                    self.mesh_polyline_session.as_mut().unwrap().target = Some(t);
                    self.component_selection.clear_choices();
                    self.mesh_polyline_prompt();
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        true
    }
    pub(super) fn validate_mesh_polyline(&mut self) -> bool {
        let valid = self.mesh_polyline_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.target.as_ref().is_none_or(|t| t.current(&self.document))
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log("Mesh or history changed; select the mesh again".into());
        }
        valid
    }
    fn prepare_mesh_polyline(
        &mut self,
        cursor: Option<Point3>,
    ) -> Result<(), viboceros_command::CommandError> {
        let s = self.mesh_polyline_session.as_mut().unwrap();
        if s.cursor == cursor && s.plan.is_some() {
            return Ok(());
        }
        s.cursor = cursor;
        s.plan = None;
        let Some(target) = &s.target else {
            return Ok(());
        };
        if cursor.is_none() {
            s.plan = s.accepted.clone();
            if s.plan.is_some() {
                return Ok(());
            }
        }
        if let Some(point) = cursor
            && s.points
                .last()
                .is_none_or(|p| !p.is_near(point, self.document.tolerance()))
        {
            if let Some(accepted) = &s.accepted {
                s.plan = Some(accepted.extended(&self.document, point)?);
                return Ok(());
            }
            if let Some(first) = s.points.first() {
                s.plan = Some(Plan::prepare(
                    &self.document,
                    target.clone(),
                    &[*first, point],
                    false,
                )?);
            }
            return Ok(());
        }
        if s.points.len() >= 2 {
            s.plan = Some(Plan::prepare(
                &self.document,
                target.clone(),
                &s.points,
                false,
            )?);
            if cursor.is_none() {
                s.accepted = s.plan.clone();
            }
        }
        Ok(())
    }
    pub(super) fn update_mesh_polyline_hover(&mut self, point: Point3) {
        if self.mesh_polyline_session.is_some() && self.validate_mesh_polyline() {
            let _ = self.prepare_mesh_polyline(Some(point));
        }
    }
    pub(super) fn accept_mesh_polyline_point(&mut self, raw: Point3) -> Option<bool> {
        self.mesh_polyline_session.as_ref()?;
        if !self.validate_mesh_polyline() {
            return Some(false);
        }
        let s = self.mesh_polyline_session.as_ref().unwrap();
        let Some(target) = s.target.as_ref() else {
            return Some(false);
        };
        let Some(point) = self.viewports[self.active_viewport].mesh_constraint_location(
            raw,
            &target.index,
            self.document.tolerance(),
        ) else {
            self.push_log("Point does not project onto the mesh".into());
            return Some(false);
        };
        if s.points
            .last()
            .is_some_and(|p| p.is_near(point, self.document.tolerance()))
        {
            return Some(false);
        }
        if s.points.len() >= solver::MAX_POINTS {
            self.push_log("Too many mesh polyline points".into());
            return Some(false);
        }
        let close = s.points.len() >= 3 && s.points[0].is_near(point, self.document.tolerance());
        let point = if close { s.points[0] } else { point };
        if !s.points.is_empty()
            && let Err(e) = self.prepare_mesh_polyline(Some(point))
        {
            self.push_log(format!("Error: {e}"));
            return Some(false);
        }
        let s = self.mesh_polyline_session.as_mut().unwrap();
        s.points.push(point);
        s.cursor = None;
        s.accepted = s.plan.clone();
        self.active_command = Some(InteractiveCommand::PolylineOnMesh {
            anchor: Some(point),
        });
        if close {
            self.commit_mesh_polyline(false);
        } else {
            self.mesh_polyline_prompt();
        }
        Some(true)
    }
    fn commit_mesh_polyline(&mut self, close: bool) -> bool {
        if !self.validate_mesh_polyline() {
            return false;
        }
        let s = self.mesh_polyline_session.as_mut().unwrap();
        if close || s.closed {
            if s.points.len() < 3 {
                self.push_log("Close requires three mesh locations".into());
                return false;
            }
            let first = s.points[0];
            if s.points.last() != Some(&first)
                && let Err(e) = self.prepare_mesh_polyline(Some(first))
            {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        } else if let Err(e) = self.prepare_mesh_polyline(None) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let s = self.mesh_polyline_session.as_mut().unwrap();
        let Some(plan) = &s.plan else {
            return false;
        };
        match plan.commit(&mut self.document, &mut s.group) {
            Ok(message) => {
                s.committed = true;
                self.push_log(message);
                self.cancel_interactive_command(false);
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn continue_mesh_polyline(&mut self, input: &str) -> bool {
        if self.mesh_polyline_session.is_none() {
            return false;
        }
        if !self.validate_mesh_polyline() {
            return true;
        }
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(false);
            return true;
        }
        if input.is_empty() {
            if self.mesh_polyline_session.as_ref().unwrap().points.len() < 2 {
                self.cancel_interactive_command(false);
            } else {
                self.commit_mesh_polyline(false);
            }
            self.command_input.clear();
            return true;
        }
        if self.picking_mesh_polyline() {
            if let Ok(id) = input.parse() {
                self.pick_mesh_polyline(Some(id));
            } else {
                self.mesh_polyline_prompt();
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            let s = self.mesh_polyline_session.as_mut().unwrap();
            s.points.pop();
            s.cursor = None;
            s.plan = None;
            s.accepted = None;
            self.active_command = Some(InteractiveCommand::PolylineOnMesh {
                anchor: s.points.last().copied(),
            });
            let _ = self.prepare_mesh_polyline(None);
            self.command_input.clear();
            self.mesh_polyline_prompt();
            return true;
        }
        if word.eq_ignore_ascii_case("Close") {
            self.commit_mesh_polyline(true);
            self.command_input.clear();
            return true;
        }
        false
    }
    pub(super) fn finish_mesh_polyline(&mut self) -> bool {
        let Some(s) = self.mesh_polyline_session.take() else {
            return false;
        };
        if !s.committed
            && let Some(target) = s.target
        {
            let _ = self
                .document
                .select_object(target.id, SelectionMode::Remove);
        }
        s.group.can_undo()
    }
}

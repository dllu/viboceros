//! Surface-constrained interpolation point drafts and cached previews.
use super::*;
use viboceros_command::{
    orient_surface::Target,
    surface_interpolation::{self as solver, Options, Plan},
};
use viboceros_geometry::NurbsCurve;
pub(super) struct Session {
    target: Option<Target>,
    points: Vec<Point3>,
    cursor: Option<Point3>,
    plan: Option<Plan>,
    closed: bool,
    closing: Vec<bool>,
    accepted: Option<Plan>,
    suspend_auto_close: bool,
    options: Options,
    group: viboceros_document::HistoryGroup,
}
impl Session {
    pub(super) fn close_point(&self) -> Option<Point3> {
        (self.options.auto_close && !self.suspend_auto_close && self.points.len() >= 3)
            .then(|| self.points[0])
    }
    pub(super) fn points(&self) -> &[Point3] {
        &self.points
    }
    pub(super) fn cursor_surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let t = self.target.as_ref()?;
        Some((t.id, Some(t.face), false))
    }
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        self.plan
            .as_ref()
            .map_or(&[], |p| std::slice::from_ref(&p.interpolation.curve))
    }
}
impl VibocerosApp {
    pub(super) fn start_surface_interpolation(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("InterpCrvOnSrf")
        }) {
            return false;
        }
        let Ok((points, reference, options)) =
            solver::input(&words[1..], self.surface_interpolation_defaults)
        else {
            return false;
        };
        if !points.is_empty() {
            return false;
        }
        let id = reference
            .surface
            .or_else(|| viboceros_command::short_path::selected_target(&self.document));
        let target = match id
            .map(|id| Target::capture(&self.document, id, reference.face))
            .transpose()
        {
            Ok(target) => target,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("InterpCrvOnSrf") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.surface_interpolation_session = Some(Session {
            target,
            points: Vec::new(),
            cursor: None,
            plan: None,
            closed: reference.close,
            closing: Vec::new(),
            accepted: None,
            suspend_auto_close: false,
            options,
            group,
        });
        self.active_command = Some(InteractiveCommand::InterpolateCurveOnSurface { anchor: None });
        self.command_input.clear();
        self.surface_interpolation_prompt();
        true
    }
    fn surface_interpolation_prompt(&mut self) {
        if let Some(s) = &self.surface_interpolation_session {
            let prompt = if s.target.is_none() {
                "Select a surface or face"
            } else if s.points.is_empty() {
                "Start of curve"
            } else {
                "Next point; Enter finishes (Close Undo AutoClose PersistentClose)"
            };
            self.push_log(format!("InterpCrvOnSrf: {prompt}"));
        }
    }
    pub(super) fn picking_surface_interpolation_face(&self) -> bool {
        self.surface_interpolation_session
            .as_ref()
            .is_some_and(|s| s.target.is_none())
    }
    pub(super) fn pick_surface_interpolation_face(
        &mut self,
        id: Option<ObjectId>,
        face: Option<usize>,
    ) -> bool {
        if !self.picking_surface_interpolation_face() {
            return false;
        }
        if !self.validate_surface_interpolation() {
            return true;
        }
        if let Some(id) = id {
            match Target::capture(&self.document, id, face) {
                Ok(t) => {
                    self.surface_interpolation_session.as_mut().unwrap().target = Some(t);
                    self.component_selection.clear_choices();
                    self.surface_interpolation_prompt();
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        true
    }
    pub(super) fn validate_surface_interpolation(&mut self) -> bool {
        let valid = self.surface_interpolation_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.target.as_ref().is_none_or(|t| t.current(&self.document))
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log(
                "Interpolation surface or history changed; select the surface again".into(),
            );
        }
        valid
    }
    fn prepare_surface_interpolation(
        &mut self,
        cursor: Option<Point3>,
    ) -> Result<(), viboceros_command::CommandError> {
        let s = self.surface_interpolation_session.as_mut().unwrap();
        if s.cursor == cursor && s.plan.is_some() {
            return Ok(());
        }
        if cursor.is_none() && s.accepted.is_some() {
            s.cursor = None;
            s.plan = s.accepted.clone();
            return Ok(());
        }
        s.cursor = cursor;
        s.plan = None;
        let Some(target) = &s.target else {
            return Ok(());
        };
        let mut points = s.points.clone();
        let mut close = s.closing.last().copied().unwrap_or(false);
        if let Some(p) = cursor
            && points
                .last()
                .is_none_or(|last| !last.is_near(p, self.document.tolerance()))
        {
            close = s.options.auto_close
                && !s.suspend_auto_close
                && points.len() >= 3
                && points[0]
                    .distance_to(p)
                    .is_ok_and(|d| d <= 1.490116119385e-8);
            points.push(if close { points[0] } else { p });
        }
        if points.len() < 2 {
            return Ok(());
        }
        s.plan = Some(Plan::prepare(
            &self.document,
            target.clone(),
            &points,
            s.closed || s.options.persistent_close || close,
        )?);
        if cursor.is_none() {
            s.accepted = s.plan.clone();
        }
        Ok(())
    }
    pub(super) fn update_surface_interpolation_hover(&mut self, point: Point3) {
        if self.surface_interpolation_session.is_some() && self.validate_surface_interpolation() {
            let _ = self.prepare_surface_interpolation(Some(point));
        }
    }
    pub(super) fn accept_surface_interpolation_point(&mut self, point: Point3) -> Option<bool> {
        self.surface_interpolation_session.as_ref()?;
        if !self.validate_surface_interpolation() {
            return Some(false);
        }
        let s = self.surface_interpolation_session.as_mut().unwrap();
        let raw = point;
        let Some(target) = &s.target else {
            return Some(false);
        };
        let point = match target.location(point, false) {
            Ok(l) => l.point,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        };
        if s.points
            .last()
            .is_some_and(|last| last.is_near(point, self.document.tolerance()))
        {
            return Some(false);
        }
        if s.points.len() >= MAX_SURFACE_INTERPOLATION_POINTS {
            self.push_log("Too many interpolation points".into());
            return Some(false);
        }
        let auto_close = s.options.auto_close
            && !s.suspend_auto_close
            && s.points.len() >= 3
            && s.points[0]
                .distance_to(point)
                .is_ok_and(|d| d <= 1.490116119385e-8);
        let cached = if s.cursor == Some(raw) {
            s.plan.take()
        } else {
            None
        };
        s.points.push(if auto_close { s.points[0] } else { point });
        s.closing.push(auto_close);
        s.cursor = None;
        s.plan = cached.clone();
        s.accepted = cached;
        self.active_command = Some(InteractiveCommand::InterpolateCurveOnSurface {
            anchor: s.points.last().copied(),
        });
        let _ = self.prepare_surface_interpolation(None);
        self.surface_interpolation_prompt();
        Some(true)
    }
    fn commit_surface_interpolation(&mut self) -> bool {
        if !self.validate_surface_interpolation() {
            return false;
        }
        if let Err(e) = self.prepare_surface_interpolation(None) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let s = self.surface_interpolation_session.as_mut().unwrap();
        let Some(plan) = &s.plan else {
            self.surface_interpolation_prompt();
            return false;
        };
        match plan.commit(&mut self.document, &mut s.group) {
            Ok(message) => {
                self.surface_interpolation_defaults = s.options;
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
    pub(super) fn continue_surface_interpolation(&mut self, input: &str) -> bool {
        if self.surface_interpolation_session.is_none() {
            return false;
        }
        if !self.validate_surface_interpolation() {
            return true;
        }
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(false);
            return true;
        }
        if input.is_empty() {
            if self
                .surface_interpolation_session
                .as_ref()
                .is_some_and(|s| s.points.len() < 2)
            {
                self.cancel_interactive_command(false);
                return true;
            }
            self.commit_surface_interpolation();
            self.command_input.clear();
            return true;
        }
        if self.picking_surface_interpolation_face() {
            if let Ok(id) = input.parse() {
                self.pick_surface_interpolation_face(Some(id), None);
            } else {
                self.surface_interpolation_prompt();
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            let s = self.surface_interpolation_session.as_mut().unwrap();
            s.points.pop();
            s.closing.pop();
            s.cursor = None;
            s.plan = None;
            s.accepted = None;
            s.closed = false;
            self.active_command = Some(InteractiveCommand::InterpolateCurveOnSurface {
                anchor: s.points.last().copied(),
            });
            let _ = self.prepare_surface_interpolation(None);
            self.command_input.clear();
            self.surface_interpolation_prompt();
            return true;
        }
        if word.eq_ignore_ascii_case("Close") {
            let s = self.surface_interpolation_session.as_mut().unwrap();
            s.closed = true;
            s.cursor = None;
            s.plan = None;
            s.accepted = None;
            self.commit_surface_interpolation();
            self.command_input.clear();
            return true;
        }
        if word.contains('=') {
            let s = self.surface_interpolation_session.as_mut().unwrap();
            match solver::input(&input.split_whitespace().collect::<Vec<_>>(), s.options) {
                Ok((points, refs, options))
                    if points.is_empty() && refs.surface.is_none() && refs.face.is_none() =>
                {
                    s.options = options;
                    s.closed |= refs.close;
                    s.cursor = None;
                    s.plan = None;
                    s.accepted = None;
                    self.surface_interpolation_defaults = options;
                    let _ = self.prepare_surface_interpolation(None);
                }
                _ => self.push_log(format!("Error: {}", solver::USAGE)),
            }
            self.command_input.clear();
            self.surface_interpolation_prompt();
            return true;
        }
        false
    }
    pub(super) fn suspend_surface_interpolation_close(&mut self, suspended: bool) {
        if let Some(s) = &mut self.surface_interpolation_session
            && s.suspend_auto_close != suspended
        {
            s.suspend_auto_close = suspended;
            s.cursor = None;
            s.plan = None;
        }
    }
    pub(super) fn finish_surface_interpolation(&mut self) -> bool {
        self.surface_interpolation_session
            .take()
            .is_some_and(|s| s.group.can_undo())
    }
}
const MAX_SURFACE_INTERPOLATION_POINTS: usize = solver::MAX_POINTS;

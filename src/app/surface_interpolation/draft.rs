//! Cached point drafts, plane/surface projection and atomic interpolation admission.
use super::*;
impl VibocerosApp {
    pub(super) fn prepare_surface_interpolation(
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
    pub(in crate::app) fn update_surface_interpolation_hover(&mut self, point: Point3) {
        if self.surface_interpolation_session.is_some() && self.validate_surface_interpolation() {
            let _ = self.prepare_surface_interpolation(Some(point));
        }
    }
    pub(in crate::app) fn accept_surface_interpolation_point(
        &mut self,
        point: Point3,
    ) -> Option<bool> {
        self.surface_interpolation_session.as_ref()?;
        if self.picking_surface_interpolation_plane() {
            return None;
        }
        if !self.validate_surface_interpolation() {
            return Some(false);
        }
        let s = self.surface_interpolation_session.as_mut().unwrap();
        let raw = point;
        let Some(target) = &s.target else {
            return Some(false);
        };
        let point = match target.point(point) {
            Ok(point) => point,
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
    pub(super) fn commit_surface_interpolation(&mut self) -> bool {
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
}

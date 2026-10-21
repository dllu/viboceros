//! Ellipse-owned base picking and a staged normal-axis ellipsoid measurement.
use super::*;
use viboceros_command::subd_ellipsoid::Input;
use viboceros_document::HistoryGroup;
use viboceros_geometry::{Ellipse3, NurbsCurve, Tolerance};
#[cfg(test)]
mod tests;
pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
    release: Vec<ObjectId>,
    preview: Vec<NurbsCurve>,
}
impl Session {
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }
    fn refresh(&mut self, point: Option<Point3>) {
        self.preview.clear();
        let Some(base) = self.input.base else {
            return;
        };
        let mut input = self.input;
        if let Some(p) = point {
            let _ = input.point(p);
        }
        let e = base.ellipse;
        if let Ok(c) = e.to_nurbs() {
            self.preview.push(c);
        }
        let Some(radius) = input.third_radius.filter(|r| *r > 0.) else {
            return;
        };
        let Ok(f) = input.frame() else {
            return;
        };
        for (first, rx) in [(f.x_axis(), e.radius_x()), (f.y_axis(), e.radius_y())] {
            if let Ok(c) = Ellipse3::try_new(
                f.origin(),
                rx,
                radius,
                first,
                f.z_axis(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .and_then(Ellipse3::to_nurbs)
            {
                self.preview.push(c);
            }
        }
    }
}
impl VibocerosApp {
    pub(super) fn valid_subd_ellipsoid(&mut self) -> bool {
        let valid = self.subd_ellipsoid_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && self.document.tolerance() == s.input.tolerance()
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log("SubDEllipsoid inputs or history changed; restart the command".into());
        }
        valid
    }
    pub(super) fn start_subd_ellipsoid(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDEllipsoid")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.subd_ellipsoid_preferences(),
            self.document.tolerance(),
        );
        for word in &words[1..] {
            if !matches!(input.option(word), Ok(true)) {
                return false;
            }
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("SubDEllipsoid") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.subd_ellipsoid_session = Some(Session {
            input,
            group,
            release: vec![],
            preview: vec![],
        });
        self.start_subd_ellipsoid_ellipse(input.getter);
        if let Some(prompt) = input.pending_prompt() {
            self.push_log(format!("SubDEllipsoid: {prompt}"));
        }
        true
    }
    pub(super) fn sync_subd_ellipsoid_base(
        &mut self,
        input: viboceros_command::ellipse_input::Input,
    ) {
        if let Some(s) = self.subd_ellipsoid_session.as_mut() {
            s.input.getter = input;
            s.input.preferences.mark_foci = input.options().mark_foci;
            self.commands
                .remember_subd_ellipsoid_preferences(s.input.preferences);
        }
    }
    pub(super) fn accept_subd_ellipsoid_base(
        &mut self,
        input: viboceros_command::ellipse_input::Input,
        release: Vec<ObjectId>,
    ) -> bool {
        if !self.valid_subd_ellipsoid() {
            return false;
        }
        let Some(s) = self.subd_ellipsoid_session.as_mut() else {
            return false;
        };
        s.input.getter = input;
        s.input.accept_base(input.definition_value().unwrap());
        s.release = release;
        self.advance_subd_ellipsoid(Ok(()))
    }
    fn subd_ellipsoid_prompt(&mut self) {
        let Some(s) = self.subd_ellipsoid_session.as_mut() else {
            return;
        };
        s.refresh(None);
        self.commands
            .remember_subd_ellipsoid_preferences(s.input.preferences);
        self.active_command = Some(InteractiveCommand::SubDEllipsoid {
            anchor: s
                .input
                .base
                .map(|b| b.ellipse.center())
                .or(s.input.getter.anchor()),
        });
        self.drafting_plane = s.input.mouse_plane();
        self.command_input.clear();
        self.push_log(
            "SubDEllipsoid: end of third axis; pick a point or enter a size; Esc cancels".into(),
        );
    }
    fn advance_subd_ellipsoid(
        &mut self,
        result: Result<(), viboceros_command::CommandError>,
    ) -> bool {
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let Some(s) = self.subd_ellipsoid_session.as_ref() else {
            return false;
        };
        if !s.input.complete() {
            self.subd_ellipsoid_prompt();
            return true;
        }
        let geometry = match s.input.geometry() {
            Ok(g) => g,
            Err(e) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.subd_ellipsoid_session.take().unwrap();
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match self
                .document
                .release_command_selection_on_history_replay(s.release.iter().copied())
                .and_then(|()| self.document.add_geometry(geometry))
            {
                Ok(_) => {
                    self.document.commit_group_transaction(&mut s.group)?;
                    Ok(())
                }
                Err(e) => {
                    self.document.rollback_transaction()?;
                    Err(e.into())
                }
            }
        })();
        self.active_command = None;
        self.drafting_plane = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        match result {
            Ok(()) => {
                self.commands
                    .remember_subd_ellipsoid_preferences(s.input.preferences);
                self.push_log("Created SubDEllipsoid".into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_subd_ellipsoid_point(&mut self, point: Point3) -> Option<bool> {
        self.subd_ellipsoid_session.as_ref()?;
        if !self.valid_subd_ellipsoid() {
            return Some(false);
        }
        if self.ellipse_session.is_some() {
            let s = self.subd_ellipsoid_session.as_mut().unwrap();
            s.input
                .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
            if s.input.pending_value() {
                self.push_log("SubDEllipsoid: enter the pending topology option first".into());
                return Some(false);
            }
            return None;
        }
        let result = self
            .subd_ellipsoid_session
            .as_mut()
            .unwrap()
            .input
            .point(point);
        Some(self.advance_subd_ellipsoid(result))
    }
    pub(super) fn continue_subd_ellipsoid(&mut self, text: &str) -> bool {
        if self.subd_ellipsoid_session.is_none() {
            return false;
        }
        if !self.valid_subd_ellipsoid() {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.subd_ellipsoid_session.as_mut().unwrap();
        if self.ellipse_session.is_some() {
            if word == "0"
                && !s.input.pending_value()
                && s.input.getter.slot() != viboceros_command::ellipse_input::Slot::SecondAxis
            {
                s.input
                    .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
                let point = s.input.zero_origin();
                self.accept_filtered_drafting_point(point, false);
                return true;
            }
            if s.input.pending_value()
                || Input::topology_option(word)
                || s.input.unsupported_option(word)
            {
                let result = s.input.option(if word.is_empty() { "Enter" } else { word });
                match result {
                    Ok(true) => {
                        self.commands
                            .remember_subd_ellipsoid_preferences(s.input.preferences);
                        self.command_input.clear();
                        let prompt = format!(
                            "SubDEllipsoid: AroundFaces={} VerticalFaces={} CapFaceStyle={}; pick the base ellipse",
                            s.input.preferences.around_faces,
                            s.input.preferences.vertical_faces,
                            if s.input.preferences.quad_caps {
                                "Quad"
                            } else {
                                "Tri"
                            }
                        );
                        let prompt = s
                            .input
                            .pending_prompt()
                            .map_or(prompt, |key| format!("SubDEllipsoid: {key}"));
                        self.push_log(prompt);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                    _ => (),
                }
                return true;
            }
            return false;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.cancel_interactive_command(true);
            return true;
        }
        match s.input.option(word) {
            Ok(true) => return true,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Ok(false) => (),
        }
        if let Some(value) =
            viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
        {
            match value {
                Ok(value) => {
                    let r = s.input.number(value);
                    self.advance_subd_ellipsoid(r);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        false
    }
    pub(super) fn update_subd_ellipsoid_hover(&mut self, point: Point3) {
        if let Some(s) = self.subd_ellipsoid_session.as_mut() {
            s.refresh(Some(point));
        }
    }
}

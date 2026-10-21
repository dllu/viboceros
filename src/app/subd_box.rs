//! SubDBox's base uses the shared rectangle/curve getter; height stays staged.
use super::*;
#[cfg(test)]
mod tests;
use viboceros_command::rectangle_input::RectangleDefinition;
use viboceros_command::subd_box as solver;
use viboceros_document::HistoryGroup;
use viboceros_geometry::{NurbsCurve, Polyline3, Tolerance};

pub(super) struct Session {
    pub(super) rectangle: rectangle::InputSession,
    pub(super) base: Option<RectangleDefinition>,
    preferences: solver::Preferences,
    value: Option<&'static str>,
    group: HistoryGroup,
    tolerance: Tolerance,
    preview: Vec<NurbsCurve>,
}
impl Session {
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }
    fn refresh(&mut self, cursor: Option<Point3>) {
        self.preview.clear();
        let Some(base) = self.base else { return };
        let mut points = base.corners.to_vec();
        points.push(points[0]);
        if let Ok(curve) =
            Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).and_then(|p| p.to_nurbs())
        {
            self.preview.push(curve);
        }
        if let Some(point) = cursor
            && let Ok(height) = solver::height_at(base, point)
            && let Ok(corners) = (solver::Definition { base, height }).corners()
        {
            let offset = if height > 0. { 4 } else { 0 };
            let mut points = corners[offset..offset + 4].to_vec();
            points.push(points[0]);
            if let Ok(curve) = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)
                .and_then(|p| p.to_nurbs())
            {
                self.preview.push(curve);
            }
            for i in 0..4 {
                if let Ok(curve) = Polyline3::try_new(
                    vec![corners[i], corners[i + 4]],
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .and_then(|p| p.to_nurbs())
                {
                    self.preview.push(curve);
                }
            }
        }
    }
}
impl VibocerosApp {
    fn valid_subd_box(&mut self) -> bool {
        let valid = self.subd_box_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
        });
        if !valid {
            self.subd_box_session = None;
            self.active_command = None;
            self.drafting_plane = None;
            self.push_log("SubDBox inputs or history changed; restart the command".into());
        }
        valid
    }
    pub(super) fn start_subd_box(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDBox")
        }) {
            return false;
        }
        let mut preferences = self.commands.subd_box_preferences();
        let mut rectangle =
            rectangle::InputSession::new(self.viewports[self.active_viewport].construction_plane());
        let mut i = 1;
        while i < words.len() {
            match preferences.option(&words[i..]) {
                Ok(Some(n)) => {
                    i += n;
                    continue;
                }
                Err(_) => return false,
                Ok(None) => (),
            }
            if !solver::rectangle_option(&mut rectangle.input, words[i]) {
                return false;
            }
            i += 1;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("SubDBox") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.subd_box_session = Some(Session {
            rectangle,
            preferences,
            base: None,
            value: None,
            group,
            tolerance: self.document.tolerance(),
            preview: vec![],
        });
        self.subd_box_prompt();
        true
    }
    pub(super) fn subd_box_prompt(&mut self) {
        let Some(s) = self.subd_box_session.as_mut() else {
            return;
        };
        self.commands.remember_subd_box_preferences(s.preferences);
        let anchor = s.base.map(|b| b.corners[0]).or(s.rectangle.input.anchor());
        let message = if let Some(value) = s.value {
            format!("SubDBox: {value}")
        } else if let Some(base) = s.base {
            format!(
                "SubDBox: height or point <{}>; Esc cancels",
                solver::default_height(base).unwrap_or(0.)
            )
        } else {
            format!(
                "SubDBox: {}; XCount={} YCount={} ZCount={}; Esc cancels",
                if s.rectangle.cursor_curve().is_some() {
                    "center on curve"
                } else {
                    s.rectangle.input.prompt()
                },
                s.preferences.counts[0],
                s.preferences.counts[1],
                s.preferences.counts[2]
            )
        };
        self.drafting_plane = if let Some(base) = s.base {
            solver::base_plane(base)
                .and_then(|p| {
                    Frame3::try_from_directions(
                        p.origin(),
                        p.x_axis().as_vector(),
                        p.z_axis().as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )
                })
                .ok()
        } else {
            s.rectangle.input.mouse_plane()
        };
        s.refresh(None);
        self.active_command = Some(InteractiveCommand::SubDBox { anchor });
        self.push_log(message);
        self.command_input.clear();
    }
    pub(super) fn accept_subd_box_base(&mut self, base: RectangleDefinition) -> bool {
        if !self.valid_subd_box() {
            return false;
        }
        if let Err(e) = solver::base_plane(base) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        self.subd_box_session.as_mut().unwrap().base = Some(base);
        self.point_filter = None;
        self.point_constraint = None;
        self.subd_box_prompt();
        true
    }
    fn finish_subd_box(&mut self, height: f64) -> bool {
        if !self.valid_subd_box() {
            return false;
        }
        let Some(s) = self.subd_box_session.as_ref() else {
            return false;
        };
        let Some(base) = s.base else { return false };
        let geometry = match (solver::Definition { base, height }).geometry(s.preferences) {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.subd_box_session.take().unwrap();
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match self.document.add_geometry(geometry) {
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
        self.command_input.clear();
        match result {
            Ok(()) => {
                self.push_log("Created SubD box".into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_subd_box_point(&mut self, point: Point3) -> Option<bool> {
        let base = self.subd_box_session.as_ref()?.base?;
        Some(match solver::height_at(base, point) {
            Ok(height) => self.finish_subd_box(height),
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
    pub(super) fn continue_subd_box(&mut self, input: &str) -> bool {
        if self.subd_box_session.is_none() {
            return false;
        }
        if !self.valid_subd_box() {
            return true;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.subd_box_session.as_mut().unwrap();
        if let Some(base) = s.base {
            let value = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
                Some(solver::default_height(base).map_err(|e| e.to_string()))
            } else {
                viboceros_drafting::PointInput::parse_length_with_units(
                    input,
                    self.document.units(),
                )
                .map(|r| r.map_err(|e| e.to_string()))
            };
            if let Some(value) = value {
                match value {
                    Ok(h) => {
                        self.finish_subd_box(h);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
                return true;
            }
            return false;
        }
        if ["Cube", "Diagonal", "EdgeMidpoint"]
            .iter()
            .any(|s| word.eq_ignore_ascii_case(s))
        {
            self.push_log("SubDBox: unsupported construction option".into());
            return true;
        }
        if s.rectangle.input.anchor().is_none() && s.rectangle.cursor_curve().is_none() {
            let args = input.split_whitespace().collect::<Vec<_>>();
            let option = s.value.map_or_else(
                || args.clone(),
                |name| std::iter::once(name).chain(args.iter().copied()).collect(),
            );
            if s.value.is_none()
                && ["XCount", "YCount", "ZCount"]
                    .iter()
                    .any(|name| word.eq_ignore_ascii_case(name))
            {
                s.value = ["XCount", "YCount", "ZCount"]
                    .into_iter()
                    .find(|name| word.eq_ignore_ascii_case(name));
                self.subd_box_prompt();
                return true;
            }
            match s.preferences.option(&option) {
                Ok(Some(n)) if n == option.len() => {
                    s.value = None;
                    self.subd_box_prompt();
                    return true;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                _ => (),
            }
            if word == "0" {
                let point = self.viewports[self.active_viewport]
                    .construction_plane()
                    .origin();
                self.accept_rectangle_point(point);
                return true;
            }
        }
        false
    }
    pub(super) fn update_subd_box_hover(&mut self, point: Point3) {
        if let Some(s) = self.subd_box_session.as_mut() {
            s.refresh(Some(point));
        }
    }
}

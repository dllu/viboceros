//! SubD primitives share rectangle input, staged geometry and grouped history.
use super::*;
#[cfg(test)]
pub(super) mod tests;
use viboceros_command::rectangle_input::RectangleDefinition;
use viboceros_command::subd_box as solver;
use viboceros_command::subd_plane as plane_solver;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Box,
    Plane,
}
impl Kind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Box => "SubDBox",
            Self::Plane => "SubDPlane",
        }
    }
}
use viboceros_document::HistoryGroup;
use viboceros_geometry::{NurbsCurve, Polyline3, Tolerance};

pub(super) struct Session {
    kind: Kind,
    pub(super) rectangle: rectangle::InputSession,
    pub(super) base: Option<RectangleDefinition>,
    preferences: solver::Preferences,
    value: Option<&'static str>,
    group: HistoryGroup,
    tolerance: Tolerance,
    preview: Vec<NurbsCurve>,
}
fn axes(kind: Kind) -> &'static [&'static str] {
    if kind == Kind::Box {
        &["XCount", "YCount", "ZCount"]
    } else {
        &["XCount", "YCount"]
    }
}
fn count_option(
    kind: Kind,
    preferences: &mut solver::Preferences,
    args: &[&str],
) -> Result<Option<usize>, viboceros_command::CommandError> {
    if kind == Kind::Box {
        return preferences.option(args);
    }
    let mut p = plane_solver::Preferences {
        counts: [preferences.counts[0], preferences.counts[1]],
    };
    let result = p.option(args)?;
    preferences.counts = [p.counts[0], p.counts[1], 1];
    Ok(result)
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
    fn valid_subd_primitive(&mut self) -> bool {
        let name = self
            .subd_primitive_session
            .as_ref()
            .map_or("SubD", |s| s.kind.name());
        let valid = self.subd_primitive_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
        });
        if !valid {
            self.subd_primitive_session = None;
            self.active_command = None;
            self.drafting_plane = None;
            self.push_log(format!(
                "{name} inputs or history changed; restart the command"
            ));
        }
        valid
    }
    pub(super) fn start_subd_primitive(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        let kind = match words
            .first()
            .map(|w| w.trim_start_matches(['_', '-']).to_ascii_lowercase())
            .as_deref()
        {
            Some("subdbox") => Kind::Box,
            Some("subdplane") => Kind::Plane,
            _ => return false,
        };
        let mut preferences = match kind {
            Kind::Box => self.commands.subd_box_preferences(),
            Kind::Plane => {
                let p = self.commands.subd_plane_preferences();
                solver::Preferences {
                    counts: [p.counts[0], p.counts[1], 1],
                }
            }
        };
        let mut rectangle =
            rectangle::InputSession::new(self.viewports[self.active_viewport].construction_plane());
        let mut i = 1;
        while i < words.len() {
            match count_option(kind, &mut preferences, &words[i..]) {
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
        let group = match self.document.begin_history_group(kind.name()) {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.subd_primitive_session = Some(Session {
            kind,
            rectangle,
            preferences,
            base: None,
            value: None,
            group,
            tolerance: self.document.tolerance(),
            preview: vec![],
        });
        self.subd_primitive_prompt();
        true
    }
    pub(super) fn subd_primitive_prompt(&mut self) {
        let Some(s) = self.subd_primitive_session.as_mut() else {
            return;
        };
        match s.kind {
            Kind::Box => {
                self.commands.remember_subd_box_preferences(s.preferences);
            }
            Kind::Plane => {
                self.commands
                    .remember_subd_plane_preferences(plane_solver::Preferences {
                        counts: [s.preferences.counts[0], s.preferences.counts[1]],
                    });
            }
        }
        let name = s.kind.name();
        let anchor = s.base.map(|b| b.corners[0]).or(s.rectangle.input.anchor());
        let message = if let Some(value) = s.value {
            format!("{name}: {value}")
        } else if let Some(base) = s.base {
            format!(
                "{name}: height or point <{}>; Esc cancels",
                solver::default_height(base).unwrap_or(0.)
            )
        } else {
            format!(
                "{name}: {}; XCount={} YCount={}{}; Esc cancels",
                if s.rectangle.cursor_curve().is_some() {
                    "center on curve"
                } else {
                    s.rectangle.input.prompt()
                },
                s.preferences.counts[0],
                s.preferences.counts[1],
                if s.kind == Kind::Box {
                    format!(" ZCount={}", s.preferences.counts[2])
                } else {
                    String::new()
                }
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
        self.active_command = Some(InteractiveCommand::SubDPrimitive {
            kind: s.kind,
            anchor,
        });
        self.push_log(message);
        self.command_input.clear();
    }
    pub(super) fn accept_subd_primitive_base(&mut self, base: RectangleDefinition) -> bool {
        if !self.valid_subd_primitive() {
            return false;
        }
        if let Err(e) = solver::base_plane(base) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        self.subd_primitive_session.as_mut().unwrap().base = Some(base);
        self.point_filter = None;
        self.point_constraint = None;
        if self.subd_primitive_session.as_ref().unwrap().kind == Kind::Plane {
            return self.finish_subd_primitive(0.);
        }
        self.subd_primitive_prompt();
        true
    }
    fn finish_subd_primitive(&mut self, height: f64) -> bool {
        if !self.valid_subd_primitive() {
            return false;
        }
        let Some(s) = self.subd_primitive_session.as_ref() else {
            return false;
        };
        let Some(base) = s.base else { return false };
        let geometry = match match s.kind {
            Kind::Box => (solver::Definition { base, height }).geometry(s.preferences),
            Kind::Plane => plane_solver::geometry(
                base,
                plane_solver::Preferences {
                    counts: [s.preferences.counts[0], s.preferences.counts[1]],
                },
            ),
        } {
            Ok(g) => g,
            Err(e) => {
                if s.kind == Kind::Plane {
                    self.subd_primitive_session = None;
                    self.active_command = None;
                    self.drafting_plane = None;
                }
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let mut s = self.subd_primitive_session.take().unwrap();
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
                self.push_log(format!("Created {}", s.kind.name()));
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn accept_subd_primitive_point(&mut self, point: Point3) -> Option<bool> {
        let base = self.subd_primitive_session.as_ref()?.base?;
        Some(match solver::height_at(base, point) {
            Ok(height) => self.finish_subd_primitive(height),
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
    pub(super) fn continue_subd_primitive(&mut self, input: &str) -> bool {
        if self.subd_primitive_session.is_none() {
            return false;
        }
        if !self.valid_subd_primitive() {
            return true;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.subd_primitive_session.as_mut().unwrap();
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
                        self.finish_subd_primitive(h);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
                return true;
            }
            return false;
        }
        if s.kind == Kind::Plane
            && word
                .split('=')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("ZCount"))
        {
            self.push_log("SubDPlane uses XCount and YCount".into());
            return true;
        }
        if ["Cube", "Diagonal", "EdgeMidpoint"]
            .iter()
            .any(|s| word.eq_ignore_ascii_case(s))
        {
            let message = format!("{}: unsupported construction option", s.kind.name());
            self.push_log(message);
            return true;
        }
        if s.rectangle.input.anchor().is_none() && s.rectangle.cursor_curve().is_none() {
            let args = input.split_whitespace().collect::<Vec<_>>();
            let option = s.value.map_or_else(
                || args.clone(),
                |name| std::iter::once(name).chain(args.iter().copied()).collect(),
            );
            if s.value.is_none()
                && axes(s.kind)
                    .iter()
                    .any(|name| word.eq_ignore_ascii_case(name))
            {
                s.value = axes(s.kind)
                    .iter()
                    .copied()
                    .find(|name| word.eq_ignore_ascii_case(name));
                self.subd_primitive_prompt();
                return true;
            }
            match count_option(s.kind, &mut s.preferences, &option) {
                Ok(Some(n)) if n == option.len() => {
                    s.value = None;
                    self.subd_primitive_prompt();
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
    pub(super) fn update_subd_primitive_hover(&mut self, point: Point3) {
        if let Some(s) = self.subd_primitive_session.as_mut() {
            s.refresh(Some(point));
        }
    }
}

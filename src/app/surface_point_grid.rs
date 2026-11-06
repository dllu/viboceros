//! Ordered grid drafts stay outside document history until the final point.
use super::*;
use viboceros_command::point_grid::{self as solver, Options};
mod parameters;

pub(super) struct Session {
    control: bool,
    options: Options,
    preferences: solver::Preferences,
    count: Vec<usize>,
    points: Vec<Point3>,
    pending: Option<&'static str>,
    prior_last_point: Option<Point3>,
    group: viboceros_document::HistoryGroup,
    tolerance: Tolerance,
}
impl Session {
    pub(super) fn preview(&self) -> Option<(&[Point3], usize)> {
        self.wants_point()
            .then(|| (self.points.as_slice(), self.count[1]))
    }
    fn name(&self) -> &'static str {
        if self.control {
            "SrfControlPtGrid"
        } else {
            "SrfPtGrid"
        }
    }
    fn wants_point(&self) -> bool {
        self.count.len() == 2 && self.pending.is_none()
    }
}
impl VibocerosApp {
    pub(super) fn start_surface_point_grid(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        let Some(name) = words.first() else {
            return false;
        };
        let name = name.trim_start_matches(['_', '-']);
        let control = if name.eq_ignore_ascii_case("SrfControlPtGrid") {
            true
        } else if name.eq_ignore_ascii_case("SrfPtGrid") {
            false
        } else {
            return false;
        };
        let preferences = self.commands.surface_point_grid_preferences(control);
        let group = match self.document.begin_history_group(if control {
            "SrfControlPtGrid"
        } else {
            "SrfPtGrid"
        }) {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let mut session = Session {
            control,
            options: Options {
                degree: preferences.degree,
                ..Options::default()
            },
            preferences,
            count: Vec::new(),
            points: Vec::new(),
            pending: None,
            prior_last_point: self.last_point,
            group,
            tolerance: self.document.tolerance(),
        };
        // Complete coordinate scripts retain the ordinary command route.
        match session.parameters(&words[1..]) {
            Ok(n) if n == words.len() - 1 => {}
            _ => return false,
        }
        self.cancel_interactive_command(false);
        session.group = match self.document.begin_history_group(session.name()) {
            Ok(group) => group,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.surface_point_grid_session = Some(session);
        self.surface_point_grid_prompt();
        true
    }
    fn valid_surface_point_grid(&mut self) -> bool {
        let valid = self.surface_point_grid_session.as_ref().is_none_or(|s| {
            s.tolerance == self.document.tolerance()
                && self.document.history_group_is_current(&s.group)
        });
        if !valid {
            self.cancel_interactive_command(false);
            self.push_log("Point-grid document or tolerance changed; restart the command".into());
        }
        valid
    }
    pub(super) fn surface_point_grid_wants_point(&self) -> bool {
        self.surface_point_grid_session
            .as_ref()
            .is_none_or(Session::wants_point)
    }
    fn surface_point_grid_prompt(&mut self) {
        let Some(s) = &self.surface_point_grid_session else {
            return;
        };
        self.commands
            .remember_surface_point_grid_preferences(s.control, s.preferences);
        let message = if let Some(name) = s.pending {
            format!("{}: {name} (1–11); Esc cancels", s.name())
        } else if s.count.len() < 2 {
            let axis = s.count.len();
            format!(
                "{}: number of points in {} <{}> (degree {}, {}KeepPoints={}); Esc cancels",
                s.name(),
                ["U", "V"][axis],
                s.preferences.counts[axis],
                s.options.degree[axis],
                if s.control {
                    String::new()
                } else {
                    format!("Closed{}={} ", ["U", "V"][axis], s.options.closed[axis])
                },
                s.options.keep_points
            )
        } else {
            format!(
                "{}: point ({}, {}) of {} × {}; Undo removes the last point; Esc cancels",
                s.name(),
                s.points.len() / s.count[1] + 1,
                s.points.len() % s.count[1] + 1,
                s.count[0],
                s.count[1]
            )
        };
        self.active_command = Some(InteractiveCommand::SurfacePointGrid {
            control: s.control,
            anchor: s.points.last().copied(),
        });
        self.command_input.clear();
        self.push_log(message);
    }
    pub(super) fn continue_surface_point_grid(&mut self, text: &str) -> bool {
        if self.surface_point_grid_session.is_none() {
            return false;
        }
        if !self.valid_surface_point_grid() {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        let s = self.surface_point_grid_session.as_mut().unwrap();
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.pending.take().is_some() {
                self.surface_point_grid_prompt();
            } else if !s.wants_point() {
                let count = s.preferences.counts[s.count.len()].to_string();
                if let Err(e) = s.parameters(&[&count]) {
                    self.push_log(format!("Error: {e}"));
                } else {
                    self.surface_point_grid_prompt();
                }
            } else {
                self.cancel_interactive_command(true);
            }
            return true;
        }

        if word.eq_ignore_ascii_case("Undo") {
            if s.wants_point() {
                s.points.pop();
                self.last_point = s.points.last().copied().or(s.prior_last_point);
            }
            self.point_filter = None;
            self.point_constraint = None;
            self.surface_point_grid_prompt();
            return true;
        }
        if s.wants_point()
            || self
                .commands
                .recognizes(text.split_whitespace().next().unwrap())
        {
            return false;
        }
        let words = text.split_whitespace().collect::<Vec<_>>();
        let result = s.parameters(&words);
        self.commands
            .remember_surface_point_grid_preferences(s.control, s.preferences);
        match result {
            Ok(n) if n == words.len() => self.surface_point_grid_prompt(),
            Ok(_) => {
                self.push_log("Error: enter point-grid counts before point coordinates".into())
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn accept_surface_point_grid(&mut self, point: Point3) -> bool {
        if !self.valid_surface_point_grid() {
            return false;
        }
        let Some(s) = self.surface_point_grid_session.as_mut() else {
            return false;
        };
        if !s.wants_point() {
            return false;
        }
        s.points.push(point);
        if s.points.len() < s.count[0] * s.count[1] {
            self.surface_point_grid_prompt();
            return true;
        }
        let mut s = self.surface_point_grid_session.take().unwrap();
        let count = [s.count[0], s.count[1]];
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            let plan = solver::construct(
                s.control,
                count,
                s.options,
                std::mem::take(&mut s.points),
                s.tolerance,
            )?;
            self.document.begin_group_transaction(&s.group)?;
            if let Err(e) = plan.insert(&mut self.document) {
                self.document.rollback_transaction()?;
                return Err(e);
            }
            self.document.commit_group_transaction(&mut s.group)?;
            Ok(())
        })();
        self.active_command = None;
        self.drafting_plane = None;
        self.point_constraint = None;
        self.point_filter = None;
        self.command_input.clear();
        match result {
            Ok(()) => {
                self.push_log(format!(
                    "Created {} by {} point-grid surface",
                    count[0], count[1]
                ));
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
}

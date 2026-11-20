use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum End {
    Start,
    Finish,
}

impl End {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("StartPoint") {
            Some(Self::Start)
        } else if name.eq_ignore_ascii_case("EndPoint") {
            Some(Self::Finish)
        } else {
            None
        }
    }

    pub(super) fn prompt(self) -> &'static str {
        match self {
            Self::Start => "Pick the start point where both rails meet. Esc cancels",
            Self::Finish => "Pick the end point where both rails meet. Esc cancels",
        }
    }

    pub(super) fn set(self, ends: &mut SweepEndPoints, point: Point3) {
        match self {
            Self::Start => ends.start = Some(point),
            Self::Finish => ends.end = Some(point),
        }
    }
}

impl VibocerosApp {
    pub(in crate::app) fn continue_sweep_two_rail_point_option(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(session) = self.sweep_two_rails.as_ref().filter(|session| {
            session.picking.is_none() && session.value.is_none() && session.slash_picker.is_none()
        }) else {
            return false;
        };
        let name = input.trim_start_matches(['_', '-']);
        let end = if name.eq_ignore_ascii_case("Point") {
            if session.section_count() == 0 && session.state.shape.ends.start.is_none() {
                End::Start
            } else {
                End::Finish
            }
        } else if let Some(end) = End::from_name(name) {
            end
        } else {
            return false;
        };
        self.command_input.clear();
        if session.state.rails.iter().any(Rail::is_closed) {
            self.push_log("Sweep2 point ends require open rails".into());
            return true;
        }
        if session.rail_count() < 2 || (end == End::Finish && session.section_count() == 0) {
            self.push_log(
                "Select both Sweep2 rails before a point end and a profile before an end point"
                    .into(),
            );
            return true;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        session.picking = Some(end);
        session.preview.clear_wires();
        self.active_command = Some(InteractiveCommand::SweepTwoRailPoint);
        self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        self.log_sweep_two_rails();
        true
    }

    pub(in crate::app) fn finish_sweep_two_rail_point_getter(&mut self) {
        if self.active_command == Some(InteractiveCommand::SweepTwoRailPoint) {
            self.active_command = None;
            self.drafting_plane = None;
            self.snaps.model_override = None;
            self.point_filter = None;
            self.point_constraint = None;
        }
    }

    pub(in crate::app) fn accept_sweep_two_rail_point(&mut self, point: Point3) -> Option<bool> {
        let end = self.sweep_two_rails.as_ref()?.picking?;
        Some(self.apply_sweep_two_rail_point(end, point))
    }

    pub(super) fn set_sweep_two_rail_point_argument(&mut self, end: End, argument: &str) {
        match Options::parse(&[argument]) {
            Ok(options) => {
                let ends = options.shape.ends;
                let point = ends.start.or(ends.end).expect("parsed Sweep2 point");
                self.apply_sweep_two_rail_point(end, point);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn apply_sweep_two_rail_point(&mut self, end: End, point: Point3) -> bool {
        if !self.validate_sweep_two_rails() {
            return false;
        }
        let session = self.sweep_two_rails.as_ref().unwrap();
        let mut ends = session.state.shape.ends;
        end.set(&mut ends, point);
        if session.section_count() + point_count(ends) > MAX_SECTIONS {
            self.push_log(format!(
                "Sweep2 accepts at most {MAX_SECTIONS} sections including point ends"
            ));
            return false;
        }
        if end == End::Finish && session.section_count() == 0 {
            self.push_log("Select a Sweep2 cross section before the end point".into());
            return false;
        }
        let tolerance = self.document.tolerance();
        let result = session
            .state
            .capture(&self.document)
            .and_then(|source| source.validate_point_ends(ends, tolerance));
        if let Err(error) = result {
            self.push_log(format!("Sweep2: {error}"));
            return false;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        session.undo.push(session.state.clone());
        session.state.shape.ends = ends;
        session.state.review |= end == End::Finish;
        session.picking = None;
        session
            .preview
            .accept_provisional(preview::Provisional::End(end, point), tolerance);
        self.finish_sweep_two_rail_point_getter();
        self.log_sweep_two_rails();
        true
    }
}

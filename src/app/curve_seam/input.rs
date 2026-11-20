//! Command line getters for selection and seam options.
use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(in crate::app) fn continue_curve_seam(&mut self, input: &str) -> bool {
        if self.curve_seam_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.command_input.clear();
        self.validate_curve_seams();
        let Some(session) = self.curve_seam_session.as_ref() else {
            return true;
        };
        let command = input.trim().trim_start_matches('_');
        if command == "!" || command.eq_ignore_ascii_case("Cancel") {
            self.cancel_curve_seam(true);
            return true;
        }
        let (option, value) = command
            .split_once(|c: char| c.is_whitespace() || c == '=')
            .map_or((command, ""), |(option, value)| (option, value.trim()));
        if command.is_empty() || command.eq_ignore_ascii_case("Enter") {
            if session.id_pending {
                self.curve_seam_session.as_mut().unwrap().id_pending = false;
                self.log_curve_seam();
            } else if session.selecting() {
                self.begin_curve_seam_adjustment();
            } else {
                self.finish_curve_seam();
            }
        } else if option.eq_ignore_ascii_case("SelID") || session.id_pending {
            self.curve_seam_input_id(if session.id_pending { command } else { value });
        } else if session.selecting() {
            if command.eq_ignore_ascii_case("SelAll") {
                let ids = self
                    .document
                    .selectable_objects()
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                self.select_curve_seams(ids, SelectionMode::Add);
            } else if command.eq_ignore_ascii_case("SelNone") {
                self.restore_curve_selection(Vec::new());
            } else {
                self.log_curve_seam();
            }
        } else {
            self.curve_seam_option(command, option, value);
        }
        true
    }

    fn curve_seam_input_id(&mut self, value: &str) {
        let session = self.curve_seam_session.as_mut().expect("CrvSeam session");
        if value.is_empty() {
            session.id_pending = true;
            self.log_curve_seam();
            return;
        }
        session.id_pending = false;
        if let Ok(id) = value.parse::<ObjectId>() {
            if session.selecting() && self.seam_eligible(id) {
                self.select_curve_seams([id], SelectionMode::Add);
                return;
            }
            let session = self.curve_seam_session.as_mut().expect("CrvSeam session");
            if let Some(index) = session
                .seams
                .iter()
                .position(|seam| seam.source.object_id() == id)
            {
                if session.flipping {
                    self.apply_curve_seam_event(self.active_viewport, Event::Flip(index));
                } else {
                    session.target = Some(index);
                    self.push_log("Enter a seam point or Parameter=value".into());
                }
                return;
            }
        }
        self.push_log("Enter an eligible closed curve object ID".into());
    }

    fn curve_seam_option(&mut self, command: &str, option: &str, value: &str) {
        let session = self.curve_seam_session.as_mut().expect("CrvSeam session");
        if session.flipping {
            self.log_curve_seam();
            return;
        }
        if command.eq_ignore_ascii_case("Flip") {
            session.flipping = true;
        } else if command.eq_ignore_ascii_case("Automatic") {
            session.automatic(self.document.tolerance());
        } else if command.eq_ignore_ascii_case("Natural") {
            for seam in &mut session.seams {
                seam.natural();
            }
        } else if option.eq_ignore_ascii_case("SnapToKnots") {
            session.snap_to_knots = match value.trim_start_matches('_') {
                "" => !session.snap_to_knots,
                yes if yes.eq_ignore_ascii_case("Yes") => true,
                no if no.eq_ignore_ascii_case("No") => false,
                _ => {
                    self.push_log("Use SnapToKnots=Yes or SnapToKnots=No".into());
                    return;
                }
            };
            self.curve_seam_snap_to_knots = session.snap_to_knots;
        } else if let Some(index) = session.target {
            let seam = &mut session.seams[index];
            let parameter = if option.eq_ignore_ascii_case("Parameter") {
                value
                    .parse::<Real>()
                    .ok()
                    .filter(|parameter| parameter.is_finite())
            } else {
                PointInput::parse_with_units(command, self.document.units())
                    .and_then(Result::ok)
                    .and_then(|point| {
                        point
                            .resolve(
                                self.viewports[self.active_viewport].construction_plane(),
                                self.last_point,
                            )
                            .ok()
                    })
                    .and_then(|point| {
                        seam.curve()
                            .closest_parameter(point, self.document.tolerance())
                            .ok()
                    })
            };
            if let Some(parameter) = parameter {
                seam.parameter = parameter;
            } else {
                self.push_log("Enter a seam point or a finite parameter".into());
                return;
            }
        } else {
            self.push_log("Choose a seam by dragging its marker or using SelID".into());
            return;
        }
        session.moving = None;
        self.log_curve_seam();
    }
}

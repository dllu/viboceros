//! Ellipse construction uses the same edit-free getter for previews and commits.
use super::*;
mod guides;
use viboceros_command::{
    curve_reference::CapturedCurve,
    ellipse_input::{Input, Mode, Slot},
};

pub(super) struct Session {
    pub(super) input: Input,
    subd_ellipsoid: bool,
    pub(super) cursor: Option<Point3>,
    curve: Option<CapturedCurve>,
    postselected: bool,
}
impl Session {
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.input
            .requests_curve()
            .then_some(self.curve.as_ref())
            .flatten()
            .map(CapturedCurve::curve)
    }
}
impl VibocerosApp {
    pub(super) fn start_subd_ellipsoid_ellipse(&mut self, input: Input) {
        self.ellipse_session = Some(Session {
            input,
            subd_ellipsoid: true,
            cursor: None,
            curve: None,
            postselected: false,
        });
        self.bind_ellipse_preselection();
        self.ellipse_prompt();
    }
    pub(super) fn try_start_ellipse(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            ["Ellipse", "Ell"]
                .iter()
                .any(|n| w.trim_start_matches(['_', '-']).eq_ignore_ascii_case(n))
        }) {
            return false;
        }
        let mut input = Input::new(
            self.commands.ellipse_options(),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.ellipse_session = Some(Session {
            input,
            subd_ellipsoid: false,
            cursor: None,
            curve: None,
            postselected: false,
        });
        self.bind_ellipse_preselection();
        self.ellipse_prompt();
        true
    }
    fn ellipse_prompt(&mut self) {
        let Some(s) = &self.ellipse_session else {
            return;
        };
        let input = s.input;
        let subd_ellipsoid = s.subd_ellipsoid;
        if subd_ellipsoid {
            self.sync_subd_ellipsoid_base(input);
        }
        let s = self.ellipse_session.as_ref().unwrap();
        let slot = if self.picking_ellipse_curve() {
            "select curve or surface edge"
        } else if s.input.requests_curve() && s.curve.is_some() {
            "center on curve"
        } else {
            match input.slot() {
                Slot::PointCount => "number of points",
                Slot::First => {
                    if input.mode() == Mode::FromFoci {
                        "first focus"
                    } else if input.mode() == Mode::Corner {
                        "first corner"
                    } else if input.mode() == Mode::Diameter {
                        "first axis start"
                    } else {
                        "center"
                    }
                }
                Slot::FirstAxis => "first axis end",
                Slot::SecondAxis => "second axis end",
                Slot::OppositeCorner if input.centered_corner() => "corner",
                Slot::OppositeCorner => "opposite corner",
                Slot::SecondFocus => "second focus",
                Slot::ThroughPoint => "point on ellipse",
                Slot::Complete => "complete",
            }
        };
        self.active_command = Some(if subd_ellipsoid {
            InteractiveCommand::SubDEllipsoid {
                anchor: input.anchor(),
            }
        } else {
            InteractiveCommand::Ellipse {
                center: input.anchor(),
                first_axis: input.reference(),
            }
        });
        self.drafting_plane = Some(input.mouse_plane());
        let options = if subd_ellipsoid && input.slot() == Slot::First {
            "; Corner, Diameter, FromFoci, AroundCurve"
        } else if input.slot() == Slot::First {
            "; Default, Vertical, Corner, Diameter, FromFoci, AroundCurve"
        } else {
            ""
        };
        let foci = if input.mode() == Mode::FromFoci {
            format!(
                "; MarkFoci={}",
                if input.options().mark_foci {
                    "Yes"
                } else {
                    "No"
                }
            )
        } else {
            String::new()
        };
        let length = input
            .locked_length()
            .map_or(String::new(), |v| format!("; length={v}"));
        let vertical = if input.vertical() && input.mode() == Mode::Diameter {
            " Vertical"
        } else {
            ""
        };
        let output = if subd_ellipsoid {
            String::new()
        } else if input.deformable() {
            format!("; Deformable; PointCount={}", input.options().point_count)
        } else if input.slot() == Slot::First {
            "; Deformable".into()
        } else {
            String::new()
        };
        self.push_log(format!(
            "{} {}{vertical}: {slot}{options}{foci}{length}{output}; Esc cancels",
            if subd_ellipsoid {
                "SubDEllipsoid"
            } else {
                "Ellipse"
            },
            input.mode().name()
        ));
    }
    pub(super) fn try_continue_ellipse(&mut self, text: &str) -> bool {
        if self.ellipse_session.is_none() {
            return false;
        }
        self.validate_ellipse_curve();
        if self.ellipse_session.is_none() {
            return true;
        }
        let picking = self.picking_ellipse_curve();
        let s = self.ellipse_session.as_mut().unwrap();
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.input.point_count_pending() {
                s.input.accept_point_count_default();
                self.bind_ellipse_preselection();
                self.ellipse_prompt();
            } else if s.input.locked_length().is_some() {
                if let Some(cursor) = s.cursor {
                    self.accept_filtered_drafting_point(cursor, false);
                } else {
                    self.push_log(
                        "Ellipse: move the pointer to choose the locked axis direction".into(),
                    );
                }
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        let prior_mode = s.input.mode();
        match s.input.option(text) {
            Ok(true) => {
                s.cursor = None;
                if prior_mode != s.input.mode() {
                    self.release_ellipse_picks();
                    let s = self.ellipse_session.as_mut().unwrap();
                    s.curve = None;
                    s.postselected = false;
                }
                self.bind_ellipse_preselection();
                self.point_filter = None;
                self.point_constraint = None;
                self.ellipse_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
            Ok(false) => {
                if picking {
                    let words = word.split_whitespace().collect::<Vec<_>>();
                    let reference = match words.as_slice() {
                        [key, id] if key.eq_ignore_ascii_case("SelID") => {
                            id.parse().ok().map(|id| (id, None))
                        }
                        [one] => one.parse().ok().map(|id| (id, None)).or_else(|| {
                            let (k, v) = one.split_once('=')?;
                            if k.eq_ignore_ascii_case("Edge") {
                                viboceros_command::curve_reference::parse_edge(v)
                                    .map(|(id, edge)| (id, Some(edge)))
                            } else if k.eq_ignore_ascii_case("Curve") {
                                v.parse().ok().map(|id| (id, None))
                            } else {
                                None
                            }
                        }),
                        _ => None,
                    };
                    if let Some((id, edge)) = reference {
                        self.pick_ellipse_curve(Some(id), edge);
                        self.command_input.clear();
                        return true;
                    }
                    if self
                        .commands
                        .recognizes(word.split_whitespace().next().unwrap_or(""))
                    {
                        return false;
                    }
                    self.ellipse_prompt();
                    self.command_input.clear();
                    return true;
                }
                if self
                    .commands
                    .recognizes(word.split_whitespace().next().unwrap_or(""))
                {
                    return false;
                }
                if s.input.point_count_pending() {
                    let result = text
                        .parse::<f64>()
                        .map_err(|_| {
                            "PointCount requires an integer from 4 through 1000".to_owned()
                        })
                        .and_then(|n| s.input.number(n).map_err(|e| e.to_string()));
                    match result {
                        Ok(()) => {
                            self.bind_ellipse_preselection();
                            self.ellipse_prompt();
                        }
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                    self.command_input.clear();
                    return true;
                }
                let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                    text,
                    self.document.units(),
                ) else {
                    return false;
                };
                match value {
                    Ok(value) => match s.input.number(value) {
                        Ok(_) => {
                            self.bind_ellipse_preselection();
                            self.ellipse_prompt();
                        }
                        Err(e) => self.push_log(format!("Error: {e}")),
                    },
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn accept_ellipse_point(&mut self, point: Point3) -> Option<bool> {
        self.ellipse_session.as_ref()?;
        self.validate_ellipse_curve();
        if self.ellipse_session.is_none() {
            return Some(false);
        }
        if self.picking_ellipse_curve() {
            self.ellipse_prompt();
            return Some(false);
        }
        let s = self.ellipse_session.as_mut().unwrap();
        if let Some(curve) = &s.curve
            && s.input.requests_curve()
        {
            match viboceros_command::curve_reference::perpendicular_frame(curve.curve_ref(), point)
            {
                Ok(frame) => {
                    s.input.around_curve(frame);
                    s.cursor = None;
                    self.ellipse_prompt();
                    return Some(true);
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return Some(false);
                }
            }
        }
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(Some(_)) => {
                if s.subd_ellipsoid {
                    let input = s.input;
                    let release = if s.postselected {
                        s.curve
                            .as_ref()
                            .map(|c| vec![c.object_id()])
                            .unwrap_or_default()
                    } else {
                        vec![]
                    };
                    self.release_ellipse_picks();
                    self.ellipse_session = None;
                    return Some(self.accept_subd_ellipsoid_base(input, release));
                }
                let command = if let Some(curve) = &s.curve {
                    s.input
                        .command_with_guide(curve.object_id(), curve.source().1, s.postselected)
                } else {
                    s.input.command()
                }
                .expect("completed ellipse");
                self.try_execute_command(&command)
            }
            Ok(None) => {
                s.cursor = None;
                self.ellipse_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
}

//! Axial Helix and Spiral input; all geometry stays in the command/kernel layers.
use super::*;
use viboceros_command::spiral_input::{Axis, Input, Kind, Numeric, Slot};
pub(super) struct Session {
    pub(super) input: Input,
    pending: Option<&'static str>,
    pub(super) cursor: Option<Point3>,
}
impl VibocerosApp {
    pub(super) fn try_start_spiral(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        let Some(kind) = words.next().and_then(Kind::parse) else {
            return false;
        };
        let mut input = Input::new(
            kind,
            self.commands.spiral_options(kind),
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|w| input.option(w).unwrap_or(false)) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.spiral_session = Some(Session {
            input,
            cursor: None,
            pending: None,
        });
        self.spiral_prompt();
        self.begin_spiral_rail_selection();
        true
    }
    fn spiral_prompt(&mut self) {
        let Some(s) = &self.spiral_session else {
            return;
        };
        let o = s.input.options();
        let scale = if o.diameter { 2. } else { 1. };
        let phase = s
            .pending
            .map(str::to_owned)
            .unwrap_or_else(|| match s.input.slot() {
                Slot::Rail => "select rail curve or surface edge".to_owned(),
                Slot::Start if s.input.axis() == Axis::Flat => "center of flat spiral".to_owned(),
                Slot::Start if s.input.kind() == Kind::Helix => {
                    "start of axis (Vertical, AroundCurve)".to_owned()
                }
                Slot::Start => "start of axis (Vertical, Flat, AroundCurve)".to_owned(),
                Slot::End => "end of axis".to_owned(),
                Slot::StartRadius => format!(
                    "{} and start point <{}>; number locks size, then pick direction",
                    if o.diameter { "diameter" } else { "radius" },
                    s.input.radius_value() * scale
                ),
                Slot::EndRadius => format!(
                    "second {} <{}>; Enter accepts this value",
                    if o.diameter { "diameter" } else { "radius" },
                    o.end_radius * scale
                ),
                Slot::Complete => "complete".to_owned(),
            });
        self.active_command = Some(InteractiveCommand::Spiral {
            kind: s.input.kind(),
            anchor: s.input.anchor(),
        });
        self.drafting_plane = Some(s.input.mouse_plane());
        self.push_log(format!(
            "{}: {phase}; Mode={:?}, Turns={}, Pitch={}, ReverseTwist={}; {}{}; Esc cancels",
            s.input.kind().name(),
            o.mode,
            o.turns,
            o.pitch,
            if o.reverse_twist { "Yes" } else { "No" },
            if o.diameter { "Radius" } else { "Diameter" },
            if s.input.axis() == Axis::AroundCurve {
                format!("; NumPointsPerTurn={}", o.points_per_turn)
            } else {
                String::new()
            }
        ));
    }
    fn finish_spiral(&mut self) {
        if !self.validate_spiral_rail() {
            return;
        }
        let Some(s) = &self.spiral_session else {
            return;
        };
        let kind = s.input.kind();
        let options = s.input.options();
        let command = match s.input.command() {
            Ok(c) => c,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return;
            }
        };
        if self.try_execute_command(&command) {
            self.commands.remember_spiral_options(kind, options);
        }
    }
    pub(super) fn try_continue_spiral(&mut self, text: &str) -> bool {
        if self.spiral_session.is_none() {
            return false;
        }
        self.validate_spiral_rail();
        if self.try_spiral_rail_token(text) {
            self.command_input.clear();
            return true;
        }
        let s = self.spiral_session.as_mut().unwrap();
        let word = text.trim().trim_start_matches('_');
        if word.trim_start_matches('-').eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if self
            .commands
            .recognizes(word.split_whitespace().next().unwrap_or(""))
            && !["Radius", "Diameter"]
                .iter()
                .any(|w| word.eq_ignore_ascii_case(w))
        {
            return false;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.pending.take().is_some() {
                self.spiral_prompt();
            } else if matches!(s.input.slot(), Slot::Start | Slot::Rail) {
                self.cancel_interactive_command(true);
            } else {
                let result = if s.input.slot() == Slot::StartRadius {
                    s.cursor
                        .ok_or("Move the pointer to set the starting direction".to_owned())
                        .and_then(|p| s.input.default_radius(p).map_err(|e| e.to_string()))
                } else {
                    s.input.enter().map_err(|e| e.to_string())
                };
                match result {
                    Ok(true) => self.finish_spiral(),
                    Ok(false) => self.spiral_prompt(),
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let pending = s.pending.or_else(|| {
            if key.eq_ignore_ascii_case("Turns") {
                Some("Turns")
            } else if key.eq_ignore_ascii_case("Pitch") {
                Some("Pitch")
            } else if key.eq_ignore_ascii_case("NumPointsPerTurn")
                || key.eq_ignore_ascii_case("PointsPerTurn")
            {
                Some("NumPointsPerTurn")
            } else if key.eq_ignore_ascii_case("Mode") && value.is_some() {
                Some("Mode")
            } else {
                None
            }
        });
        if let Some(key) = pending {
            let value = if s.pending.is_some() {
                Some(word)
            } else {
                value
            };
            let result = if let Some(value) = value {
                if key == "Mode" {
                    s.input.option(&format!("Mode={value}")).map(|_| ())
                } else {
                    let units = if key == "Turns" || key == "NumPointsPerTurn" {
                        &viboceros_geometry::LengthUnitSystem::Unset
                    } else {
                        self.document.units()
                    };
                    let number =
                        viboceros_drafting::PointInput::parse_length_with_units(value, units)
                            .ok_or_else(|| "expected a positive number".to_owned())
                            .and_then(|r| r.map_err(|e| e.to_string()));
                    match number {
                        Ok(v) => s.input.numeric(
                            if key == "Turns" {
                                Numeric::Turns
                            } else if key == "NumPointsPerTurn" {
                                Numeric::PointsPerTurn
                            } else {
                                Numeric::Pitch
                            },
                            v,
                        ),
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            self.command_input.clear();
                            return true;
                        }
                    }
                }
            } else {
                s.pending = Some(key);
                Ok(())
            };
            match result {
                Ok(()) => {
                    if value.is_some() {
                        s.pending = None;
                    }
                    self.spiral_prompt();
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.point_filter = None;
            self.point_constraint = None;
            self.command_input.clear();
            return true;
        }
        let result = match s.input.option(word) {
            Ok(true) => {
                self.point_filter = None;
                self.point_constraint = None;
                self.command_input.clear();
                self.spiral_prompt();
                self.begin_spiral_rail_selection();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_input.clear();
                return true;
            }
            Ok(false) => {
                if s.input.slot() == Slot::StartRadius
                    || s.input.slot() == Slot::EndRadius
                    || s.input.slot() == Slot::End && s.input.axis() == Axis::Vertical
                {
                    if let Some(number) = viboceros_drafting::PointInput::parse_length_with_units(
                        text,
                        self.document.units(),
                    ) {
                        match number {
                            Ok(v) => s.input.number(v),
                            Err(e) => {
                                self.push_log(format!("Error: {e}"));
                                self.command_input.clear();
                                return true;
                            }
                        }
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        };
        match result {
            Ok(true) => self.finish_spiral(),
            Ok(false) => self.spiral_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        true
    }
    pub(super) fn accept_spiral_point(&mut self, point: Point3) -> Option<bool> {
        self.spiral_session.as_ref()?;
        self.validate_spiral_rail();
        let s = self.spiral_session.as_mut().unwrap();
        if s.input.slot() == Slot::Rail {
            self.spiral_prompt();
            return Some(false);
        }
        if s.pending.is_some() {
            self.spiral_prompt();
            return Some(false);
        }
        s.input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        Some(match s.input.point(point) {
            Ok(true) => {
                self.finish_spiral();
                true
            }
            Ok(false) => {
                self.spiral_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        })
    }
}

impl VibocerosApp {
    pub(super) fn picking_spiral_rail(&self) -> bool {
        self.spiral_session
            .as_ref()
            .is_some_and(|s| s.input.slot() == Slot::Rail)
    }
    pub(super) fn validate_spiral_rail(&mut self) -> bool {
        if self
            .spiral_session
            .as_ref()
            .is_some_and(|s| !s.input.rail_current(&self.document))
        {
            let session = self.spiral_session.as_mut().unwrap();
            session.input.clear_rail();
            session.pending = None;
            session.cursor = None;
            self.document.clear_selection();
            self.component_selection.clear_choices();
            self.point_filter = None;
            self.point_constraint = None;
            self.push_log("Spiral rail changed or became unavailable; select it again".to_owned());
            self.spiral_prompt();
            return false;
        }
        true
    }
    fn begin_spiral_rail_selection(&mut self) {
        if self.picking_spiral_rail() {
            self.document.clear_selection();
        }
    }
    pub(super) fn pick_spiral_rail(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_spiral_rail() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a rail curve or surface edge".to_owned())
            .and_then(|id| {
                self.spiral_session
                    .as_mut()
                    .unwrap()
                    .input
                    .capture_rail(&self.document, id, edge)
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(()) => {
                self.spiral_session.as_mut().unwrap().cursor = None;
                if edge.is_none()
                    && let Some(id) = id
                {
                    let _ = self
                        .document
                        .select_object(id, viboceros_document::SelectionMode::Add);
                }
                self.component_selection.clear_choices();
                self.spiral_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn try_spiral_rail_token(&mut self, text: &str) -> bool {
        if !self.picking_spiral_rail() {
            return false;
        }
        let Some((key, value)) = text.trim().trim_start_matches('_').split_once('=') else {
            return false;
        };
        let source = if key.eq_ignore_ascii_case("Curve") {
            value.parse().ok().map(|id| (id, None))
        } else if key.eq_ignore_ascii_case("Edge") {
            viboceros_command::curve_reference::parse_edge(value).map(|(id, e)| (id, Some(e)))
        } else if key.eq_ignore_ascii_case("PathName") {
            let ids = self
                .document
                .objects()
                .filter(|o| {
                    o.attributes()
                        .name()
                        .is_some_and(|n| n.eq_ignore_ascii_case(value))
                })
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if ids.len() != 1 {
                self.push_log(
                    "Error: a rail name must identify exactly one selectable curve".to_owned(),
                );
                return true;
            }
            Some((ids[0], None))
        } else {
            return false;
        };
        if let Some((id, edge)) = source {
            self.pick_spiral_rail(Some(id), edge);
        } else {
            self.push_log("Error: invalid spiral rail reference".to_owned());
        }
        true
    }
}

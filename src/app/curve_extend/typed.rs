//! Typed options, lengths, IDs, and drafted coordinates for Extend.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn continue_curve_extend(&mut self, text: &str) -> bool {
        if self.curve_extend_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.command_input.clear();
        self.validate_curve_extend();
        let Some(session) = &mut self.curve_extend_session else {
            return true;
        };
        let text = text.trim();
        let word = text.trim_start_matches('_');
        if word == "!" || word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            let result = self.document.undo_history_group(&mut session.history);
            match result {
                Ok(true) => self.push_log("Undid the last extension".into()),
                Ok(false) => self.push_log("No extension to undo".into()),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.validate_curve_extend();
            self.log_curve_extend();
            return true;
        }
        if text.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if session.getter.take().is_some() {
                self.log_curve_extend();
            } else {
                match session.step {
                    Step::Boundaries => self.finish_extend_boundaries(),
                    Step::Source => self.cancel_interactive_command(false),
                    Step::End(..) => {
                        let length = session.prefs.last_length;
                        self.accept_extend_length(length);
                    }
                }
            }
            return true;
        }
        let (name, value) = word
            .split_once(|c: char| c == '=' || c.is_whitespace())
            .unwrap_or((word, ""));
        let value = value.trim();
        let continuation = session.getter;
        if matches!(continuation, Some(Getter::SelId)) || name.eq_ignore_ascii_case("SelID") {
            let id = if matches!(continuation, Some(Getter::SelId)) {
                word
            } else {
                value
            };
            if id.is_empty() {
                session.getter = Some(Getter::SelId);
                self.log_curve_extend();
            } else if let Ok(id) = id.parse::<ObjectId>() {
                session.getter = None;
                if self.picking_curve_extend_boundaries() {
                    self.select_extend_boundaries([id], SelectionMode::Add);
                    self.log_curve_extend();
                } else if self.picking_curve_extend_source() {
                    self.set_typed_extend_source(id, None);
                }
            } else {
                self.push_log("Enter an existing object ID".into());
            }
            return true;
        }
        if self.picking_curve_extend_boundaries() {
            if word.eq_ignore_ascii_case("SelAll") {
                let ids = self
                    .document
                    .objects()
                    .filter(|object| self.document.is_object_selectable(object.id()))
                    .map(|object| object.id())
                    .collect::<Vec<_>>();
                self.select_extend_boundaries(ids, SelectionMode::Add);
                return true;
            }
            if word.eq_ignore_ascii_case("SelNone") {
                self.document.clear_selection();
                return true;
            }
        }
        let Some(session) = &mut self.curve_extend_session else {
            return true;
        };
        let key = match continuation {
            Some(Getter::Type) => Some(("Type", word, Getter::Type)),
            Some(Getter::Join) => Some(("Join", word, Getter::Join)),
            _ if name.eq_ignore_ascii_case("Type") => Some(("Type", value, Getter::Type)),
            _ if name.eq_ignore_ascii_case("Join") => Some(("Join", value, Getter::Join)),
            _ => None,
        };
        if let Some((key, value, getter)) = key {
            if value.is_empty() {
                session.getter = Some(getter);
            } else {
                match session.prefs.options.set(key, value) {
                    Ok(()) => {
                        session.getter = None;
                        if let Step::End(_, input) = &mut session.step {
                            *input = EndInput::Along;
                        }
                        session.preview = preview::Preview::default();
                        self.curve_extend_preferences[session.command as usize] = session.prefs;
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            self.log_curve_extend();
            return true;
        }
        if session.command == Command::Dynamic && word.eq_ignore_ascii_case("ToBoundary") {
            self.document.clear_selection();
            session.step = Step::Boundaries;
            session.typed_source = None;
            session.preview = preview::Preview::default();
            self.log_curve_extend();
            return true;
        }
        if matches!(session.step, Step::Source) && name.eq_ignore_ascii_case("Edge") {
            if let Some((id, index)) = viboceros_command::curve_reference::parse_edge(value) {
                self.set_typed_extend_source(id, Some(index));
            } else {
                self.push_log("Enter Edge=object-id,index".into());
            }
            return true;
        }
        if let Some((id, edge)) = session.typed_source
            && (word.eq_ignore_ascii_case("Start") || word.eq_ignore_ascii_case("End"))
        {
            let point = self
                .document
                .object(id)
                .and_then(|object| {
                    viboceros_command::curve_reference::resolve(
                        object.geometry(),
                        edge,
                        self.document.tolerance(),
                    )
                })
                .and_then(|reference| {
                    if word.eq_ignore_ascii_case("Start") {
                        reference.curve().start_point().ok()
                    } else {
                        reference.curve().end_point().ok()
                    }
                });
            if let Some(point) = point {
                self.take_extend_source(id, edge, Some(point));
            } else {
                self.push_log("Select an open curve or surface edge".into());
            }
            return true;
        }
        if let Step::End(_, input) = &mut session.step {
            let style = session.prefs.options.style;
            let next = if style == Style::Arc && word.eq_ignore_ascii_case("Center") {
                Some(EndInput::Center)
            } else if matches!(style, Style::Arc | Style::Smooth)
                && word.eq_ignore_ascii_case("ToPoint")
            {
                Some(EndInput::ToPoint)
            } else {
                None
            };
            if let Some(next) = next {
                *input = next;
                session.preview = preview::Preview::default();
                self.log_curve_extend();
                return true;
            }
        }
        let key_length =
            name.eq_ignore_ascii_case("ExtensionLength") || name.eq_ignore_ascii_case("Length");
        if key_length && value.is_empty() {
            session.getter = Some(Getter::Length);
            self.log_curve_extend();
            return true;
        }
        if let Some(length) = PointInput::parse_length_with_units(
            if key_length { value } else { text },
            self.document.units(),
        ) {
            match length {
                Ok(length) => {
                    session.getter = None;
                    self.accept_extend_length(length);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        if session.wants_point()
            && let Some(parsed) = PointInput::parse_with_units(text, self.document.units())
        {
            match parsed.and_then(|p| {
                p.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    session.anchor(),
                )
            }) {
                Ok(point) => {
                    self.accept_curve_extend_point(point);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        } else {
            self.log_curve_extend();
        }
        true
    }

    fn set_typed_extend_source(&mut self, id: ObjectId, edge: Option<usize>) {
        if let Some(session) = &mut self.curve_extend_session {
            session.typed_source = Some((id, edge));
        }
        self.log_curve_extend();
    }

    fn accept_extend_length(&mut self, length: f64) {
        let Some(session) = &mut self.curve_extend_session else {
            return;
        };
        if !length.is_finite() || length == 0. {
            self.push_log("Enter a nonzero extension or shortening length".into());
            return;
        }
        if let Step::End(source, input) = &session.step {
            if matches!(input, EndInput::Center) {
                self.push_log("Pick the center of the arc extension".into());
                return;
            }
            let source = source.clone();
            let target = match input {
                EndInput::Centered(center) => Target::ArcCenterLength {
                    center: *center,
                    length,
                },
                _ => Target::Length(length),
            };
            if !self.finish_extend_step(&source, &[], target) {
                return;
            }
        } else {
            session.method = Method::Length(length);
            session.step = Step::Source;
            self.document.clear_selection();
        }
        let session = self.curve_extend_session.as_mut().unwrap();
        session.prefs.last_length = length;
        self.curve_extend_preferences[session.command as usize] = session.prefs;
        self.log_curve_extend();
    }
}

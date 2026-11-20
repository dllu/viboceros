//! Ordered curve-end picks and remembered Connect options.
use super::*;
use viboceros_command::{CommandError, connect};
use viboceros_geometry::NurbsCurve;
mod preview;

#[derive(Clone, Copy)]
enum Getter {
    Arcs,
    OtherCurves,
    ObjectId,
}

pub(super) struct Session {
    first: Option<connect::Source>,
    options: connect::Options,
    getter: Option<Getter>,
    tolerance: Tolerance,
    preview: preview::Preview,
}

impl Session {
    pub(super) fn hint(&self) -> &'static str {
        match self.getter {
            Some(Getter::Arcs) => "Extend arcs by Arc or Line",
            Some(Getter::OtherCurves) => "Extend other curves by Line or Smooth",
            Some(Getter::ObjectId) => "Enter the curve object ID",
            None if self.first.is_none() => "Select first curve near the end",
            None => "Select second curve near the end",
        }
    }

    pub(super) fn preview_curves(&self, document: &Document) -> Option<&[NurbsCurve]> {
        self.preview.curves(document, self.options)
    }
}

impl VibocerosApp {
    pub(super) fn start_curve_connect(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|word| {
            !word
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Connect")
        }) {
            return false;
        }
        let options = match connect::parse(&words[1..], self.commands.connect_options()) {
            Ok(input) if input.picks.iter().any(Option::is_some) => return false,
            Ok(input) => input.options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.curve_connect_session = Some(Session {
            first: None,
            options,
            getter: None,
            tolerance: self.document.tolerance(),
            preview: preview::Preview::default(),
        });
        self.commands.remember_connect_options(options);
        self.push_log(format!("> {text}"));
        self.log_connect_prompt();
        true
    }

    fn log_connect_prompt(&mut self) {
        if let Some(session) = &self.curve_connect_session {
            self.push_log(format!(
                "{} ({})",
                session.hint(),
                session.options.arguments()
            ));
        }
    }

    pub(super) fn validate_curve_connect(&mut self) {
        let stale = self.curve_connect_session.as_ref().is_some_and(|session| {
            session.tolerance != self.document.tolerance()
                || session
                    .first
                    .as_ref()
                    .is_some_and(|source| !source.is_current(&self.document))
        });
        if stale {
            self.cancel_interactive_command(false);
            self.push_log("Connect source or document tolerance changed".into());
        }
    }

    pub(super) fn picking_curve_connect(&self) -> bool {
        self.plane_prompt.is_none()
            && self.subcurve_prompt.is_none()
            && self
                .curve_connect_session
                .as_ref()
                .is_some_and(|s| s.getter.is_none())
    }

    pub(super) fn connect_first_id(&self) -> Option<ObjectId> {
        self.curve_connect_session
            .as_ref()?
            .first
            .as_ref()
            .map(connect::Source::id)
    }

    pub(super) fn pick_curve_connect(
        &mut self,
        object: Option<ObjectId>,
        pick: Option<(usize, egui::Pos2)>,
    ) -> bool {
        if self.curve_connect_session.is_none()
            || self.plane_prompt.is_some()
            || self.subcurve_prompt.is_some()
        {
            return false;
        }
        if !self.picking_curve_connect() {
            return true;
        }
        if let Some(id) = object {
            let point = pick.and_then(|(viewport, cursor)| {
                self.viewports[viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    None,
                    cursor,
                    self.document.tolerance(),
                )
            });
            if pick.is_none() || point.is_some() {
                self.connect_source(id, point, None);
            }
        }
        true
    }

    fn connect_source(
        &mut self,
        object: ObjectId,
        point: Option<Point3>,
        portion: Option<viboceros_geometry::Curve3>,
    ) {
        self.validate_curve_connect();
        let Some(session) = &mut self.curve_connect_session else {
            return;
        };
        let source = match connect::Source::capture(&self.document, object, point, portion) {
            Ok(source) => source,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        let Some(first) = &session.first else {
            session.first = Some(source);
            self.document
                .set_transient_selection([object])
                .expect("captured Connect source");
            self.document.mark_object_selection_temporary([object]);
            self.log_connect_prompt();
            return;
        };
        if first.id() == source.id() {
            self.push_log("Select a different second curve".into());
            return;
        }
        let result = connect::prepare(&self.document, [first, &source], session.options)
            .and_then(|prepared| prepared.commit(&mut self.document));
        self.cancel_interactive_command(false);
        self.push_log(match result {
            Ok(_) => "Connected two curves".into(),
            Err(error) => format!("Unable to connect these curves: {error}"),
        });
    }

    pub(super) fn finish_connect_portion(
        &mut self,
        portion: Option<(
            viboceros_command::curve_reference::CapturedCurve,
            viboceros_geometry::Curve3,
        )>,
    ) -> Result<(), CommandError> {
        self.clear_temporary_subcurve_input();
        if let Some((reference, curve)) = portion {
            if !reference.current(&self.document) {
                return Err(CommandError::ConnectRequiresTwoCurves);
            }
            self.connect_source(reference.object_id(), None, Some(curve));
        } else {
            self.log_connect_prompt();
        }
        Ok(())
    }

    pub(super) fn continue_curve_connect(&mut self, text: &str) -> bool {
        if self.curve_connect_session.is_none()
            || self.subcurve_prompt.is_some()
            || self.plane_prompt.is_some()
        {
            return false;
        }
        self.command_input.clear();
        self.validate_curve_connect();
        let Some(session) = &mut self.curve_connect_session else {
            return true;
        };
        let text = text.trim();
        let command = text.trim_start_matches(['_', '-']);
        if text == "!" || command.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if command.eq_ignore_ascii_case("SubCrv") {
            self.begin_temporary_subcurve_prompt(None, subcurve_prompt::Temporary::Connect);
            return true;
        }
        let (name, value) = command
            .split_once(char::is_whitespace)
            .unwrap_or((command, ""));
        if name.eq_ignore_ascii_case("SelID") || matches!(session.getter, Some(Getter::ObjectId)) {
            let id = if name.eq_ignore_ascii_case("SelID") {
                value.trim()
            } else {
                text
            };
            if id.is_empty() {
                session.getter = Some(Getter::ObjectId);
                self.log_connect_prompt();
            } else if let Ok(id) = id.parse::<ObjectId>() {
                session.getter = None;
                self.connect_source(id, None, None);
            } else {
                self.push_log("Enter an existing curve object ID".into());
            }
            return true;
        }
        let option = match session.getter {
            Some(Getter::Arcs) => format!("ExtendArcsBy={command}"),
            Some(Getter::OtherCurves) => format!("ExtendOtherCurvesBy={command}"),
            _ if command.eq_ignore_ascii_case("Join") => {
                format!("Join={}", if session.options.join { "No" } else { "Yes" })
            }
            _ if command.eq_ignore_ascii_case("ExtendArcsBy") => {
                session.getter = Some(Getter::Arcs);
                self.log_connect_prompt();
                return true;
            }
            _ if command.eq_ignore_ascii_case("ExtendOtherCurvesBy") => {
                session.getter = Some(Getter::OtherCurves);
                self.log_connect_prompt();
                return true;
            }
            _ if !value.trim().is_empty() => format!("{name}={}", value.trim()),
            _ => command.to_owned(),
        };
        if !text.is_empty() {
            match connect::parse(&[&option], session.options) {
                Ok(input) if input.picks.iter().all(Option::is_none) => {
                    session.options = input.options;
                    session.getter = None;
                    self.commands.remember_connect_options(input.options);
                }
                _ => self.push_log(format!("Error: {}", connect::USAGE)),
            }
        } else if session.getter.take().is_none() {
            self.cancel_interactive_command(true);
            return true;
        }
        self.log_connect_prompt();
        true
    }
}

//! Suspend a numeric getter while Distance, Length, or Radius supplies a value.
use super::*;
use viboceros_command::{CommandError, RadiusMeasurement};
use viboceros_drafting::{ObjectSnapModes, PointConstraintState, PointFilterSession};

mod length;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Distance,
    Length,
    Radius,
}

impl Kind {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim_start_matches(['\'', '_', '-']);
        if text.eq_ignore_ascii_case("Distance") {
            Some(Self::Distance)
        } else if text.eq_ignore_ascii_case("Length") || text.eq_ignore_ascii_case("Len") {
            Some(Self::Length)
        } else if text.eq_ignore_ascii_case("Radius") {
            Some(Self::Radius)
        } else {
            None
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Distance => "Distance",
            Self::Length => "Length",
            Self::Radius => "Radius",
        }
    }
}

#[derive(Clone, Copy)]
enum Getter {
    Fillet,
    EdgeBlend { hover: Option<(Point3, f64)> },
    Offset,
    OffsetSrf,
    Circle,
}

impl Getter {
    fn active(app: &VibocerosApp, transparent: bool) -> Option<Self> {
        if app
            .curve_fillet_session
            .as_ref()
            .is_some_and(curve_fillet::Session::takes_radius_measurement)
        {
            Some(Self::Fillet)
        } else if app
            .offset_surface_session
            .as_ref()
            .is_some_and(offset_surface::Session::takes_distance_measurement)
        {
            Some(Self::OffsetSrf)
        } else if app
            .curve_offset_session
            .as_ref()
            .is_some_and(curve_offset::Session::takes_distance_measurement)
        {
            Some(Self::Offset)
        } else if transparent && app.circle_takes_length_measurement() {
            Some(Self::Circle)
        } else {
            None
        }
    }

    fn resume(self, app: &mut VibocerosApp) {
        match self {
            Self::Fillet => app.curve_fillet_prompt(),
            Self::EdgeBlend { .. } => app.log_edge_blend_prompt(),
            Self::Offset => app.curve_offset_prompt(),
            Self::OffsetSrf => app.log_offset_surface(),
            Self::Circle => {
                if let Some(command) = app.active_command {
                    app.push_log(command.prompt().to_owned());
                }
            }
        }
    }

    fn accept(self, app: &mut VibocerosApp, value: f64) -> bool {
        let value = value.to_string();
        match self {
            Self::Fillet => app.continue_curve_fillet(&value),
            Self::EdgeBlend { .. } => app.try_continue_edge_fillet(&value),
            Self::Offset => app.continue_curve_offset(&value),
            Self::OffsetSrf => app.continue_offset_surface(&value),
            Self::Circle => app.try_continue_circle(&value),
        }
    }
}

pub(super) struct Session {
    kind: Kind,
    getter: Getter,
    command: Option<InteractiveCommand>,
    point_filter: Option<PointFilterSession>,
    point_constraint: Option<PointConstraintState>,
    snap_override: Option<ObjectSnapModes>,
    last_point: Option<Point3>,
    drafting_plane: Option<Frame3>,
    length: length::Inputs,
}

impl VibocerosApp {
    pub(super) fn nested_measurement_captures_input(&self) -> bool {
        self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && self.copy_cplane_source.is_none()
            && self.zoom_target.is_none()
            && self.zoom_factor_pending.is_none()
            && self.snap_size_pending.is_none()
            && !self.zoom_window_pending
    }

    pub(super) fn start_nested_measurement(&mut self, input: &str) -> bool {
        let Some(kind) = Kind::parse(input) else {
            return false;
        };
        let Some(getter) = Getter::active(self, input.starts_with('\'')) else {
            return false;
        };
        self.begin_nested_measurement(kind, getter)
    }

    pub(super) fn start_edge_blend_distance(&mut self) -> bool {
        self.begin_nested_measurement(Kind::Distance, Getter::EdgeBlend { hover: None })
    }

    fn begin_nested_measurement(&mut self, kind: Kind, getter: Getter) -> bool {
        if !self.nested_measurement_captures_input()
            || self.object_prompt.is_some()
            || self.nested_measurement.is_some()
        {
            return false;
        }
        self.nested_measurement = Some(Session {
            kind,
            getter,
            command: self.active_command,
            point_filter: self.point_filter.take(),
            point_constraint: self.point_constraint.take(),
            snap_override: self.snaps.model_override.take(),
            last_point: self.last_point,
            drafting_plane: self.drafting_plane.take(),
            length: length::Inputs::default(),
        });
        self.command_input.clear();
        match kind {
            Kind::Radius => self.begin_nested_radius(),
            Kind::Length => {
                self.active_command = None;
                self.log_nested_length();
            }
            Kind::Distance => {
                let command = InteractiveCommand::Distance {
                    start: None,
                    previous_last: self.last_point,
                    display_units: self.nested_distance_units,
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
        }
        true
    }

    pub(super) fn update_edge_blend_distance_hover(&mut self, point: Option<Point3>) -> bool {
        let Some(Session {
            getter: Getter::EdgeBlend { hover },
            ..
        }) = self.nested_measurement.as_mut()
        else {
            return false;
        };
        let sample = match (self.active_command, point) {
            (
                Some(InteractiveCommand::Distance {
                    start: Some(start), ..
                }),
                Some(point),
            ) => start.distance_to(point).ok().map(|value| (start, value)),
            _ => None,
        };
        std::mem::replace(hover, sample) != sample
    }

    pub(super) fn edge_blend_distance_hover(&self) -> Option<f64> {
        let Getter::EdgeBlend {
            hover: Some((anchor, value)),
        } = self.nested_measurement.as_ref()?.getter
        else {
            return None;
        };
        let InteractiveCommand::Distance { start, .. } = self.active_command? else {
            return None;
        };
        (start == Some(anchor) && !self.distance_units_prompt).then_some(value)
    }

    pub(super) fn nested_measurement_kind(&self) -> Option<Kind> {
        self.nested_measurement.as_ref().map(|session| session.kind)
    }

    pub(super) fn measuring_nested_radius(&self) -> bool {
        self.nested_measurement_kind() == Some(Kind::Radius)
    }

    pub(super) fn nested_measurement_hint(&self) -> Option<&str> {
        let session = self.nested_measurement.as_ref()?;
        Some(if let Some(prompt) = &self.chain_prompt {
            prompt.hint()
        } else if let Some(prompt) = &self.subcurve_prompt {
            prompt.hint()
        } else if session.kind == Kind::Length {
            session.length.hint()
        } else {
            self.active_command?.prompt()
        })
    }

    pub(super) fn continue_nested_measurement(&mut self, input: &str) -> bool {
        let Some(kind) = self.nested_measurement_kind() else {
            return false;
        };
        if !self.nested_measurement_captures_input() {
            return false;
        }
        let text = input.trim().trim_start_matches('_');
        if text.eq_ignore_ascii_case("Cancel") || text == "!" {
            return self.cancel_nested_measurement();
        }
        if self.try_run_view_command(input) {
            return true;
        }
        if self.subcurve_prompt.is_some() {
            self.continue_subcurve_prompt(input);
            return true;
        }
        if kind == Kind::Length {
            return self.continue_nested_length(text);
        }
        if self.try_continue_distance(input) || self.try_continue_radius(input) {
            return true;
        }
        if text.is_empty() || text.eq_ignore_ascii_case("Enter") {
            return self.cancel_nested_measurement();
        }
        if self.try_one_shot_snap(input)
            || self.try_continue_point_constraint(input)
            || self.try_continue_point_filter(input)
        {
            return true;
        }
        if !self.try_continue_point_input(input) {
            self.log_nested_point_prompt();
        }
        self.command_input.clear();
        true
    }

    fn log_nested_point_prompt(&mut self) {
        if self.measuring_nested_radius() {
            self.log_radius_prompt();
        } else if let Some(command) = self.active_command {
            self.push_log(command.prompt().to_owned());
        }
    }

    pub(super) fn nested_measurement_wants_point(&self) -> bool {
        self.subcurve_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.source.is_some())
            || self.nested_measurement_kind() == Some(Kind::Distance) && !self.distance_units_prompt
    }

    pub(super) fn accept_nested_measurement_point(&mut self, point: Point3) -> Option<bool> {
        self.nested_measurement.as_ref()?;
        let plane = self.viewports[self.active_viewport].construction_plane();
        if self.subcurve_prompt.is_some() {
            let accepted = self.accept_standalone_subcurve_point(point);
            if accepted == Some(true) && self.subcurve_prompt.is_some() {
                self.drafting_plane.get_or_insert(plane);
                self.last_point = Some(point);
                self.snaps.model_override = None;
            }
            return Some(accepted.unwrap_or(false));
        }
        let accepted = match self.active_command {
            Some(InteractiveCommand::Distance {
                start: None,
                previous_last,
                display_units,
            }) if !self.distance_units_prompt => {
                let command = InteractiveCommand::Distance {
                    start: Some(point),
                    previous_last,
                    display_units,
                };
                self.active_command = Some(command);
                self.drafting_plane = Some(plane);
                self.last_point = Some(point);
                self.snaps.model_override = None;
                self.push_log(command.prompt().to_owned());
                true
            }
            Some(InteractiveCommand::Distance {
                start: Some(start),
                display_units,
                ..
            }) if !self.distance_units_prompt => self.finish_distance(
                start,
                point,
                display_units,
                self.drafting_plane.unwrap_or(plane),
            ),
            Some(InteractiveCommand::Radius { .. }) => {
                self.finish_radius(point, false, false, None)
            }
            _ => false,
        };
        Some(accepted)
    }

    pub(super) fn cancel_nested_measurement(&mut self) -> bool {
        let Some(session) = self.resume_nested_getter() else {
            return false;
        };
        self.push_log(format!("{} measurement canceled", session.kind.name()));
        session.getter.resume(self);
        true
    }

    fn resume_nested_getter(&mut self) -> Option<Session> {
        let mut session = self.nested_measurement.take()?;
        if session.kind == Kind::Radius {
            self.radius_session = None;
        } else if session.kind == Kind::Length {
            self.chain_prompt = None;
        }
        self.subcurve_prompt = None;
        self.distance_units_prompt = false;
        self.active_command = session.command;
        self.point_filter = session.point_filter.take();
        self.point_constraint = session.point_constraint;
        self.snaps.model_override = session.snap_override;
        self.last_point = session.last_point;
        self.drafting_plane = session.drafting_plane;
        self.command_input.clear();
        Some(session)
    }

    pub(super) fn finish_nested_value(&mut self, value: f64) -> bool {
        if value <= 0.
            && self
                .nested_measurement
                .as_ref()
                .is_some_and(|s| matches!(s.getter, Getter::EdgeBlend { .. }))
        {
            self.push_log("Error: pick two different points for the edge blend size".into());
            return false;
        }
        let Some(session) = self.resume_nested_getter() else {
            return false;
        };
        session.getter.accept(self, value)
    }

    pub(super) fn accept_nested_radius(
        &mut self,
        result: Result<RadiusMeasurement, CommandError>,
    ) -> bool {
        let value = match result.and_then(|sample| sample.model_value(false)) {
            Ok(Some(value)) if value > 0.0 => value,
            Ok(_) => {
                self.push_log("Pick a curve location with a finite positive radius".into());
                return false;
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        self.push_log(format!("Radius = {value}"));
        self.finish_nested_value(value)
    }
}

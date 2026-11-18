//! Return a model-unit radius to the numeric getter that requested it.
use super::*;
use viboceros_drafting::{ObjectSnapModes, PointConstraintState, PointFilterSession, PointInput};

#[derive(Clone, Copy)]
enum Getter {
    Fillet,
    Offset,
    OffsetSrf,
}

pub(super) struct Nested {
    getter: Getter,
    command: Option<InteractiveCommand>,
    point_filter: Option<PointFilterSession>,
    point_constraint: Option<PointConstraintState>,
    snap_override: Option<ObjectSnapModes>,
    last_point: Option<Point3>,
}

impl VibocerosApp {
    pub(in crate::app) fn start_nested_radius(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some()
            || self.object_prompt.is_some()
            || self.radius_session.is_some()
            || !input
                .trim()
                .trim_start_matches(['\'', '_'])
                .eq_ignore_ascii_case("Radius")
        {
            return false;
        }
        let getter = if self
            .curve_fillet_session
            .as_ref()
            .is_some_and(curve_fillet::Session::takes_radius_measurement)
        {
            Getter::Fillet
        } else if self
            .offset_surface_session
            .as_ref()
            .is_some_and(offset_surface::Session::takes_distance_measurement)
        {
            Getter::OffsetSrf
        } else if self
            .curve_offset_session
            .as_ref()
            .is_some_and(curve_offset::Session::takes_distance_measurement)
        {
            Getter::Offset
        } else {
            return false;
        };
        let nested = Nested {
            getter,
            command: self.active_command,
            point_filter: self.point_filter.take(),
            point_constraint: self.point_constraint.take(),
            snap_override: self.snaps.model_override.take(),
            last_point: self.last_point,
        };
        self.radius_session = Some(Session {
            unrestricted: true,
            nested: Some(nested),
            ..Default::default()
        });
        self.active_command = Some(InteractiveCommand::Radius {
            diameter: false,
            mark: false,
            display_units: None,
        });
        self.command_input.clear();
        self.log_radius_prompt();
        true
    }

    pub(in crate::app) fn continue_nested_radius(&mut self, input: &str) -> bool {
        if !self.measuring_nested_radius() || self.plane_prompt.is_some() {
            return false;
        }
        let token = input.trim_start_matches('_');
        if token.eq_ignore_ascii_case("Cancel") || token == "!" {
            return self.cancel_nested_radius();
        }
        if self.subcurve_prompt.is_some() {
            let _ = self.continue_subcurve_prompt(input);
            return true;
        }
        if self.try_continue_radius(input) {
            return true;
        }
        match PointInput::parse_with_units(input, self.document.units()) {
            Some(point) => match point.and_then(|point| {
                point.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    self.last_point,
                )
            }) {
                Ok(point) => {
                    self.finish_radius(point, false, false, None);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            None => self.log_radius_prompt(),
        }
        self.command_input.clear();
        true
    }

    pub(in crate::app) fn measuring_nested_radius(&self) -> bool {
        self.radius_session
            .as_ref()
            .is_some_and(|s| s.nested.is_some())
    }

    pub(in crate::app) fn cancel_nested_radius(&mut self) -> bool {
        let Some(parent) = self.resume_after_radius() else {
            return false;
        };
        self.push_log("Radius measurement canceled".into());
        match parent {
            Getter::Fillet => self.curve_fillet_prompt(),
            Getter::Offset => self.curve_offset_prompt(),
            Getter::OffsetSrf => self.log_offset_surface(),
        }
        true
    }

    fn resume_after_radius(&mut self) -> Option<Getter> {
        let parent = self.radius_session.as_mut()?.nested.take()?;
        self.radius_session = None;
        self.subcurve_prompt = None;
        self.active_command = parent.command;
        self.point_filter = parent.point_filter;
        self.point_constraint = parent.point_constraint;
        self.snaps.model_override = parent.snap_override;
        self.last_point = parent.last_point;
        self.command_input.clear();
        Some(parent.getter)
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
        let Some(parent) = self.resume_after_radius() else {
            return false;
        };
        self.push_log(format!("Radius = {value}"));
        let value = value.to_string();
        match parent {
            Getter::Fillet => self.continue_curve_fillet(&value),
            Getter::Offset => self.continue_curve_offset(&value),
            Getter::OffsetSrf => self.continue_offset_surface(&value),
        }
    }
}

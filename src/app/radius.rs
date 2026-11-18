//! Radius and Diameter share the viewport getter and curvature query.
use super::*;
use crate::viewport::radius::{RadiusClick, RadiusHit, RadiusInput};
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::{CommandError, RadiusMeasurement};
use viboceros_document::{GeometrySnapshot, HistoryGroup};
use viboceros_geometry::{Curve3, NurbsCurve};
mod nested;
mod options;

struct Portion {
    curve: Curve3,
    display: NurbsCurve,
}

struct Source {
    curve: CapturedCurve,
    portion: Option<Portion>,
}

struct Hover {
    viewport: usize,
    pixel: egui::Pos2,
    hit: RadiusHit,
    source: GeometrySnapshot,
    value: String,
}

impl Hover {
    fn source_current(&self, document: &Document) -> bool {
        document.object(self.hit.object).is_some_and(|object| {
            document.is_object_selectable(self.hit.object)
                && object.geometry_snapshot().shares_storage_with(&self.source)
        })
    }
}

impl Source {
    fn curve_ref(&self) -> viboceros_geometry::CurveRef<'_> {
        self.portion
            .as_ref()
            .map_or_else(|| self.curve.curve_ref(), |portion| portion.curve.as_ref())
    }

    fn viewport_input(&self) -> RadiusInput<'_> {
        RadiusInput::Fixed {
            curve: self.curve_ref(),
            display: self
                .portion
                .as_ref()
                .map_or_else(|| self.curve.curve(), |portion| &portion.display),
            object: self.curve.object_id(),
        }
    }
}

#[derive(Default)]
pub(super) struct Session {
    source: Option<Source>,
    selecting: bool,
    choosing_units: bool,
    unrestricted: bool,
    hover: Option<Hover>,
    subcurve_resume: Option<InteractiveCommand>,
    history: Option<HistoryGroup>,
    nested: Option<nested::Nested>,
}

fn name(diameter: bool) -> &'static str {
    if diameter { "Diameter" } else { "Radius" }
}

fn mark_name(diameter: bool) -> &'static str {
    if diameter {
        "MarkDiameter"
    } else {
        "MarkRadius"
    }
}

impl Session {
    pub(super) fn viewport_input(&self, preselected: bool) -> Option<RadiusInput<'_>> {
        if self.choosing_units {
            return None;
        }
        Some(self.source.as_ref().filter(|_| !self.selecting).map_or(
            RadiusInput::Curves {
                restrict_to_selection: !self.selecting && !self.unrestricted && preselected,
            },
            Source::viewport_input,
        ))
    }
}

impl VibocerosApp {
    pub(super) fn radius_status(&self) -> Option<&str> {
        self.radius_session
            .as_ref()?
            .hover
            .as_ref()
            .map(|hover| hover.value.as_str())
    }

    pub(super) fn update_radius_hover(
        &mut self,
        hit: Option<(usize, egui::Pos2, RadiusHit)>,
    ) -> bool {
        let Some(InteractiveCommand::Radius {
            diameter,
            display_units,
            ..
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = &self.radius_session else {
            return false;
        };
        let hit = hit.filter(|_| !session.selecting && !session.choosing_units);
        if session.hover.as_ref().is_some_and(|old| {
            hit == Some((old.viewport, old.pixel, old.hit)) && old.source_current(&self.document)
        }) {
            return false;
        }
        let hover = hit.and_then(|(viewport, pixel, hit)| {
            let source = self
                .document
                .object(hit.object)?
                .geometry_snapshot()
                .clone();
            let measurement = self.radius_measurement(hit).ok()?;
            let value = measurement
                .display_value(&self.document, diameter, display_units)
                .ok()?;
            Some(Hover {
                viewport,
                pixel,
                hit,
                source,
                value: format!("{} = {value}", name(diameter)),
            })
        });
        let changed = session.hover.is_some() || hover.is_some();
        self.radius_session.as_mut().unwrap().hover = hover;
        changed
    }

    pub(super) fn validate_radius_source(&mut self) {
        let Some(session) = &mut self.radius_session else {
            return;
        };
        if session
            .source
            .as_ref()
            .is_some_and(|s| !s.curve.current(&self.document))
        {
            session.source = None;
            session.hover = None;
            session.selecting = true;
            self.push_log("Measurement source changed. Select a curve again.".into());
        }
    }

    fn radius_measurement(&self, hit: RadiusHit) -> Result<RadiusMeasurement, CommandError> {
        let source = self.radius_session.as_ref().and_then(|s| s.source.as_ref());
        let curve = if let Some(source) = source {
            if source.curve.object_id() != hit.object || !source.curve.current(&self.document) {
                return Err(CommandError::Usage("The measurement source changed"));
            }
            source.curve_ref()
        } else {
            self.document
                .object(hit.object)
                .filter(|_| self.document.is_object_selectable(hit.object))
                .and_then(|object| object.geometry().curve_ref())
                .ok_or(CommandError::Usage("Select an available curve"))?
        };
        RadiusMeasurement::evaluate(curve, hit.parameter)
    }

    pub(super) fn pick_radius_location(&mut self, click: RadiusClick) -> bool {
        let Some(InteractiveCommand::Radius {
            diameter,
            mark,
            display_units,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = &self.radius_session else {
            return false;
        };
        if session.selecting {
            if let Some(hit) = click.hit {
                self.choose_radius_curve(hit.object);
            }
            return true;
        }
        // Use the visible sample when the button is released at the same
        // pixel. Panel changes between press and release can resize the view.
        let hit = session
            .hover
            .as_ref()
            .filter(|hover| {
                hover.viewport == click.viewport
                    && hover.pixel == click.pixel
                    && hover.source_current(&self.document)
            })
            .map(|hover| hover.hit)
            .or(click.hit);
        let Some(hit) = hit else { return true };
        if self.measuring_nested_radius() {
            let result = self.radius_measurement(hit);
            self.accept_nested_radius(result);
            return true;
        }
        let result = (|| {
            let measurement = self.radius_measurement(hit)?;
            if !mark {
                return measurement.finish(&mut self.document, diameter, false, display_units);
            }
            let session = self.radius_session.as_mut().unwrap();
            if session.history.is_none() {
                session.history = Some(self.document.begin_history_group(name(diameter))?);
            }
            measurement.finish_grouped(
                &mut self.document,
                session.history.as_mut().unwrap(),
                display_units,
            )
        })();
        match result {
            Ok(report) => {
                self.snaps.model_override = None;
                self.push_log(report);
                self.log_radius_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn finish_radius(
        &mut self,
        point: Point3,
        diameter: bool,
        mark: bool,
        units: Option<&'static str>,
    ) -> bool {
        if self
            .radius_session
            .as_ref()
            .is_some_and(|s| s.selecting || s.choosing_units)
        {
            self.log_radius_prompt();
            return false;
        }
        let session = self.radius_session.as_ref();
        let result = if let Some(source) = session.and_then(|s| s.source.as_ref()) {
            (|| {
                if !source.curve.current(&self.document) {
                    return Err(CommandError::Usage("The measurement source changed"));
                }
                let curve = source.curve_ref();
                let parameter = curve.closest_parameter(point, self.document.tolerance())?;
                RadiusMeasurement::evaluate(curve, parameter)
            })()
        } else {
            RadiusMeasurement::nearest(
                &self.document,
                point,
                self.document.selected_object_count() != 0
                    && session.is_none_or(|s| !s.unrestricted),
            )
        };
        if self.measuring_nested_radius() {
            return self.accept_nested_radius(result);
        }
        self.finish_radius_measurement(result, diameter, mark, units, point)
    }

    fn finish_radius_measurement(
        &mut self,
        result: Result<RadiusMeasurement, CommandError>,
        diameter: bool,
        mark: bool,
        units: Option<&'static str>,
        point: Point3,
    ) -> bool {
        let input = radius_command_line(point, diameter, mark, units);
        let result = result.and_then(|measurement| {
            if mark
                && let Some(group) = self
                    .radius_session
                    .as_mut()
                    .and_then(|s| s.history.as_mut())
            {
                measurement.finish_grouped(&mut self.document, group, units)
            } else {
                measurement.finish(&mut self.document, diameter, mark, units)
            }
        });
        self.report_radius_result(result, input)
    }

    fn report_radius_result(
        &mut self,
        result: Result<String, CommandError>,
        input: String,
    ) -> bool {
        match result {
            Ok(report) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(report);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}

fn radius_command_line(point: Point3, diameter: bool, mark: bool, units: Option<&str>) -> String {
    let name = name(diameter);
    format!(
        "{name} Mark{name}={} {}{}",
        if mark { "Yes" } else { "No" },
        format_model_point(point),
        units
            .map(|unit| format!(" Units={unit}"))
            .unwrap_or_default()
    )
}

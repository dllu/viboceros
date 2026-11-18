//! Shared state for a draft containing straight and circular segments.
mod options;
pub(crate) mod segments;
pub use segments::Segment;

use viboceros_document::Geometry;
use viboceros_geometry::{
    CircularArc3, Frame3, GeometryError, MAX_POLYCURVE_SEGMENTS, Point3, Tolerance, UnitVector3,
};

pub const USAGE: &str =
    "Polyline [PersistentClose=Yes|No] start [Mode=Line|Arc] points-or-options [Enter|Close]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Line,
    Arc,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    mode: Mode,
    persistent_close: bool,
    helpers: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Next {
    #[default]
    End,
    Direction,
    ThroughPoint,
    Center,
    TangentEnd(UnitVector3),
    ThroughEnd(Point3),
    CenterEnd(Point3),
    AlongLength,
}

#[derive(Clone, Debug)]
pub struct Input {
    plane: Frame3,
    preferences: Preferences,
    first: Option<Point3>,
    segments: Vec<Segment>,
    next: Next,
    length: Option<f64>,
}

#[derive(Clone, Copy)]
pub enum Value {
    Distance(f64),
    Radius(f64),
    Angle(f64),
}

pub enum OptionResult {
    Updated,
    Finish { close: bool },
    Undo,
}

#[derive(Clone, Copy, Default)]
pub struct Preview {
    pub guide: Option<[Point3; 2]>,
    pub next: Option<Segment>,
    pub closing: Option<Segment>,
}

impl Input {
    pub fn new(plane: Frame3, preferences: Preferences) -> Self {
        Self {
            plane,
            preferences,
            first: None,
            segments: Vec::new(),
            next: Next::End,
            length: None,
        }
    }

    pub fn preferences(&self) -> Preferences {
        self.preferences
    }

    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.first.is_none() {
            self.plane = plane;
        }
    }

    pub fn plane(&self) -> Frame3 {
        self.plane
    }

    pub fn first(&self) -> Option<Point3> {
        self.first
    }

    pub fn anchor(&self) -> Option<Point3> {
        self.segments
            .last()
            .and_then(|s| s.end().ok())
            .or(self.first)
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn helper_tracking(&self) -> bool {
        self.preferences.helpers
            && !matches!(
                self.next,
                Next::Direction | Next::Center | Next::ThroughPoint | Next::AlongLength
            )
            && self.first.is_some()
    }

    pub fn helper_directions(&self) -> [Option<UnitVector3>; 3] {
        let tangent = match self.next {
            Next::TangentEnd(tangent) => Some(tangent),
            _ => self.end_tangent(),
        };
        [
            tangent,
            Some(self.plane.x_axis()),
            Some(self.plane.y_axis()),
        ]
    }

    pub fn end_tangent(&self) -> Option<UnitVector3> {
        self.segments.last().and_then(|s| s.tangent().ok())
    }

    pub fn value(
        &mut self,
        value: Value,
        tolerance: Tolerance,
    ) -> Result<Option<Geometry>, GeometryError> {
        match value {
            Value::Distance(number) | Value::Radius(number) => {
                let distance = if matches!(value, Value::Distance(_)) && number < 0. {
                    -2. * number
                } else {
                    number
                };
                if !distance.is_finite() || distance <= tolerance.absolute() {
                    return Err(GeometryError::Degenerate {
                        context: "polyline segment length",
                    });
                }
                self.length = Some(distance);
                // A typed radius replaces a through-point reference.
                if matches!(self.next, Next::ThroughEnd(_)) {
                    self.next = Next::End;
                }
                Ok(None)
            }
            Value::Angle(degrees) => {
                let Next::CenterEnd(center) = self.next else {
                    return Ok(None);
                };
                let anchor = self.anchor().unwrap();
                let tangent = self
                    .end_tangent()
                    .filter(|_| degrees > 0. && degrees < 360.)
                    .ok_or(GeometryError::Degenerate {
                        context: "polyline center arc angle",
                    })?;
                let segment = Segment::angle_arc(center, anchor, degrees, tangent, tolerance)?;
                self.append(segment, false, tolerance)
            }
        }
    }

    pub fn pick(
        &mut self,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<Option<Geometry>, GeometryError> {
        let Some(anchor) = self.anchor() else {
            self.first = Some(point);
            return Ok(None);
        };
        match self.next {
            Next::Direction => {
                self.next = Next::TangentEnd(anchor.vector_to(point)?.normalized(tolerance)?);
                Ok(None)
            }
            Next::Center | Next::ThroughPoint => {
                if anchor.distance_to(point)? <= tolerance.absolute() {
                    return Err(GeometryError::Degenerate {
                        context: "polyline arc reference",
                    });
                }
                self.next = if self.next == Next::Center {
                    Next::CenterEnd(point)
                } else {
                    Next::ThroughEnd(point)
                };
                Ok(None)
            }
            _ => {
                let first = self.first.unwrap();
                let closes = self.segments.len() >= 2 && seams_match(first, point);
                let segment = if closes {
                    self.closing_segment(
                        anchor,
                        first,
                        self.next,
                        self.end_tangent(),
                        self.length,
                        tolerance,
                    )?
                } else {
                    self.next_segment(anchor, point, tolerance)?
                };
                self.append(segment, closes, tolerance)
            }
        }
    }

    fn append(
        &mut self,
        segment: Segment,
        closes: bool,
        tolerance: Tolerance,
    ) -> Result<Option<Geometry>, GeometryError> {
        if self.segments.len() >= MAX_POLYCURVE_SEGMENTS {
            return Err(GeometryError::InvalidPolyCurve {
                context: "too many polyline segments",
            });
        }
        let first = self.first.unwrap();
        if self.segments.len() >= 2 && closes {
            return segments::geometry(first, &self.segments, Some(segment), tolerance).map(Some);
        }
        self.segments.push(segment);
        self.reset_next();
        Ok(None)
    }

    pub fn finish(&self, close: bool, tolerance: Tolerance) -> Result<Geometry, GeometryError> {
        let min = if close { 2 } else { 1 };
        if self.segments.len() < min {
            return Err(GeometryError::Degenerate {
                context: "too few polyline points",
            });
        }
        let first = self.first.unwrap();
        let last = self.anchor().unwrap();
        let closing = (last != first
            && (close || (self.preferences.persistent_close && self.segments.len() >= 2)))
            .then(|| {
                self.closing_segment(
                    last,
                    first,
                    self.next,
                    self.end_tangent(),
                    self.length,
                    tolerance,
                )
            })
            .transpose()?;
        segments::geometry(first, &self.segments, closing, tolerance)
    }

    pub fn preview(&self, point: Point3, tolerance: Tolerance) -> Preview {
        let Some(anchor) = self.anchor() else {
            return Preview::default();
        };
        if matches!(
            self.next,
            Next::Direction | Next::ThroughPoint | Next::Center
        ) {
            return Preview {
                guide: Some([anchor, point]),
                ..Preview::default()
            };
        }
        let first = self.first.unwrap();
        let next = if self.segments.len() >= 2 && seams_match(first, point) {
            self.closing_segment(
                anchor,
                first,
                self.next,
                self.end_tangent(),
                self.length,
                tolerance,
            )
            .ok()
        } else {
            self.next_segment(anchor, point, tolerance).ok()
        };
        let closing = if self.preferences.persistent_close && !self.segments.is_empty() {
            next.and_then(|segment| {
                let end = segment.end().ok()?;
                if seams_match(end, first) {
                    return None;
                }
                self.closing_segment(
                    end,
                    first,
                    Next::End,
                    segment.tangent().ok(),
                    None,
                    tolerance,
                )
                .ok()
            })
        } else {
            None
        };
        Preview {
            guide: None,
            next,
            closing,
        }
    }

    pub fn persistent_preview(&self, tolerance: Tolerance) -> Option<Segment> {
        if !self.preferences.persistent_close || self.segments.len() < 2 {
            return None;
        }
        self.closing_segment(
            self.anchor()?,
            self.first?,
            self.next,
            self.end_tangent(),
            self.length,
            tolerance,
        )
        .ok()
    }

    fn closing_segment(
        &self,
        last: Point3,
        first: Point3,
        next: Next,
        tangent: Option<UnitVector3>,
        radius: Option<f64>,
        tolerance: Tolerance,
    ) -> Result<Segment, GeometryError> {
        if self.preferences.mode == Mode::Arc {
            self.arc_segment(last, first, next, tangent, radius, tolerance)
        } else {
            Segment::line(last, first, tolerance)
        }
    }

    fn next_segment(
        &self,
        start: Point3,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<Segment, GeometryError> {
        if self.preferences.mode == Mode::Line {
            let end = if self.next == Next::AlongLength {
                let direction = self
                    .end_tangent()
                    .unwrap_or(self.plane.x_axis())
                    .as_vector();
                let along = start.vector_to(point)?.dot(direction)?;
                let length = self.length.map_or(along, |length| length.copysign(along));
                start.translated(direction.scaled(length)?)?
            } else if let Some(length) = self.length {
                start.translated(start.direction_to(point)?.as_vector().scaled(length)?)?
            } else {
                point
            };
            return Segment::line(start, end, tolerance);
        }
        self.arc_segment(
            start,
            point,
            self.next,
            self.end_tangent(),
            self.length,
            tolerance,
        )
    }

    fn arc_segment(
        &self,
        start: Point3,
        point: Point3,
        next: Next,
        incoming: Option<UnitVector3>,
        radius: Option<f64>,
        tolerance: Tolerance,
    ) -> Result<Segment, GeometryError> {
        match next {
            Next::ThroughEnd(through) => Ok(Segment::Arc(CircularArc3::try_from_three_points(
                start, through, point, tolerance,
            )?)),
            Next::CenterEnd(center) => {
                Segment::centered_arc(center, start, point, self.plane, incoming, tolerance)
            }
            next => {
                let tangent = match next {
                    Next::TangentEnd(tangent) => tangent,
                    _ => incoming.unwrap_or(self.plane.x_axis()),
                };
                if let Some(radius) = radius {
                    Segment::radius_arc(start, tangent, point, radius, tolerance)
                } else {
                    Segment::tangent_arc(start, tangent, point, tolerance)
                }
            }
        }
    }

    fn undo(&mut self) {
        if self.segments.pop().is_none() {
            self.first = None;
        }
        self.reset_next();
    }

    fn reset_next(&mut self) {
        self.next = Next::End;
        self.length = None;
    }
}

fn seams_match(a: Point3, b: Point3) -> bool {
    const ZERO_TOLERANCE: f64 = 2.3283064365386963e-10; // 2^-32
    (a.x() - b.x()).abs() <= ZERO_TOLERANCE
        && (a.y() - b.y()).abs() <= ZERO_TOLERANCE
        && (a.z() - b.z()).abs() <= ZERO_TOLERANCE
}

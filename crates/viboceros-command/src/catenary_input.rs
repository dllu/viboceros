//! Edit-free Catenary getters shared by the frontend and native oracle recipes.
use crate::CommandError;
use viboceros_geometry::{
    CatenaryConstruction, CatenaryCurve, CatenaryOutput, CatenarySolution,
    DEFAULT_CATENARY_POINT_COUNT, Frame3, GeometryError, MAX_CATENARY_POINT_COUNT,
    MIN_SMOOTH_CATENARY_POINT_COUNT, Point3, Real, Tolerance, Vector3, try_catenary,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    ThroughPoint,
    Length,
    Parameter,
    Apex,
}
impl Mode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ThroughPoint => "ThroughPoint",
            Self::Length => "Length",
            Self::Parameter => "Parameter",
            Self::Apex => "Apex",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub output: CatenaryOutput,
    pub point_count: usize,
    pub mark_apex: bool,
    pub length: Option<Real>,
    pub parameter: Option<Real>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::ThroughPoint,
            output: CatenaryOutput::Smooth,
            point_count: DEFAULT_CATENARY_POINT_COUNT,
            mark_apex: false,
            length: None,
            parameter: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    Start,
    End,
    Axis,
    Constraint,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    options: Options,
    plane: Frame3,
    tolerance: Tolerance,
    start: Option<Point3>,
    end: Option<Point3>,
    axis_point: Option<Point3>,
    construction: Option<CatenaryConstruction>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub curve: CatenaryCurve,
    pub apex: Option<Point3>,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
impl Input {
    pub fn new(options: Options, plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            options: Options {
                point_count: options.point_count.max(MIN_SMOOTH_CATENARY_POINT_COUNT),
                ..options
            },
            plane,
            tolerance,
            start: None,
            end: None,
            axis_point: None,
            construction: None,
        }
    }
    pub fn options(self) -> Options {
        self.options
    }
    pub fn slot(self) -> Slot {
        if self.construction.is_some() {
            Slot::Complete
        } else if self.start.is_none() {
            Slot::Start
        } else if self.end.is_none() {
            Slot::End
        } else if self.axis_point.is_none() {
            Slot::Axis
        } else {
            Slot::Constraint
        }
    }
    pub fn anchor(self) -> Option<Point3> {
        match self.slot() {
            Slot::Start => None,
            Slot::End | Slot::Axis => self.start,
            Slot::Constraint if matches!(self.options.mode, Mode::Length | Mode::Parameter) => {
                self.start
            }
            _ => self.end,
        }
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.start.is_none() {
            self.plane = plane;
        }
    }
    pub fn markers(self) -> Vec<Point3> {
        [self.start, self.end, self.axis_point]
            .into_iter()
            .flatten()
            .collect()
    }
    pub fn default_axis(self) -> Result<Point3, GeometryError> {
        self.start
            .ok_or_else(|| invalid("catenary start is required"))?
            .translated(self.plane.y_axis().as_vector().scaled(-1.)?)
    }
    pub fn mouse_plane(self) -> Frame3 {
        if let (Some(start), Some(end), Some(axis)) = (self.start, self.end, self.axis_point)
            && let Ok(plane) = Frame3::try_from_directions(
                start,
                start.vector_to(end).unwrap(),
                start.vector_to(axis).unwrap(),
                self.tolerance,
            )
        {
            return plane;
        }
        self.plane
            .with_origin(self.anchor().unwrap_or(self.plane.origin()))
    }
    pub fn axis_guide(self) -> Option<[Point3; 2]> {
        (self.slot() == Slot::Axis)
            .then(|| Some([self.start?, self.default_axis().ok()?]))
            .flatten()
    }
    pub fn option(&mut self, word: &str) -> Result<bool, GeometryError> {
        if self.slot() == Slot::Complete {
            return Err(invalid("catenary is complete"));
        }
        let (key, value) = word
            .trim_start_matches(['_', '-'])
            .split_once('=')
            .map_or((word.trim_start_matches(['_', '-']), None), |(k, v)| {
                (k, Some(v.trim_start_matches('_')))
            });
        let mut next = *self;
        if key.eq_ignore_ascii_case("Mode") {
            next.options.mode = match value {
                Some(v) if v.eq_ignore_ascii_case("ThroughPoint") => Mode::ThroughPoint,
                Some(v) if v.eq_ignore_ascii_case("Length") => Mode::Length,
                Some(v) if v.eq_ignore_ascii_case("Parameter") => Mode::Parameter,
                Some(v) if v.eq_ignore_ascii_case("Apex") => Mode::Apex,
                _ => {
                    return Err(invalid(
                        "Mode requires ThroughPoint, Length, Parameter or Apex",
                    ));
                }
            };
        } else if key.eq_ignore_ascii_case("Output") {
            next.options.output = match value {
                None => {
                    if self.options.output == CatenaryOutput::Smooth {
                        CatenaryOutput::Polyline
                    } else {
                        CatenaryOutput::Smooth
                    }
                }
                Some(v) if v.eq_ignore_ascii_case("Smooth") => CatenaryOutput::Smooth,
                Some(v) if v.eq_ignore_ascii_case("Polyline") => CatenaryOutput::Polyline,
                _ => return Err(invalid("Output requires Smooth or Polyline")),
            };
        } else if key.eq_ignore_ascii_case("PointCount") {
            next.options.point_count = value
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| invalid("PointCount requires an integer"))?;
        } else if key.eq_ignore_ascii_case("MarkApex") {
            next.options.mark_apex = match value {
                None => !next.options.mark_apex,
                Some(v) if v.eq_ignore_ascii_case("Yes") => true,
                Some(v) if v.eq_ignore_ascii_case("No") => false,
                _ => return Err(invalid("MarkApex requires Yes or No")),
            };
        } else {
            return Ok(false);
        }
        next.validate_count()?;
        *self = next;
        Ok(true)
    }
    fn validate_count(self) -> Result<(), GeometryError> {
        let minimum = MIN_SMOOTH_CATENARY_POINT_COUNT;
        if !(minimum..=MAX_CATENARY_POINT_COUNT).contains(&self.options.point_count) {
            return Err(GeometryError::InvalidCatenaryPointCount {
                actual: self.options.point_count,
                minimum,
                maximum: MAX_CATENARY_POINT_COUNT,
            });
        }
        Ok(())
    }
    pub fn scalar_default(self) -> Option<Real> {
        if self.options.mode == Mode::Length {
            self.options.length
        } else {
            self.options.parameter
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<bool, GeometryError> {
        self.point_impl(point, true)
    }
    fn point_impl(&mut self, point: Point3, validate: bool) -> Result<bool, GeometryError> {
        let mut next = *self;
        match self.slot() {
            Slot::Start => next.start = Some(point),
            Slot::End => {
                if self.start.unwrap().distance_to(point)? <= self.tolerance.absolute() {
                    return Err(invalid("catenary endpoints must be distinct"));
                }
                next.end = Some(point);
            }
            Slot::Axis => {
                let start = self.start.unwrap();
                let axis = start
                    .vector_to(point)?
                    .normalized(self.tolerance)?
                    .as_vector();
                let span = start.vector_to(self.end.unwrap())?;
                if span.cross(axis)?.length()? <= self.tolerance.absolute() {
                    return Err(invalid("catenary axis must differ from its endpoint chord"));
                }
                next.axis_point = Some(point);
            }
            Slot::Constraint if matches!(self.options.mode, Mode::ThroughPoint | Mode::Apex) => {
                let start = self.start.unwrap();
                let original = start.vector_to(self.axis_point.unwrap())?;
                let (axis, construction) = orient_constraint(
                    start,
                    self.end.unwrap(),
                    original,
                    picked_constraint(self.options.mode, point),
                    self.tolerance,
                )?;
                if axis != original {
                    next.axis_point = Some(start.translated(axis)?);
                }
                next.construction = Some(construction);
            }
            Slot::Constraint => {
                let start = self.start.unwrap();
                let offset = start.vector_to(point)?;
                let axis = start.vector_to(self.axis_point.unwrap())?;
                if offset.dot(axis)? < 0. {
                    next.axis_point = Some(start.translated(axis.scaled(-1.)?)?);
                }
                next.set_scalar(start.distance_to(point)?)?;
            }
            Slot::Complete => return Err(invalid("catenary is complete")),
        }
        if next.slot() == Slot::Complete && validate {
            next.accept_solution()?;
        }
        *self = next;
        Ok(self.slot() == Slot::Complete)
    }
    pub fn number(&mut self, value: Real) -> Result<bool, GeometryError> {
        let mut next = *self;
        next.set_scalar(value)?;
        next.accept_solution()?;
        *self = next;
        Ok(true)
    }
    fn set_scalar(&mut self, value: Real) -> Result<(), GeometryError> {
        if self.slot() != Slot::Constraint || !value.is_finite() || value == 0. {
            return Err(invalid("catenary requires a nonzero length or parameter"));
        }
        let construction = match self.options.mode {
            Mode::Length => CatenaryConstruction::Length(value),
            Mode::Parameter => CatenaryConstruction::Parameter(value),
            _ => {
                return Err(invalid(
                    "choose Length or Parameter before entering a number",
                ));
            }
        };
        let start = self.start.unwrap();
        let original = start.vector_to(self.axis_point.unwrap())?;
        let (axis, construction) = orient_constraint(
            start,
            self.end.unwrap(),
            original,
            construction,
            self.tolerance,
        )?;
        if axis != original {
            self.axis_point = Some(start.translated(axis)?);
        }
        self.construction = Some(construction);
        Ok(())
    }
    pub fn relative_base(self) -> Option<Point3> {
        (self.slot() == Slot::Axis
            || self.slot() == Slot::Constraint
                && matches!(self.options.mode, Mode::Length | Mode::Parameter))
        .then_some(self.start)
        .flatten()
    }
    pub fn enter_cancels(self) -> bool {
        match self.slot() {
            Slot::Start | Slot::End => true,
            Slot::Constraint => match self.options.mode {
                Mode::Length | Mode::Parameter => self.scalar_default().is_none(),
                Mode::Apex => true,
                Mode::ThroughPoint => !self
                    .start
                    .unwrap()
                    .vector_to(self.end.unwrap())
                    .and_then(|chord| {
                        self.start
                            .unwrap()
                            .vector_to(self.axis_point.unwrap())
                            .and_then(|axis| chord.dot(axis))
                    })
                    .is_ok_and(|height| height > 0.),
            },
            _ => false,
        }
    }
    pub fn enter(&mut self) -> Result<bool, GeometryError> {
        match self.slot() {
            Slot::Axis => self.point(self.default_axis()?),
            Slot::Constraint if matches!(self.options.mode, Mode::Length | Mode::Parameter) => self
                .number(
                    self.scalar_default()
                        .ok_or_else(|| invalid("no remembered catenary value"))?,
                ),
            Slot::Constraint if self.options.mode == Mode::ThroughPoint => {
                let mut next = *self;
                next.construction = Some(CatenaryConstruction::Apex(self.end.unwrap()));
                next.accept_solution()?;
                *self = next;
                Ok(true)
            }
            _ => Err(invalid("pick a catenary point")),
        }
    }
    fn accept_solution(&mut self) -> Result<(), GeometryError> {
        let solution = self.solution()?;
        self.options.parameter = Some(solution.parameter());
        self.options.length = Some(solution.length());
        Ok(())
    }
    pub fn solution(self) -> Result<CatenarySolution, GeometryError> {
        let start = self
            .start
            .ok_or_else(|| invalid("catenary start is required"))?;
        let end = self
            .end
            .ok_or_else(|| invalid("catenary end is required"))?;
        let axis = self
            .axis_point
            .ok_or_else(|| invalid("catenary axis is required"))?;
        let construction = self
            .construction
            .ok_or_else(|| invalid("catenary constraint is required"))?;
        try_catenary(
            start,
            end,
            start.vector_to(axis)?,
            construction,
            self.options.output,
            self.options.point_count,
            self.tolerance,
        )
    }
    pub fn preview(self, cursor: Point3) -> Option<Preview> {
        if self.slot() != Slot::Constraint {
            return None;
        }
        let mut next = self;
        next.point_impl(cursor, false).ok()?;
        let solution = next.solution().ok()?;
        Some(Preview {
            apex: self.options.mark_apex.then_some(solution.apex()),
            curve: solution.into_curve(),
        })
    }
    pub fn command(self) -> Result<String, CommandError> {
        let point = |p: Point3| {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        };
        let value = match self
            .construction
            .ok_or_else(|| invalid("catenary constraint is required"))?
        {
            CatenaryConstruction::Length(v) | CatenaryConstruction::Parameter(v) => v.to_string(),
            CatenaryConstruction::ThroughPoint(p) | CatenaryConstruction::Apex(p) => point(p),
        };
        Ok(format!(
            "Catenary {} {} {} {} Mode={} Output={} PointCount={} MarkApex={}",
            point(self.start.unwrap()),
            point(self.end.unwrap()),
            point(self.axis_point.unwrap()),
            value,
            if self.options.mode == Mode::ThroughPoint
                && matches!(self.construction, Some(CatenaryConstruction::Apex(_)))
            {
                Mode::Apex.name()
            } else {
                self.options.mode.name()
            },
            if self.options.output == CatenaryOutput::Smooth {
                "Smooth"
            } else {
                "Polyline"
            },
            self.options.point_count,
            if self.options.mark_apex { "Yes" } else { "No" }
        ))
    }
}
pub(crate) fn picked_constraint(mode: Mode, point: Point3) -> CatenaryConstruction {
    if mode == Mode::Apex {
        CatenaryConstruction::Apex(point)
    } else {
        CatenaryConstruction::ThroughPoint(point)
    }
}

/// Normalize signed scalar constraints and choose the side of a picked sag.
/// Through points choose their side relative to the endpoint chord; apex
/// heights choose their side relative to the start.
pub(crate) fn orient_constraint(
    start: Point3,
    end: Point3,
    axis: Vector3,
    construction: CatenaryConstruction,
    tolerance: Tolerance,
) -> Result<(Vector3, CatenaryConstruction), GeometryError> {
    let (reverse, construction) = match construction {
        CatenaryConstruction::Length(n) => (n < 0., CatenaryConstruction::Length(n.abs())),
        CatenaryConstruction::Parameter(n) => (n < 0., CatenaryConstruction::Parameter(n.abs())),
        CatenaryConstruction::Apex(point) => {
            (start.vector_to(point)?.dot(axis)? < 0., construction)
        }
        CatenaryConstruction::ThroughPoint(point) => {
            let chord = start.vector_to(end)?.normalized(tolerance)?.as_vector();
            let offset = start.vector_to(point)?;
            let side = offset.dot(axis)? - offset.dot(chord)? * axis.dot(chord)?;
            (side < 0., construction)
        }
    };
    Ok((if reverse { axis.scaled(-1.)? } else { axis }, construction))
}

#[cfg(test)]
mod tests;

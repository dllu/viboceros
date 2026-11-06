use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Default,
    Vertical,
    FourPoint,
    Bisector,
    Angled,
    Extension,
    Normal,
}
#[derive(Clone, Copy, Debug)]
pub struct Definition {
    pub line: LineSegment,
    pub accepted_point: Point3,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    plane: Frame3,
    mode: Mode,
    both: bool,
    ignore_trims: bool,
    points: [Option<Point3>; 3],
    angle: Option<Real>,
    distance: Option<viboceros_drafting::PointConstraintState>,
    cancelled: bool,
    extension_line: Option<(Point3, Point3)>,
    normal_plane: Option<UnitVector3>,
    cursor: Option<Point3>,
}
impl Input {
    pub fn new(plane: Frame3) -> Self {
        Self {
            plane,
            mode: Mode::Default,
            both: false,
            ignore_trims: false,
            points: [None; 3],
            angle: None,
            distance: None,
            cancelled: false,
            extension_line: None,
            normal_plane: None,
            cursor: None,
        }
    }
    pub fn mode(self) -> Mode {
        self.mode
    }
    pub fn both_sides(self) -> bool {
        self.both
    }
    pub fn ignore_trims(self) -> bool {
        self.ignore_trims
    }
    pub fn wants_normal_origin(self) -> bool {
        self.mode == Mode::Normal && self.points[0].is_none()
    }
    pub fn normal(&mut self, location: super::normal::Location) -> Result<(), CommandError> {
        if !self.wants_normal_origin() {
            return Err(CommandError::Usage(USAGE));
        }
        let (axis, plane) = match location.constraint {
            super::normal::Constraint::Axis(normal) => (
                Some((
                    location.point,
                    location.point.translated(normal.as_vector())?,
                )),
                None,
            ),
            super::normal::Constraint::Plane(tangent) => (None, Some(tangent)),
        };
        self.points[0] = Some(location.point);
        self.extension_line = axis;
        self.normal_plane = plane;
        Ok(())
    }
    pub fn mouse_plane(self) -> Option<Frame3> {
        Frame3::try_from_normal(
            self.anchor()?,
            self.normal_plane?.as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .ok()
    }
    pub fn plane(self) -> Frame3 {
        self.plane
    }
    pub fn points(self) -> [Option<Point3>; 3] {
        self.points
    }
    fn count(self) -> usize {
        self.points.into_iter().flatten().count()
    }
    pub fn anchor(self) -> Option<Point3> {
        match self.mode {
            Mode::FourPoint if self.points[2].is_some() => self.points[2],
            Mode::Angled if self.angle.is_some() => self.reference().ok().flatten().map(|r| r.0),
            _ => self.points[0],
        }
    }
    pub fn requests_angle(self) -> bool {
        self.mode == Mode::Angled && self.count() == 2 && self.angle.is_none()
    }
    pub fn prompt(self) -> &'static str {
        match (self.mode, self.count()) {
            (Mode::Default, 0) if self.both => "middle point of line",
            (Mode::Normal, 0) => {
                "select a normal reference, then a start point on it; IgnoreTrims=Yes|No"
            }
            (Mode::Extension, 0) => "pick a curve or surface edge near its end",
            (Mode::Default | Mode::Vertical, 0) => {
                "start point; BothSides, Vertical, FourPoint, Bisector, Angled, Extension, Normal or Perpendicular"
            }
            (Mode::FourPoint, 0) => "start of reference line",
            (Mode::FourPoint, 1) => "end of reference line",
            (Mode::FourPoint, 2) => "start of new line",
            (Mode::Bisector, 0) => "start of bisector",
            (Mode::Bisector, 1) => "start of angle",
            (Mode::Bisector, 2) => "end of angle",
            (Mode::Angled, 0) => "start of reference line",
            (Mode::Angled, 1) => "end of reference line",
            _ if self.requests_angle() => "pivot angle in degrees",
            _ => "end point or length; BothSides",
        }
    }
    pub fn option(&mut self, text: &str) -> Result<bool, CommandError> {
        let word = text.trim_start_matches(['_', '-']);
        if let Some((key, value)) = word.split_once('=') {
            if key.eq_ignore_ascii_case("BothSides") {
                self.both = parse_yes_no(value.trim_start_matches('_'))
                    .ok_or(CommandError::Usage(USAGE))?;
                return Ok(true);
            }
            if key.eq_ignore_ascii_case("IgnoreTrims") && self.wants_normal_origin() {
                self.ignore_trims = parse_yes_no(value.trim_start_matches('_'))
                    .ok_or(CommandError::Usage(USAGE))?;
                return Ok(true);
            }
            return Ok(false);
        }
        if word.eq_ignore_ascii_case("IgnoreTrims") && self.wants_normal_origin() {
            self.ignore_trims = !self.ignore_trims;
            return Ok(true);
        }
        if word.eq_ignore_ascii_case("BothSides") {
            if self.both {
                return Err(CommandError::Usage("BothSides is already active"));
            }
            self.both = true;
            return Ok(true);
        }
        if self.count() != 0 {
            return Ok(false);
        }
        let mode = if word.eq_ignore_ascii_case("Vertical") {
            Mode::Vertical
        } else if word.eq_ignore_ascii_case("FourPoint") {
            Mode::FourPoint
        } else if word.eq_ignore_ascii_case("Bisector") {
            Mode::Bisector
        } else if word.eq_ignore_ascii_case("Angled") {
            Mode::Angled
        } else if word.eq_ignore_ascii_case("Normal") {
            Mode::Normal
        } else if word.eq_ignore_ascii_case("Extension") {
            Mode::Extension
        } else {
            return Ok(false);
        };
        self.mode = mode;
        self.angle = None;
        self.distance = None;
        Ok(true)
    }
    pub fn wants_curve(self) -> bool {
        self.mode == Mode::Extension && self.points[0].is_none()
    }
    pub fn extension(&mut self, curve: CurveRef<'_>, pick: Point3) -> Result<(), CommandError> {
        if !self.wants_curve() {
            return Err(CommandError::Usage(
                "Line Extension already has a reference",
            ));
        }
        // Compare distance along the source. Unequal polyline segments and
        // folded curves distinguish this from parameter or endpoint distance.
        let station = curve.closest_parameter(pick, Tolerance::NUMERICAL_VALIDATION)?;
        let lengths = viboceros_geometry::ArcLengthSampler::try_new(curve, Tolerance::DEFAULT)?;
        let at_end = lengths.distance_at_parameter(station)? > 0.5 * lengths.total_length();
        let parameter = if at_end {
            *curve.domain().end()
        } else {
            *curve.domain().start()
        };
        let sample = curve.evaluate_with_tangent(parameter)?;
        let direction = if at_end {
            sample.tangent()
        } else {
            sample.tangent().opposite()
        };
        let start = sample.point();
        let end = start.translated(direction.as_vector())?;
        self.points[0] = Some(start);
        self.extension_line = Some((start, end));
        self.cursor = Some(pick);
        Ok(())
    }
    pub fn observe_cursor(&mut self, point: Point3) {
        self.cursor = Some(point);
    }
    /// A constrained Extension length accepts the remembered cursor on Enter.
    /// Other Line getters keep their existing cancellation policy here.
    pub fn enter(self) -> Option<Definition> {
        (self.mode == Mode::Extension && self.distance.is_some())
            .then(|| self.preview(self.cursor?))
            .flatten()
    }
    /// The actual finite constraint line is significant at remote coordinates.
    /// Do not replace its endpoints with a normalized direction before projecting.
    fn reference(self) -> Result<Option<(Point3, Point3)>, CommandError> {
        let Some(origin) = self.points[0] else {
            return Ok(None);
        };
        Ok(match self.mode {
            Mode::Extension | Mode::Normal => self.extension_line,
            Mode::Vertical => Some((origin, origin.translated(self.plane.z_axis().as_vector())?)),
            Mode::FourPoint if self.count() == 2 => Some((origin, self.points[1].unwrap())),
            Mode::FourPoint if self.count() == 3 => {
                let start = self.points[2].unwrap();
                let direction = origin.direction_to(self.points[1].unwrap())?;
                Some((start, start.translated(direction.as_vector())?))
            }
            Mode::Bisector if self.count() == 3 && !self.cancelled => {
                let a = origin.direction_to(self.points[1].unwrap())?.as_vector();
                let b = origin.direction_to(self.points[2].unwrap())?.as_vector();
                let sum = Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())?;
                Some((origin, origin.translated(sum)?))
            }
            Mode::Angled if self.count() == 2 && self.angle.is_some() => {
                Some(super::native_rotation::reference_line(
                    origin,
                    self.points[1].unwrap(),
                    self.plane.z_axis(),
                    self.angle.unwrap().to_radians(),
                )?)
            }
            _ => None,
        })
    }
    pub fn direction(self) -> Result<Option<UnitVector3>, CommandError> {
        self.reference()?
            .map(|(a, b)| a.direction_to(b).map_err(Into::into))
            .transpose()
    }
    pub fn cancelled(self) -> bool {
        self.cancelled
    }
    pub fn resolve_point(self, point: Point3) -> Result<Point3, CommandError> {
        if let Some(tangent) = self.normal_plane {
            let distance = self
                .anchor()
                .unwrap()
                .vector_to(point)?
                .dot(tangent.as_vector())?;
            return Ok(point.translated(tangent.as_vector().scaled(-distance)?)?);
        }
        if let Some((a, b)) = self.reference()? {
            return Ok(crate::native_line_projection::closest_point(a, b, point)?.0);
        }
        Ok(point)
    }
    pub fn last_point(self) -> Option<Point3> {
        self.points.into_iter().flatten().last()
    }
    pub fn endpoint(self) -> bool {
        self.count()
            == match self.mode {
                Mode::Default | Mode::Vertical | Mode::Extension | Mode::Normal => 1,
                Mode::Angled => 2,
                _ => 3,
            }
            && !self.requests_angle()
    }
    fn complete(self, point: Point3) -> Result<Definition, CommandError> {
        let origin = self.anchor().ok_or(CommandError::Usage(USAGE))?;
        let point = self.resolve_point(point)?;
        let point = if let Some(distance) = self.distance {
            distance.apply_typed(point).map_err(|_| {
                CommandError::Usage("Line distance cannot be resolved at this point")
            })?
        } else {
            point
        };
        super::segment(origin, point)?;
        let start = if self.both {
            origin.translated(point.vector_to(origin)?)?
        } else {
            origin
        };
        let line = super::segment(start, point)?;
        Ok(Definition {
            line,
            accepted_point: point,
        })
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, CommandError> {
        let count = self.count();
        if self.wants_normal_origin() {
            return Err(CommandError::Usage(
                "Choose a Line Normal reference and start point first",
            ));
        }
        if self.wants_curve() {
            return Err(CommandError::Usage("Pick a Line Extension reference first"));
        }
        if self.requests_angle() {
            return Err(CommandError::Usage(
                "Line Angled: enter an angle before the endpoint",
            ));
        }
        let required = match self.mode {
            Mode::Default | Mode::Vertical | Mode::Extension | Mode::Normal => 1,
            Mode::Angled => 2,
            _ => 3,
        };
        if count == required {
            return self.complete(point).map(Some);
        }
        if count > 0 {
            let previous = if self.mode == Mode::Bisector {
                self.points[0].unwrap()
            } else {
                self.points[count - 1].unwrap()
            };
            if self.mode != Mode::FourPoint || count != 2 {
                super::segment(previous, point)?;
            }
        }
        let mut next = *self;
        next.points[count] = Some(if self.mode == Mode::FourPoint && count == 2 {
            self.resolve_point(point)?
        } else {
            point
        });
        if next.mode == Mode::Bisector && next.count() == 3 {
            let a = next.points[0]
                .unwrap()
                .direction_to(next.points[1].unwrap())?
                .as_vector();
            let b = next.points[0]
                .unwrap()
                .direction_to(next.points[2].unwrap())?
                .as_vector();
            next.cancelled = a.x() + b.x() == 0. && a.y() + b.y() == 0. && a.z() + b.z() == 0.;
        }
        *self = next;
        Ok(None)
    }
    pub fn number(&mut self, value: Real) -> Result<Option<Definition>, CommandError> {
        if !value.is_finite() {
            return Err(CommandError::Usage(
                "Line requires a finite angle or length",
            ));
        }
        if self.requests_angle() {
            if !value.to_radians().is_finite() {
                return Err(CommandError::Usage("Line angle is too large"));
            }
            self.angle = Some(value);
            return Ok(None);
        }
        if !self.endpoint() {
            return Err(CommandError::Usage("Line length needs an endpoint prompt"));
        }
        if self.mode == Mode::Normal
            && let Some(direction) = self.direction()?
        {
            if value == 0. {
                return Err(CommandError::Usage("Line needs a nonzero length"));
            }
            let point = self
                .anchor()
                .unwrap()
                .translated(direction.as_vector().scaled(value)?)?;
            return self.complete(point).map(Some);
        }
        let mut distance =
            viboceros_drafting::PointConstraintState::new(self.anchor().unwrap(), self.plane);
        distance
            .set(
                viboceros_drafting::PointConstraintInput::Distance(value),
                self.plane,
            )
            .map_err(|_| CommandError::Usage("Line needs a finite nonzero length"))?;
        self.distance = Some(distance);
        Ok(None)
    }
    pub fn preview(self, point: Point3) -> Option<Definition> {
        self.endpoint().then(|| self.complete(point).ok()).flatten()
    }
}

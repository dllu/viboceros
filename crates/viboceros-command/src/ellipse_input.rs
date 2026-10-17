//! Edit-free Ellipse construction shared by commands, previews and point getters.
use viboceros_geometry::{Ellipse3, Frame3, GeometryError, Point3, Real, Tolerance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Default,
    Vertical,
    Corner,
    Diameter,
    FromFoci,
    AroundCurve,
}
impl Mode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Vertical => "Vertical",
            Self::Corner => "Corner",
            Self::Diameter => "Diameter",
            Self::FromFoci => "FromFoci",
            Self::AroundCurve => "AroundCurve",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub mark_foci: bool,
    pub point_count: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            mark_foci: false,
            point_count: 12,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    PointCount,
    First,
    FirstAxis,
    SecondAxis,
    OppositeCorner,
    SecondFocus,
    ThroughPoint,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub ellipse: Ellipse3,
    pub foci: Option<[Point3; 2]>,
    pub deformable_point_count: Option<usize>,
}
impl Definition {
    pub fn to_nurbs(self) -> Result<viboceros_geometry::NurbsCurve, GeometryError> {
        match self.deformable_point_count {
            Some(count) => self.ellipse.try_deformable(count),
            None => self.ellipse.to_nurbs(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    mode: Mode,
    vertical: bool,
    centered_corner: bool,
    deformable: bool,
    point_count_pending: bool,
    options: Options,
    points: [Option<Point3>; 3],
    length: Option<Real>,
    result: Option<Definition>,
    plane: Frame3,
    tolerance: Tolerance,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
impl Input {
    pub fn new(options: Options, plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            mode: Mode::Default,
            vertical: false,
            centered_corner: false,
            deformable: false,
            point_count_pending: false,
            options,
            points: [None; 3],
            length: None,
            result: None,
            plane,
            tolerance,
        }
    }
    pub fn mode(self) -> Mode {
        self.mode
    }
    pub fn centered_corner(self) -> bool {
        self.centered_corner
    }
    pub fn vertical(self) -> bool {
        self.mode == Mode::Vertical || self.vertical
    }
    pub fn options(self) -> Options {
        self.options
    }
    pub fn deformable(self) -> bool {
        self.deformable
    }
    pub fn point_count_pending(self) -> bool {
        self.point_count_pending
    }
    pub fn accept_point_count_default(&mut self) {
        self.point_count_pending = false;
    }
    pub fn requests_curve(self) -> bool {
        self.mode == Mode::AroundCurve && self.points[0].is_none() && !self.point_count_pending
    }
    /// Latch the guide center and normal before the two axis getters.
    pub fn around_curve(&mut self, plane: Frame3) {
        self.mode = Mode::AroundCurve;
        self.plane = plane;
        self.points = [Some(plane.origin()), None, None];
        self.length = None;
        self.result = None;
    }
    /// AroundCurve's axes lie in its captured normal plane.
    pub fn resolve_point(self, point: Point3) -> Result<Point3, GeometryError> {
        if self.mode == Mode::AroundCurve && !self.requests_curve() {
            let [x, y, _] = self.plane.coordinates_of(point)?;
            self.plane.point_at([x, y, 0.])
        } else {
            Ok(point)
        }
    }
    pub fn slot(self) -> Slot {
        if self.point_count_pending {
            return Slot::PointCount;
        }
        if self.result.is_some() {
            return Slot::Complete;
        }
        if self.points[0].is_none() {
            return Slot::First;
        }
        if self.mode == Mode::Corner {
            return Slot::OppositeCorner;
        }
        if self.points[1].is_none() {
            return if self.mode == Mode::FromFoci {
                Slot::SecondFocus
            } else {
                Slot::FirstAxis
            };
        }
        if self.mode == Mode::FromFoci {
            Slot::ThroughPoint
        } else {
            Slot::SecondAxis
        }
    }
    pub fn anchor(self) -> Option<Point3> {
        if self.points[1].is_some() && matches!(self.mode, Mode::Diameter | Mode::FromFoci) {
            self.points[0]?.midpoint(self.points[1]?).ok()
        } else {
            self.points[0]
        }
    }
    pub fn reference(self) -> Option<Point3> {
        self.points[1]
    }
    pub fn markers(self) -> Vec<Point3> {
        self.points.into_iter().flatten().collect()
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.points[0].is_none() {
            self.plane = plane
        }
    }
    pub fn mouse_plane(self) -> Frame3 {
        if self.vertical()
            && let (Some(center), Some(first)) = (self.points[0], self.points[1])
            && let Ok(frame) = Frame3::try_from_directions(
                center,
                center.vector_to(first).unwrap(),
                self.plane.z_axis().as_vector(),
                self.tolerance,
            )
        {
            return frame;
        }
        self.plane
            .with_origin(self.anchor().unwrap_or(self.plane.origin()))
    }
    pub fn option(&mut self, word: &str) -> Result<bool, GeometryError> {
        let word = word.trim_start_matches(['_', '-']);
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v.trim_start_matches('_'))));
        if self.point_count_pending && key.eq_ignore_ascii_case("Enter") && value.is_none() {
            self.accept_point_count_default();
            return Ok(true);
        }
        if key.eq_ignore_ascii_case("Degree") {
            return Err(invalid(
                "deformable Ellipse output is cubic; Degree is unavailable in Rhino 8.32",
            ));
        }
        if key.eq_ignore_ascii_case("MarkFoci") && self.mode == Mode::FromFoci {
            self.options.mark_foci = match value {
                None => !self.options.mark_foci,
                Some(v) if v.eq_ignore_ascii_case("Yes") => true,
                Some(v) if v.eq_ignore_ascii_case("No") => false,
                _ => return Err(invalid("MarkFoci requires Yes or No")),
            };
            if let Some(result) = self.result {
                self.result = Some(self.definition(result.ellipse)?);
            }
            return Ok(true);
        }
        if self.result.is_some() {
            return Err(invalid("ellipse is complete"));
        }
        if key.eq_ignore_ascii_case("Deformable") {
            if self.points[0].is_some() || self.deformable {
                return Err(invalid(
                    "Deformable is available before the first ellipse point",
                ));
            }
            self.deformable = match value {
                None => true,
                Some(v) if v.eq_ignore_ascii_case("Yes") => true,
                Some(v) if v.eq_ignore_ascii_case("No") => false,
                _ => return Err(invalid("Deformable requires Yes or No")),
            };
            return Ok(true);
        }
        if key.eq_ignore_ascii_case("PointCount") && self.deformable && self.points[0].is_none() {
            if let Some(value) = value {
                let count = value
                    .parse::<usize>()
                    .map_err(|_| invalid("PointCount requires an integer from 4 through 1000"))?;
                self.set_point_count(count)?;
            } else {
                self.point_count_pending = true;
            }
            return Ok(true);
        }
        if value.is_none()
            && key.eq_ignore_ascii_case("Vertical")
            && self.mode == Mode::Diameter
            && self.points[0].is_none()
        {
            self.vertical = true;
            return Ok(true);
        }
        if value.is_none()
            && key.eq_ignore_ascii_case("Corner")
            && self.mode == Mode::Default
            && self.points[1].is_none()
        {
            self.centered_corner = self.points[0].is_some();
            self.mode = Mode::Corner;
            self.length = None;
            return Ok(true);
        }
        if self.points[0].is_none()
            && value.is_none()
            && let Some(mode) = [
                Mode::Default,
                Mode::Vertical,
                Mode::Corner,
                Mode::Diameter,
                Mode::FromFoci,
                Mode::AroundCurve,
            ]
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(key))
        {
            self.mode = mode;
            self.vertical = false;
            self.centered_corner = false;
            self.length = None;
            return Ok(true);
        }
        Ok(false)
    }
    fn definition(self, ellipse: Ellipse3) -> Result<Definition, GeometryError> {
        Ok(Definition {
            ellipse,
            deformable_point_count: self.deformable.then_some(self.options.point_count),
            foci: if self.options.mark_foci && self.mode == Mode::FromFoci {
                let [negative, positive] = ellipse.foci()?;
                Some([positive, negative])
            } else {
                None
            },
        })
    }
    pub fn definition_value(self) -> Option<Definition> {
        self.result
    }
    pub fn preview(self, point: Point3) -> Option<Definition> {
        let mut next = self;
        next.point(point).ok().flatten()
    }
    pub fn locked_length(self) -> Option<Real> {
        self.length
    }
    pub fn number(&mut self, value: Real) -> Result<(), GeometryError> {
        if self.point_count_pending {
            if !value.is_finite() || value.fract() != 0. || !(4.0..=1000.0).contains(&value) {
                return Err(invalid(
                    "PointCount requires an integer from 4 through 1000",
                ));
            }
            return self.set_point_count(value as usize);
        }
        if !value.is_finite() || value <= self.tolerance.absolute() {
            return Err(invalid(
                "ellipse axis length must be greater than tolerance",
            ));
        }
        match self.slot() {
            Slot::FirstAxis | Slot::SecondAxis | Slot::OppositeCorner => {
                self.length = Some(value);
                Ok(())
            }
            _ => Err(invalid("pick an ellipse point")),
        }
    }
    fn set_point_count(&mut self, count: usize) -> Result<(), GeometryError> {
        if !(4..=1000).contains(&count) {
            return Err(invalid(
                "PointCount requires an integer from 4 through 1000",
            ));
        }
        self.options.point_count = count;
        self.point_count_pending = false;
        Ok(())
    }
    fn center(self) -> Result<Point3, GeometryError> {
        let first = self.points[0].ok_or_else(|| invalid("ellipse first point required"))?;
        if self.mode == Mode::Diameter {
            first.midpoint(self.points[1].ok_or_else(|| invalid("ellipse second point required"))?)
        } else {
            Ok(first)
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, GeometryError> {
        if self.point_count_pending {
            return Err(invalid("enter the ellipse point count first"));
        }
        if self.requests_curve() {
            return Err(invalid("select an ellipse guide and center on it"));
        }
        let point = self.resolve_point(point)?;
        let mut next = *self;
        let ellipse = match self.slot() {
            Slot::PointCount => unreachable!("point-count getter handled before geometry"),
            Slot::First => {
                next.points[0] = Some(point);
                None
            }
            Slot::FirstAxis | Slot::SecondFocus => {
                let first = self.points[0].unwrap();
                let offset = first.vector_to(point)?;
                if offset.length()? <= self.tolerance.absolute() {
                    return Err(invalid("ellipse axis points must be distinct"));
                }
                next.points[1] = Some(if let Some(length) = self.length {
                    first.translated(
                        offset
                            .normalized(self.tolerance)?
                            .as_vector()
                            .scaled(length)?,
                    )?
                } else {
                    point
                });
                next.length = None;
                None
            }
            Slot::SecondAxis => {
                let center = self.center()?;
                let first = center.vector_to(self.points[1].unwrap())?;
                let frame = Frame3::try_from_directions(
                    center,
                    first,
                    if self.vertical()
                        && self.plane.z_axis().as_vector().cross(first)?.length()?
                            > self.tolerance.absolute()
                    {
                        self.plane.z_axis().as_vector()
                    } else {
                        center.vector_to(point)?
                    },
                    self.tolerance,
                )?;
                let height = frame.coordinates_of(point)?[1];
                let y = if height < 0. {
                    frame.y_axis().opposite()
                } else {
                    frame.y_axis()
                };
                let radius = if let Some(length) = self.length {
                    if self.mode == Mode::Diameter {
                        0.5 * length
                    } else {
                        length
                    }
                } else {
                    height.abs()
                };
                next.length = None;
                Some(Ellipse3::try_new(
                    center,
                    first.length()?,
                    radius,
                    frame.x_axis(),
                    y,
                    self.tolerance,
                )?)
            }
            Slot::ThroughPoint => Some(Ellipse3::try_from_foci_with_plane(
                self.points[0].unwrap(),
                self.points[1].unwrap(),
                point,
                self.plane,
                self.tolerance,
            )?),
            Slot::OppositeCorner => {
                let first = self.points[0].unwrap();
                let [mut x, mut y, _] = self.plane.with_origin(first).coordinates_of(point)?;
                if let Some(length) = self.length {
                    let scale = length / first.distance_to(point)?;
                    x *= scale;
                    y *= scale;
                }
                next.length = None;
                let factor = if self.centered_corner { 1. } else { 0.5 };
                let center = if self.centered_corner {
                    first
                } else {
                    self.plane
                        .with_origin(first)
                        .point_at([factor * x, factor * y, 0.])?
                };
                Some(Ellipse3::try_new(
                    center,
                    factor * x.abs(),
                    factor * y.abs(),
                    self.plane.x_axis(),
                    self.plane.y_axis(),
                    self.tolerance,
                )?)
            }
            Slot::Complete => return Err(invalid("ellipse is complete")),
        };
        let result = ellipse.map(|e| next.definition(e)).transpose()?;
        if result.is_some() {
            next.points[2] = Some(point);
            next.result = result
        }
        *self = next;
        Ok(result)
    }
    /// Retain guide selection and replay policy for AroundCurve completion.
    pub fn command_with_guide(
        self,
        id: viboceros_document::ObjectId,
        edge: Option<usize>,
        postselected: bool,
    ) -> Result<String, GeometryError> {
        let e = self
            .result
            .ok_or_else(|| invalid("ellipse is incomplete"))?
            .ellipse;
        if self.mode != Mode::AroundCurve {
            return Err(invalid("ellipse has no curve construction"));
        }
        let q = e.quadrants()?;
        let reference = edge.map_or_else(
            || {
                format!(
                    "{}={id}",
                    if postselected { "PickedCurve" } else { "Curve" }
                )
            },
            |edge| format!("Edge={id},{edge}"),
        );
        let point = |p: Point3| {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        };
        Ok(format!(
            "Ellipse {}AroundCurve {reference} {} {} {}",
            self.output_options(),
            point(e.center()),
            point(q[0]),
            point(q[1])
        ))
    }
    fn output_options(self) -> String {
        if self.deformable {
            format!("Deformable=Yes PointCount={} ", self.options.point_count)
        } else {
            String::new()
        }
    }
    /// Emit the completed analytic construction, so history has one command owner.
    pub fn command(self) -> Result<String, GeometryError> {
        let result = self
            .result
            .ok_or_else(|| invalid("ellipse is incomplete"))?;
        let e = result.ellipse;
        let q = e.quadrants()?;
        let p = |v: Point3| {
            let [x, y, z] = v.to_array();
            format!("{x},{y},{z}")
        };
        if self.mode == Mode::FromFoci {
            // A minor-axis point preserves the completed plane even when the
            // authored point was collinear and needed the captured CPlane.
            return Ok(format!(
                "Ellipse {}FromFoci {} {} {} MarkFoci={}",
                self.output_options(),
                p(self.points[0].unwrap()),
                p(self.points[1].unwrap()),
                p(q[1]),
                if self.options.mark_foci { "Yes" } else { "No" }
            ));
        }
        Ok(format!(
            "Ellipse {}{} {} {}",
            self.output_options(),
            p(e.center()),
            p(q[0]),
            p(q[1])
        ))
    }
}

#[derive(Default)]
pub(super) struct EllipseCommand(crate::remembered::Remembered<Options>);
impl crate::Command for EllipseCommand {
    fn name(&self) -> &'static str {
        "Ellipse"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["Ell"]
    }
    fn ellipse_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn run(
        &self,
        document: &mut viboceros_document::Document,
        arguments: &[&str],
    ) -> Result<String, crate::CommandError> {
        self.run_in_context(document, arguments, crate::CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut viboceros_document::Document,
        arguments: &[&str],
        context: crate::CommandContext,
    ) -> Result<String, crate::CommandError> {
        use viboceros_document::Geometry;
        let mut input = Input::new(
            self.0.get(),
            context.construction_plane,
            document.tolerance(),
        );
        let mut consumed = 0;
        let triples = arguments
            .iter()
            .enumerate()
            .filter(|(index, w)| {
                w.parse::<Real>().is_ok()
                    && !index.checked_sub(1).is_some_and(|i| {
                        arguments[i]
                            .trim_start_matches(['_', '-'])
                            .eq_ignore_ascii_case("PointCount")
                    })
            })
            .count()
            == 9
            && !arguments.iter().any(|w| w.contains(','));
        let mut guide = None;
        let mut source_selection = None;
        while consumed < arguments.len() {
            let word = arguments[consumed];
            let prior_mode = input.mode();
            if input.option(word)? {
                if input.mode() != prior_mode {
                    guide = None;
                    source_selection = None;
                }
                consumed += 1;
                continue;
            }
            if input.point_count_pending() {
                input.number(
                    word.parse().map_err(|_| {
                        crate::CommandError::Usage("PointCount requires an integer")
                    })?,
                )?;
                consumed += 1;
                continue;
            }
            if input.requests_curve() && guide.is_none() {
                let (key, value) = word.split_once('=').ok_or(crate::CommandError::Usage(
                    "Ellipse AroundCurve Curve=id|Edge=id,index center axis-end axis-end",
                ))?;
                let (id, edge) = if key.eq_ignore_ascii_case("Curve")
                    || key.eq_ignore_ascii_case("PickedCurve")
                {
                    (
                        value
                            .parse()
                            .map_err(|_| crate::CommandError::Usage("invalid ellipse curve"))?,
                        None,
                    )
                } else if key.eq_ignore_ascii_case("Edge") {
                    let (id, index) = crate::curve_reference::parse_edge(value)
                        .ok_or(crate::CommandError::Usage("invalid ellipse edge"))?;
                    (id, Some(index))
                } else {
                    return Err(crate::CommandError::Usage(
                        "select an ellipse curve or surface edge",
                    ));
                };
                guide = Some(crate::curve_reference::CapturedCurve::capture(
                    document, id, edge,
                )?);
                if edge.is_none() {
                    source_selection = Some((
                        id,
                        key.eq_ignore_ascii_case("PickedCurve") || !document.is_selected(id),
                    ));
                }
                consumed += 1;
                continue;
            }
            if word.contains(',') || input.slot() == Slot::First || triples {
                let (point, count) = crate::parse_point(&arguments[consumed..])?;
                if let Some(guide) = guide.take() {
                    input.around_curve(crate::curve_reference::perpendicular_frame(
                        guide.curve_ref(),
                        point,
                    )?);
                } else {
                    input.point(point)?;
                }
                consumed += count;
            } else {
                let value =
                    viboceros_drafting::PointInput::parse_length_with_units(word, document.units())
                        .ok_or(crate::CommandError::Usage(
                            "expected an ellipse axis length or point",
                        ))?
                        .map_err(|_| crate::CommandError::Usage("invalid ellipse axis length"))?;
                input.number(value)?;
                consumed += 1;
            }
        }
        let result = input.definition_value().ok_or(crate::CommandError::Usage(
            "Ellipse requires complete construction points",
        ))?;
        if let Some((id, postselected)) = source_selection {
            if postselected {
                document.release_command_selection_on_history_replay([id])?;
                document.select_command_results([])?;
            } else {
                document.select_command_results([id])?;
            }
        }
        if let Some(foci) = result.foci {
            for point in foci {
                document.add_geometry(Geometry::Point(point))?;
            }
        }
        let geometry = if result.deformable_point_count.is_some() {
            Geometry::NurbsCurve(result.to_nurbs()?)
        } else {
            Geometry::Ellipse(result.ellipse)
        };
        let id = document.add_geometry(geometry)?;
        self.0.set(input.options());
        Ok(format!(
            "Added ellipse {id} (radii {:.6} × {:.6})",
            result.ellipse.radius_x(),
            result.ellipse.radius_y()
        ))
    }
}

#[cfg(test)]
mod tests;

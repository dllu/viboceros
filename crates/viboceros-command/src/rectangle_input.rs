//! Rectangle getters return geometry definitions without document edits.
use viboceros_geometry::{Frame3, GeometryError, Point3, Polyline3, Real, Tolerance};
const ZERO: Real = 2.3283064365386963e-10;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangleDefinition {
    pub corners: [Point3; 4],
    pub rounding: Option<crate::rectangle_rounding::Corner>,
}
impl RectangleDefinition {
    pub fn plane_points(self) -> [Point3; 3] {
        [self.corners[0], self.corners[1], self.corners[3]]
    }
    pub fn polyline(self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        Polyline3::try_new(
            self.corners.into_iter().chain([self.corners[0]]).collect(),
            tolerance,
        )?
        .try_chord_length_parameterized()
    }
    pub fn from_corners(
        plane: Frame3,
        first: Point3,
        opposite: Point3,
    ) -> Result<Self, GeometryError> {
        let frame = plane.with_origin(first);
        let [x, y, _] = frame.coordinates_of(opposite)?;
        rectangle(frame, x.min(0.), x.max(0.), y.min(0.), y.max(0.))
    }
    pub fn from_center(
        plane: Frame3,
        center: Point3,
        corner: Point3,
    ) -> Result<Self, GeometryError> {
        let frame = plane.with_origin(center);
        let [x, y, _] = frame.coordinates_of(corner)?;
        rectangle(frame, -x.abs(), x.abs(), -y.abs(), y.abs())
    }
    pub fn from_three_points(
        first: Point3,
        second: Point3,
        width: Point3,
    ) -> Result<Self, GeometryError> {
        let x = first.vector_to(second)?;
        let frame = Frame3::try_from_directions(
            first,
            x,
            first.vector_to(width)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let h = frame.coordinates_of(width)?[1];
        rectangle(frame, 0., x.length()?, 0., h)
    }
}
fn rectangle(
    frame: Frame3,
    x0: Real,
    x1: Real,
    y0: Real,
    y1: Real,
) -> Result<RectangleDefinition, GeometryError> {
    if ![x0, x1, y0, y1].iter().all(|n| n.is_finite()) {
        return Err(GeometryError::NonFinite {
            context: "rectangle dimensions",
        });
    }
    if x1 - x0 <= ZERO || y1 - y0 <= ZERO {
        return Err(GeometryError::Degenerate {
            context: "rectangle dimensions",
        });
    }
    Ok(RectangleDefinition {
        rounding: None,
        corners: [
            frame.point_at([x0, y0, 0.])?,
            frame.point_at([x1, y0, 0.])?,
            frame.point_at([x1, y1, 0.])?,
            frame.point_at([x0, y1, 0.])?,
        ],
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RectangleMode {
    #[default]
    Corners,
    Center,
    ThreePoint,
    EdgeMidpoint,
    Vertical,
    AroundCurve,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangleInput {
    plane: Frame3,
    mode: RectangleMode,
    points: [Option<Point3>; 2],
    length: Option<Real>,
    width: Option<Real>,
    centered: bool,
    rounding: Option<crate::rectangle_rounding::Input>,
}
impl RectangleInput {
    pub fn new(plane: Frame3) -> Self {
        Self {
            plane,
            mode: RectangleMode::Corners,
            points: [None; 2],
            length: None,
            width: None,
            centered: false,
            rounding: None,
        }
    }
    /// Enable Rectangle's corner getter; surface-flow planes keep it disabled.
    pub fn with_rounding(mut self, options: crate::rectangle_rounding::Options) -> Self {
        self.rounding = Some(crate::rectangle_rounding::Input::new(options));
        self
    }
    pub fn rounded(self) -> bool {
        self.rounding.is_some_and(|r| r.enabled)
    }
    pub fn rounding_default(self) -> Option<Real> {
        self.rounding.map(|r| r.default_value())
    }
    pub fn rounding_options(self) -> Option<crate::rectangle_rounding::Options> {
        self.rounding.map(|r| r.options)
    }
    pub fn rho_pending(self) -> bool {
        self.rounding_pending()
            && self.rounding.unwrap().options.kind == crate::rectangle_rounding::Kind::Conic
    }
    pub fn rounding_pending(self) -> bool {
        self.rounding.is_some_and(|r| r.pending())
    }
    fn complete(&mut self, rectangle: RectangleDefinition) -> Option<RectangleDefinition> {
        self.rounding
            .as_mut()
            .map_or(Some(rectangle), |r| r.complete(rectangle))
    }
    /// Interactive callers latch the receiving viewport at the first point.
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.points[0].is_none() {
            self.plane = plane;
        }
    }
    pub fn mode(self) -> RectangleMode {
        self.mode
    }
    pub fn anchor(self) -> Option<Point3> {
        self.points[1].or(self.points[0])
    }
    pub fn requests_curve(self) -> bool {
        self.mode == RectangleMode::AroundCurve && self.points[0].is_none()
    }
    pub fn option(&mut self, word: &str) -> bool {
        if let Some(rounding) = &mut self.rounding {
            if rounding.option(word) {
                return true;
            }
            if rounding.pending() {
                return false;
            }
        }
        let word = word.trim_start_matches(['_', '-']);
        let mode = match word.to_ascii_lowercase().as_str() {
            "3point" => RectangleMode::ThreePoint,
            "edgemidpoint" => RectangleMode::EdgeMidpoint,
            "vertical" => RectangleMode::Vertical,
            "center" => RectangleMode::Center,
            "aroundcurve" => RectangleMode::AroundCurve,
            "corner" | "corners" => RectangleMode::Corners,
            _ => return false,
        };
        if self.points[0].is_some()
            && !(mode == RectangleMode::ThreePoint
                && matches!(self.mode, RectangleMode::Corners | RectangleMode::Center)
                && self.points[1].is_none())
        {
            return false;
        }
        self.centered = mode == RectangleMode::ThreePoint && self.mode == RectangleMode::Center;
        self.mode = mode;
        self.length = None;
        self.width = None;
        true
    }
    pub fn curve_frame(
        curve: viboceros_geometry::CurveRef<'_>,
        pick: Point3,
    ) -> Result<Frame3, GeometryError> {
        crate::curve_reference::perpendicular_frame(curve, pick)
    }
    pub fn around_curve(&mut self, plane: Frame3) {
        self.plane = plane;
        self.mode = RectangleMode::Center;
        self.points = [Some(plane.origin()), None];
        self.length = None;
        self.width = None;
    }
    pub fn prompt(self) -> &'static str {
        if self.rounding_pending() {
            return match self.rounding.unwrap().options.kind {
                crate::rectangle_rounding::Kind::Arc => {
                    "radius or point for rounded corner; Corner=Arc|Conic"
                }
                crate::rectangle_rounding::Kind::Conic => {
                    "rho or point for conic corner; Corner=Arc|Conic"
                }
            };
        }
        if self.width.is_some() {
            return "choose rectangle side or orientation";
        }
        match (self.mode, self.points, self.length) {
            (RectangleMode::AroundCurve, _, _) => "select a curve, then its rectangle center",
            (RectangleMode::Center, [None, _], _) => "center of rectangle",
            (RectangleMode::EdgeMidpoint, [None, _], _) => "midpoint of first edge",
            (_, [None, _], _) => "first corner; 3Point, Center, Vertical, AroundCurve",
            (RectangleMode::ThreePoint, [Some(_), None], _) if self.centered => {
                "midpoint of rectangle edge"
            }
            (RectangleMode::ThreePoint | RectangleMode::EdgeMidpoint, [Some(_), None], _) => {
                "end of first edge"
            }
            (RectangleMode::ThreePoint | RectangleMode::EdgeMidpoint, _, _) => {
                "rectangle width point or width"
            }
            (RectangleMode::Vertical, [Some(_), None], _) => "end of vertical rectangle edge",
            (RectangleMode::Vertical, _, _) => {
                "rectangle height point or height; Enter uses edge length"
            }
            (_, _, Some(_)) => "rectangle width or opposite corner",
            _ => "opposite corner or length",
        }
    }
    pub fn mouse_plane(self) -> Option<Frame3> {
        if self.rounding_pending() {
            return self.rounding.unwrap().plane();
        }
        self.points[0]?;
        if self.mode == RectangleMode::Vertical
            && let [Some(first), Some(second)] = self.points
        {
            return Frame3::try_from_directions(
                first,
                first.vector_to(second).ok()?,
                self.plane.z_axis().as_vector(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .ok();
        }
        Some(
            self.plane
                .with_origin(self.points[0].unwrap_or(self.plane.origin())),
        )
    }
    pub fn point(&mut self, p: Point3) -> Result<Option<RectangleDefinition>, GeometryError> {
        if self.rounding_pending() {
            return self.rounding.as_mut().unwrap().point(p).map(Some);
        }
        if self.requests_curve() {
            return Err(GeometryError::Degenerate {
                context: "rectangle curve reference",
            });
        }
        let Some(first) = self.points[0] else {
            self.points[0] = Some(p);
            return Ok(None);
        };
        let result = match self.mode {
            RectangleMode::Corners | RectangleMode::Center => {
                let p = if let Some(length) = self.length {
                    let frame = self.plane.with_origin(first);
                    let [_x, y, z] = frame.coordinates_of(p)?;
                    frame.point_at([
                        length
                            * if self.mode == RectangleMode::Center {
                                0.5
                            } else {
                                1.
                            },
                        y,
                        z,
                    ])?
                } else {
                    p
                };
                if self.mode == RectangleMode::Center {
                    RectangleDefinition::from_center(self.plane, first, p)?
                } else {
                    RectangleDefinition::from_corners(self.plane, first, p)?
                }
            }
            RectangleMode::ThreePoint | RectangleMode::EdgeMidpoint => {
                let Some(second) = self.points[1] else {
                    if first.distance_to(p)? <= ZERO {
                        return Err(GeometryError::Degenerate {
                            context: "rectangle first edge",
                        });
                    }
                    let p = if let Some(length) = self.length {
                        first.translated(
                            first
                                .vector_to(p)?
                                .normalized_nonzero()?
                                .as_vector()
                                .scaled(length)?,
                        )?
                    } else {
                        p
                    };
                    self.points[1] = Some(p);
                    self.length = None;
                    return Ok(None);
                };
                let first = if self.mode == RectangleMode::EdgeMidpoint {
                    first.translated(first.vector_to(second)?.scaled(-1.)?)?
                } else {
                    first
                };
                let p = if let Some(width) = self.width {
                    let frame = Frame3::try_from_directions(
                        first,
                        first.vector_to(second)?,
                        first.vector_to(p)?,
                        Tolerance::NUMERICAL_VALIDATION,
                    )?;
                    first.translated(frame.y_axis().as_vector().scaled(width)?)?
                } else {
                    p
                };
                if self.centered {
                    let frame = Frame3::try_from_directions(
                        first,
                        first.vector_to(second)?,
                        first.vector_to(p)?,
                        Tolerance::NUMERICAL_VALIDATION,
                    )?;
                    let length = first.distance_to(second)?;
                    let height = frame.coordinates_of(p)?[1];
                    rectangle(frame, -length, length, -height, height)?
                } else {
                    RectangleDefinition::from_three_points(first, second, p)?
                }
            }
            RectangleMode::Vertical => {
                let Some(second) = self.points[1] else {
                    if first.distance_to(p)? <= ZERO {
                        return Err(GeometryError::Degenerate {
                            context: "rectangle vertical edge",
                        });
                    }
                    let p = if let Some(length) = self.length {
                        first.translated(
                            first
                                .vector_to(p)?
                                .normalized_nonzero()?
                                .as_vector()
                                .scaled(length)?,
                        )?
                    } else {
                        p
                    };
                    Frame3::try_from_directions(
                        first,
                        first.vector_to(p)?,
                        self.plane.z_axis().as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )?;
                    self.points[1] = Some(p);
                    self.length = None;
                    return Ok(None);
                };
                let frame = Frame3::try_from_directions(
                    first,
                    first.vector_to(second)?,
                    self.plane.z_axis().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                let height = frame.coordinates_of(p)?[1];
                let height = self.width.map_or(height, |width| width * height.signum());
                RectangleDefinition::from_three_points(
                    first,
                    second,
                    first.translated(frame.y_axis().as_vector().scaled(height)?)?,
                )?
            }
            RectangleMode::AroundCurve => {
                return Err(GeometryError::Degenerate {
                    context: "rectangle curve reference",
                });
            }
        };
        Ok(self.complete(result))
    }
    pub fn number(&mut self, value: Real) -> Result<Option<RectangleDefinition>, GeometryError> {
        if self.rounding_pending() {
            return self.rounding.as_mut().unwrap().number(value).map(Some);
        }
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "rectangle length",
            });
        }
        if value <= ZERO {
            return Err(GeometryError::Degenerate {
                context: "rectangle length",
            });
        }
        let first = self.points[0].ok_or(GeometryError::Degenerate {
            context: "rectangle first point",
        })?;
        if matches!(self.mode, RectangleMode::Corners | RectangleMode::Center) {
            if let Some(length) = self.length {
                let frame = self.plane.with_origin(first);
                return rectangle(
                    frame,
                    if self.mode == RectangleMode::Center {
                        -length * 0.5
                    } else {
                        0.
                    },
                    if self.mode == RectangleMode::Center {
                        length * 0.5
                    } else {
                        length
                    },
                    if self.mode == RectangleMode::Center {
                        -value * 0.5
                    } else {
                        0.
                    },
                    if self.mode == RectangleMode::Center {
                        value * 0.5
                    } else {
                        value
                    },
                )
                .map(|rectangle| self.complete(rectangle));
            }
            self.length = Some(value);
            return Ok(None);
        }
        if self.points[1].is_some() {
            self.width = Some(value);
        } else {
            self.length = Some(value);
        }
        Ok(None)
    }
    pub fn enter(&mut self) -> Result<Option<RectangleDefinition>, GeometryError> {
        if self.rounding_pending() {
            return self.number(self.rounding.unwrap().default_value());
        }
        if let Some(length) = self.length {
            return self.number(length);
        }
        if let [Some(first), Some(second)] = self.points {
            let length = first.distance_to(second)?;
            return self.number(length);
        }
        Ok(None)
    }
    pub fn preview(self, p: Point3) -> Option<RectangleDefinition> {
        let mut trial = self;
        trial.point(p).ok().flatten().or_else(|| {
            trial
                .rounding_pending()
                .then(|| trial.enter().ok().flatten())
                .flatten()
        })
    }
}

pub const USAGE: &str = "Rectangle [Rounded] [Center|3Point|3Point EdgeMidpoint|Vertical|AroundCurve Curve=id|Edge=parent,index] points-or-dimensions [Corner=Arc|Conic radius-or-rho|Enter]";
/// Read scripted construction without adding helper objects to the document.
pub fn from_arguments(
    doc: &viboceros_document::Document,
    args: &[&str],
    plane: Frame3,
) -> Result<RectangleDefinition, crate::CommandError> {
    from_arguments_with_options(doc, args, plane, Default::default())
        .map(|(rectangle, _)| rectangle)
}
/// Scripted consumers share curve capture and point/dimension admission while
/// retaining their own construction options and completion phases.
pub(crate) struct RectangleScript {
    pub input: RectangleInput,
    curve: Option<crate::curve_reference::CapturedCurve>,
}
impl RectangleScript {
    pub fn new(input: RectangleInput) -> Self {
        Self { input, curve: None }
    }
    pub fn has_curve(&self) -> bool {
        self.curve.is_some()
    }
    pub fn value(
        &mut self,
        doc: &viboceros_document::Document,
        args: &[&str],
        legacy: bool,
    ) -> Result<(usize, Option<RectangleDefinition>), crate::CommandError> {
        use crate::CommandError;
        let token = *args.first().ok_or(CommandError::Usage(USAGE))?;
        if self.input.requests_curve() && self.curve.is_none() {
            let (key, value) = token.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let (id, edge) = if key.eq_ignore_ascii_case("Curve") {
                (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
            } else if key.eq_ignore_ascii_case("Edge") {
                let (id, edge) =
                    crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                (id, Some(edge))
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            self.curve = Some(crate::curve_reference::CapturedCurve::capture(
                doc, id, edge,
            )?);
            return Ok((1, None));
        }
        if token.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
            return Ok((1, self.input.enter()?));
        }
        if token.contains(',') || self.input.anchor().is_none() || self.curve.is_some() || legacy {
            let (point, count) = if token == "0" && self.input.anchor().is_none() && !legacy {
                (self.input.plane.origin(), 1)
            } else {
                crate::parse_point(args)?
            };
            if let Some(curve) = self.curve.take() {
                self.input
                    .around_curve(RectangleInput::curve_frame(curve.curve_ref(), point)?);
                Ok((count, None))
            } else {
                Ok((count, self.input.point(point)?))
            }
        } else {
            let value = token
                .parse::<Real>()
                .map_err(|_| CommandError::Usage(USAGE))?;
            Ok((1, self.input.number(value)?))
        }
    }
}

pub fn from_arguments_with_options(
    doc: &viboceros_document::Document,
    args: &[&str],
    plane: Frame3,
    options: crate::rectangle_rounding::Options,
) -> Result<(RectangleDefinition, crate::rectangle_rounding::Options), crate::CommandError> {
    use crate::CommandError;
    let mut script = RectangleScript::new(RectangleInput::new(plane).with_rounding(options));
    let mut result = None;
    let mut i = 0;
    // Preserve the existing six-scalar, two-corner spelling.
    let legacy = args.len() == 6 && args.iter().all(|v| v.parse::<Real>().is_ok());
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if script.input.option(args[i]) {
            i += 1;
            continue;
        }
        let (count, rectangle) = script.value(doc, &args[i..], legacy)?;
        i += count;
        result = rectangle;
    }
    result
        .map(|r| (r, script.input.rounding_options().unwrap()))
        .ok_or(CommandError::Usage(USAGE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_document::{Document, Geometry, SelectionMode};
    #[test]
    fn rectangle_failed_getters_and_scripted_commands_leave_document_and_history_intact() {
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
            .unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let commands = CommandRegistry::with_builtins();
        for text in [
            "Rectangle 1,2,3 1,4,9",
            "Rectangle 3Point 1,2,3 1,2,3 4,5,6",
            "Rectangle 3Point 1,2,3 4,5,6 7,8,9",
            "Rectangle Center 1,2,3 0 10",
            "Rectangle 1,2,3 10 -5",
            "Rectangle Vertical 0,0,0 0,0,10 4,5,6",
            "Rectangle AroundCurve Curve=999 1,2,3 4,5,6",
            "Rectangle 0,0,0 10 20 extra",
        ] {
            let before = format!("{doc:?}");
            assert!(commands.execute(&mut doc, text).is_err(), "{text}");
            assert_eq!(format!("{doc:?}"), before, "{text}");
            assert!(doc.undo_label().is_none());
        }
        commands.execute(&mut doc, "Rect 1 2 3 4 8 9").unwrap();
        assert_eq!(doc.objects().count(), 2);
        assert_eq!(doc.undo().unwrap().as_deref(), Some("Rectangle"));
        assert_eq!(doc.objects().count(), 1);
    }
    #[test]
    fn rectangle_invalid_width_is_retryable_and_preview_is_pure() {
        let plane = crate::CommandContext::default().construction_plane;
        let mut input = RectangleInput::new(plane);
        input.option("Vertical");
        input.point(Point3::try_new(1., 2., 3.).unwrap()).unwrap();
        input.point(Point3::try_new(5., 8., 9.).unwrap()).unwrap();
        let before = input;
        assert!(input.point(Point3::try_new(5., 8., 9.).unwrap()).is_err());
        assert_eq!(input, before);
        input.number(5.).unwrap();
        let before = input;
        let choice = Point3::try_new(1., 2., -8.).unwrap();
        let preview = input.preview(choice).unwrap();
        assert_eq!(input, before);
        assert_eq!(input.point(choice).unwrap(), Some(preview));
        assert!((preview.corners[2].distance_to(preview.corners[1]).unwrap() - 5.).abs() < 1e-10);
    }
}

//! Circle construction getters that return a definition without document edits.
use viboceros_geometry::{Circle3, Frame3, GeometryError, Point3, Real, Tolerance};

mod point_construction;
pub use point_construction::PointConstruction;
pub(crate) use point_construction::vertical_size;

/// Parse the initial diameter options in either of Rhino's supported orders.
pub fn diameter_prefix(arguments: &[&str]) -> Option<(bool, usize)> {
    let [first, rest @ ..] = arguments else {
        return None;
    };
    let is = |word, mode| crate::option_name_eq(word, mode);
    if is(first, "2Point") {
        let vertical = rest.first().is_some_and(|word| is(word, "Vertical"));
        Some((vertical, 1 + usize::from(vertical)))
    } else if is(first, "Vertical") && rest.first().is_some_and(|word| is(word, "2Point")) {
        Some((true, 2))
    } else {
        None
    }
}

const ZERO: Real = 2.3283064365386963e-10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CircleSizeMode {
    #[default]
    Radius,
    Diameter,
    Circumference,
    Area,
}
impl CircleSizeMode {
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim_start_matches(['_', '-']);
        [
            Self::Radius,
            Self::Diameter,
            Self::Circumference,
            Self::Area,
        ]
        .into_iter()
        .find(|m| name.eq_ignore_ascii_case(m.label()))
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Radius => "Radius",
            Self::Diameter => "Diameter",
            Self::Circumference => "Circumference",
            Self::Area => "Area",
        }
    }
    pub const fn prompt(self) -> &'static str {
        match self {
            Self::Radius => {
                "Circle radius; Diameter, Circumference, Area, ProjectOsnap; Enter accepts the remembered radius"
            }
            Self::Diameter => {
                "Circle diameter; Radius, Circumference, Area, ProjectOsnap; Enter accepts the remembered diameter"
            }
            Self::Circumference => "Circle circumference",
            Self::Area => "Circle area",
        }
    }
    pub fn radius(self, value: Real) -> Real {
        match self {
            Self::Radius => value,
            Self::Diameter => value * 0.5,
            Self::Circumference => value / std::f64::consts::TAU,
            Self::Area => (value / std::f64::consts::PI).sqrt(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircleDefinition {
    pub frame: Frame3,
    pub radius: Real,
}
impl CircleDefinition {
    /// Retain the circle's center, oriented axes and positive radius for a getter.
    pub fn from_circle(circle: Circle3) -> Result<Self, GeometryError> {
        let radius = valid_radius(circle.radius())?;
        Ok(Self {
            frame: Frame3::try_from_directions(
                circle.center(),
                circle.x_axis().as_vector(),
                circle.y_axis().as_vector(),
                Tolerance::NUMERICAL_VALIDATION,
            )?,
            radius,
        })
    }
}
fn valid_radius(radius: Real) -> Result<Real, GeometryError> {
    if !radius.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "Circle radius",
        });
    }
    if radius <= ZERO {
        return Err(GeometryError::Degenerate {
            context: "Circle radius",
        });
    }
    Ok(radius)
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Center {
        center: Option<Point3>,
        mode: CircleSizeMode,
    },
    TwoPoint {
        first: Option<Point3>,
        vertical: bool,
    },
    ThreePoint {
        first: Option<Point3>,
        second: Option<Point3>,
    },
    ThreePointRadius {
        first: Point3,
        second: Point3,
        radius: Option<Real>,
    },
    Vertical {
        center: Option<Point3>,
        radius: Option<Real>,
        mode: CircleSizeMode,
    },
    Orientation {
        center: Point3,
        normal: Option<Point3>,
        mode: CircleSizeMode,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircleInput {
    state: State,
    plane: Frame3,
}
impl CircleInput {
    pub fn new(plane: Frame3) -> Self {
        Self {
            state: State::Center {
                center: None,
                mode: CircleSizeMode::Radius,
            },
            plane,
        }
    }
    pub fn with_size_mode(mut self, mode: CircleSizeMode) -> Self {
        if let State::Center { center, .. } = self.state {
            self.state = State::Center { center, mode };
        }
        self
    }
    pub fn at_center(plane: Frame3, center: Point3) -> Self {
        Self {
            state: State::Center {
                center: Some(center),
                mode: CircleSizeMode::Radius,
            },
            plane,
        }
    }
    pub fn construction_plane(&self) -> Frame3 {
        self.plane
    }
    /// Latch the viewport construction plane before the first construction pick.
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.anchor().is_none() {
            self.plane = plane;
        }
    }
    pub const fn anchor(self) -> Option<Point3> {
        match self.state {
            State::Center { center, .. } | State::Vertical { center, .. } => center,
            State::TwoPoint { first, .. } => first,
            State::ThreePoint { first, second } => match second {
                Some(point) => Some(point),
                None => first,
            },
            State::ThreePointRadius { second, .. } => Some(second),
            State::Orientation { center, .. } => Some(center),
        }
    }
    pub const fn at_start(self) -> bool {
        matches!(self.state, State::Center { center: None, .. })
    }
    pub fn remembers_radius(self) -> bool {
        !matches!(
            self.state,
            State::TwoPoint { .. } | State::ThreePoint { .. }
        )
    }
    pub fn size_mode(self) -> CircleSizeMode {
        match self.state {
            State::Center { mode, .. }
            | State::Vertical { mode, .. }
            | State::Orientation { mode, .. } => mode,
            _ => CircleSizeMode::Radius,
        }
    }
    /// Whether Radius is a size mode rather than a three-point constraint.
    pub fn has_size_mode(self) -> bool {
        matches!(
            self.state,
            State::Center { .. } | State::Vertical { .. } | State::Orientation { .. }
        )
    }
    /// Point getters leave scalar input to the common drafting constraints.
    pub fn requests_size(self) -> bool {
        matches!(
            self.state,
            State::Center {
                center: Some(_),
                ..
            } | State::Vertical {
                center: Some(_),
                radius: None,
                ..
            } | State::Orientation {
                normal: Some(_),
                ..
            } | State::ThreePointRadius { radius: None, .. }
        )
    }
    pub fn prompt(self) -> &'static str {
        match self.state {
            State::Center { center: None, .. } => "Center of circle; Vertical, 2Point, 3Point",
            State::Center { mode, .. } => match mode {
                CircleSizeMode::Radius => {
                    "Circle radius; Diameter, Orientation, Circumference, Area; Enter accepts the remembered radius"
                }
                CircleSizeMode::Diameter => {
                    "Circle diameter; Radius, Orientation, Circumference, Area; Enter accepts the remembered diameter"
                }
                _ => mode.prompt(),
            },
            State::TwoPoint {
                first: None,
                vertical: false,
            } => "First end of diameter; Vertical",
            State::TwoPoint { first: None, .. } => "First end of diameter",
            State::TwoPoint { .. } => "Second end of diameter",
            State::ThreePoint { first: None, .. } => "First point on circle",
            State::ThreePoint { second: None, .. } => "Second point on circle",
            State::ThreePoint { .. } => "Third point on circle; Radius",
            State::ThreePointRadius { radius: None, .. } => "Radius of three-point circle",
            State::ThreePointRadius { .. } => "Center direction of circle",
            State::Vertical { center: None, .. } => "Center of vertical circle; 2Point",
            State::Vertical {
                radius: Some(_), ..
            } => "Direction of vertical circle",
            State::Vertical { mode, .. } => mode.prompt(),
            State::Orientation { normal: None, .. } => "New orientation of circle",
            State::Orientation { mode, .. } => mode.prompt(),
        }
    }
    pub fn option(&mut self, name: &str) -> bool {
        let name = name.trim().trim_start_matches(['_', '-']);
        if let State::Center { center: None, .. } = self.state {
            let state = if name.eq_ignore_ascii_case("Vertical") {
                State::Vertical {
                    center: None,
                    radius: None,
                    mode: self.size_mode(),
                }
            } else if name.eq_ignore_ascii_case("2Point") {
                State::TwoPoint {
                    first: None,
                    vertical: false,
                }
            } else if name.eq_ignore_ascii_case("3Point") {
                State::ThreePoint {
                    first: None,
                    second: None,
                }
            } else {
                return false;
            };
            self.state = state;
            return true;
        }
        if matches!(self.state, State::Vertical { center: None, .. })
            && name.eq_ignore_ascii_case("2Point")
            || matches!(
                self.state,
                State::TwoPoint {
                    first: None,
                    vertical: false
                }
            ) && name.eq_ignore_ascii_case("Vertical")
        {
            self.state = State::TwoPoint {
                first: None,
                vertical: true,
            };
            return true;
        }
        if let State::Center {
            center: Some(center),
            mode,
        }
        | State::Vertical {
            center: Some(center),
            radius: None,
            mode,
        }
        | State::Orientation {
            center,
            normal: Some(_),
            mode,
        } = self.state
            && name.eq_ignore_ascii_case("Orientation")
        {
            self.state = State::Orientation {
                center,
                normal: None,
                mode,
            };
            return true;
        }
        if let State::ThreePoint {
            first: Some(first),
            second: Some(second),
        } = self.state
            && name.eq_ignore_ascii_case("Radius")
        {
            self.state = State::ThreePointRadius {
                first,
                second,
                radius: None,
            };
            return true;
        }
        let Some(mode) = CircleSizeMode::parse(name) else {
            return false;
        };
        self.state = match self.state {
            State::Center {
                center: Some(center),
                ..
            } => State::Center {
                center: Some(center),
                mode,
            },
            State::Vertical {
                center: Some(center),
                radius: None,
                ..
            } => State::Vertical {
                center: Some(center),
                radius: None,
                mode,
            },
            State::Orientation {
                center,
                normal: Some(normal),
                ..
            } => State::Orientation {
                center,
                normal: Some(normal),
                mode,
            },
            _ => return false,
        };
        true
    }
    fn plane_at(self, center: Point3) -> Result<Frame3, GeometryError> {
        Frame3::try_from_directions(
            center,
            self.plane.x_axis().as_vector(),
            self.plane.y_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
    }
    fn vertical_plane(self, center: Point3) -> Result<Frame3, GeometryError> {
        Frame3::try_from_directions(
            center,
            self.plane.x_axis().as_vector(),
            self.plane.z_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
    }
    pub fn number(&mut self, value: Real) -> Result<Option<CircleDefinition>, GeometryError> {
        let radius = valid_radius(self.size_mode().radius(value))?;
        match self.state {
            State::Center {
                center: Some(center),
                ..
            } => Ok(Some(CircleDefinition {
                frame: self.plane_at(center)?,
                radius,
            })),
            State::Orientation {
                center,
                normal: Some(normal),
                ..
            } => Ok(Some(CircleDefinition {
                frame: Frame3::try_from_normal(
                    center,
                    center.vector_to(normal)?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?,
                radius,
            })),
            State::Vertical {
                center: Some(center),
                radius: None,
                mode,
            } => {
                if matches!(mode, CircleSizeMode::Circumference | CircleSizeMode::Area) {
                    return Ok(Some(CircleDefinition {
                        frame: self.vertical_plane(center)?,
                        radius,
                    }));
                }
                self.state = State::Vertical {
                    center: Some(center),
                    radius: Some(radius),
                    mode,
                };
                Ok(None)
            }
            State::ThreePointRadius {
                first,
                second,
                radius: None,
            } => {
                if radius < first.distance_to(second)? * 0.5 {
                    return Err(GeometryError::Degenerate {
                        context: "Circle chord exceeds diameter",
                    });
                }
                self.state = State::ThreePointRadius {
                    first,
                    second,
                    radius: Some(radius),
                };
                Ok(None)
            }
            _ => Err(GeometryError::Degenerate {
                context: "Circle getter requires a point",
            }),
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<CircleDefinition>, GeometryError> {
        let tol = Tolerance::NUMERICAL_VALIDATION;
        let circle = match self.state {
            State::Center { center: None, mode } => {
                self.state = State::Center {
                    center: Some(point),
                    mode,
                };
                return Ok(None);
            }
            State::Center {
                center: Some(center),
                mode,
            } => PointConstruction::Center { center, mode }.circle(point, self.plane, tol)?,
            State::TwoPoint {
                first: None,
                vertical,
            } => {
                self.state = State::TwoPoint {
                    first: Some(point),
                    vertical,
                };
                return Ok(None);
            }
            State::TwoPoint {
                first: Some(first),
                vertical,
            } => PointConstruction::Diameter { first, vertical }.circle(point, self.plane, tol)?,
            State::ThreePoint {
                first: None,
                second,
            } => {
                self.state = State::ThreePoint {
                    first: Some(point),
                    second,
                };
                return Ok(None);
            }
            State::ThreePoint {
                first: Some(first),
                second: None,
            } => {
                if first.distance_to(point)? <= ZERO {
                    return Err(GeometryError::Degenerate {
                        context: "Circle repeated point",
                    });
                }
                self.state = State::ThreePoint {
                    first: Some(first),
                    second: Some(point),
                };
                return Ok(None);
            }
            State::ThreePoint {
                first: Some(first),
                second: Some(second),
            } => PointConstruction::ThreePoint { first, second }.circle(point, self.plane, tol)?,
            State::ThreePointRadius {
                first,
                second,
                radius,
            } => PointConstruction::ThreePointRadius {
                first,
                second,
                radius,
            }
            .circle(point, self.plane, tol)?,
            State::Vertical {
                center: None,
                radius,
                mode,
            } => {
                self.state = State::Vertical {
                    center: Some(point),
                    radius,
                    mode,
                };
                return Ok(None);
            }
            State::Vertical {
                center: Some(center),
                radius,
                mode,
            } => PointConstruction::Vertical {
                center,
                radius,
                mode,
            }
            .circle(point, self.plane, tol)?,
            State::Orientation {
                center,
                normal: None,
                mode,
            } => {
                center.vector_to(point)?.normalized_nonzero()?;
                self.state = State::Orientation {
                    center,
                    normal: Some(point),
                    mode,
                };
                return Ok(None);
            }
            State::Orientation {
                center,
                normal: Some(normal),
                mode,
            } => PointConstruction::Oriented {
                center,
                normal_point: normal,
                mode,
            }
            .circle(point, self.plane, tol)?,
        };
        Ok(Some(CircleDefinition::from_circle(circle)?))
    }
    pub fn mouse_plane(self) -> Option<Frame3> {
        match self.state {
            State::Orientation {
                center,
                normal: Some(normal),
                ..
            } => Frame3::try_from_normal(
                center,
                center.vector_to(normal).ok()?,
                Tolerance::NUMERICAL_VALIDATION,
            )
            .ok(),
            _ => None,
        }
    }
    pub fn snap_projection_plane(self) -> Option<Frame3> {
        if !matches!(
            self.size_mode(),
            CircleSizeMode::Radius | CircleSizeMode::Diameter
        ) {
            return None;
        }
        match self.state {
            State::Center {
                center: Some(center),
                ..
            }
            | State::Vertical {
                center: Some(center),
                ..
            } => Some(self.plane.with_origin(center)),
            State::Orientation { .. } => self.mouse_plane(),
            _ => None,
        }
    }
    pub fn preview(self, point: Point3) -> Option<CircleDefinition> {
        let mut state = self;
        state.point(point).ok().flatten()
    }
}

#[cfg(test)]
mod tests;

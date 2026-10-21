//! Reference spheres reuse the circle getters and sphere fitting kernel.
use crate::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use viboceros_geometry::{Circle3, Frame3, GeometryError, Point3, Real, SphereFit, Tolerance};
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Circle(CircleInput),
    Diameter(Option<Point3>),
    Four([Option<Point3>; 3]),
    FourRadius {
        section: CircleDefinition,
        radius: Option<Real>,
    },
    Around(crate::circle_around_input::Input),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereInput {
    state: State,
    plane: Frame3,
}
impl SphereInput {
    pub fn new(plane: Frame3) -> Self {
        Self {
            state: State::Circle(CircleInput::new(plane)),
            plane,
        }
    }
    /// Latch the first pick's viewport without changing the chosen construction mode.
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.anchor().is_none() {
            self.plane = plane;
            if let State::Circle(c) = &mut self.state {
                c.set_initial_plane(plane);
            }
        }
    }
    pub fn anchor(self) -> Option<Point3> {
        match self.state {
            State::Circle(c) => c.anchor(),
            State::Diameter(p) => p,
            State::Four(p) => p.into_iter().flatten().last(),
            State::FourRadius { section, .. } => Some(section.frame.origin()),
            State::Around(input) => Some(input.center()),
        }
    }
    pub fn at_start(self) -> bool {
        matches!(self.state, State::Circle(c) if c.at_start())
    }
    pub fn around_frame(frame: Frame3) -> Self {
        Self {
            state: State::Around(crate::circle_around_input::Input::new(
                frame,
                Default::default(),
                Tolerance::NUMERICAL_VALIDATION,
            )),
            plane: frame,
        }
    }
    pub fn mouse_plane(self) -> Option<Frame3> {
        match self.state {
            State::Around(input) => Some(input.frame()),
            State::Circle(input) => input.mouse_plane(),
            _ => None,
        }
    }
    pub fn size_mode(self) -> CircleSizeMode {
        match self.state {
            State::Circle(c) => c.size_mode(),
            State::Around(input) => input.size_mode(),
            _ => CircleSizeMode::Radius,
        }
    }
    pub fn fit_points(points: &[Point3]) -> Result<CircleDefinition, GeometryError> {
        Self::fit_optional(points)?.ok_or(GeometryError::Degenerate {
            context: "reference sphere fit",
        })
    }
    pub fn fit_optional(points: &[Point3]) -> Result<Option<CircleDefinition>, GeometryError> {
        let Some(fit) = SphereFit::try_fit_to_points(points)? else {
            return Ok(None);
        };
        Ok(Some(CircleDefinition {
            frame: Frame3::try_from_normal(
                fit.center(),
                viboceros_geometry::Vector3::try_new(0., 0., 1.)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?,
            radius: fit.radius(),
        }))
    }
    pub fn requests_size(self) -> bool {
        match self.state {
            State::Circle(c) => c.requests_size(),
            State::Around(_) => true,
            State::FourRadius { radius: None, .. } => true,
            _ => false,
        }
    }
    pub fn default_size(self) -> Option<Real> {
        if matches!(self.state, State::FourRadius { .. }) || !self.requests_size() {
            return None;
        }
        Some(match self.size_mode() {
            CircleSizeMode::Radius => 1.,
            CircleSizeMode::Diameter => 2.,
            CircleSizeMode::Circumference => std::f64::consts::TAU,
            CircleSizeMode::Area => std::f64::consts::PI,
        })
    }
    pub fn prompt(self) -> String {
        match self.state {
            State::Circle(c) if c.at_start() => {
                "Center of reference sphere; 2Point, 3Point, Tangent, 4Point, AroundCurve, FitPoints".into()
            }
            State::Circle(c) => c
                .prompt()
                .replace("circle", "reference sphere")
                .replace("Circle", "Reference sphere"),
            State::Diameter(None) => "First end of sphere diameter".into(),
            State::Diameter(Some(_)) => "Second end of sphere diameter".into(),
            State::Four(p) => format!(
                "Point {} of reference sphere{}",
                p.into_iter().flatten().count() + 1,
                if p[1].is_some() { "; Radius" } else { "" }
            ),
            State::FourRadius { radius: None, .. } => "Sphere radius and orientation".into(),
            State::FourRadius { .. } => "Sphere center side of section circle".into(),
            State::Around(input) => format!(
                "Reference sphere {}; Radius, Diameter, Circumference, Area",
                input.size_mode().label()
            ),
        }
    }
    pub fn option(&mut self, name: &str) -> bool {
        let name = name.trim().trim_start_matches(['_', '-']);
        if let State::Around(input) = &mut self.state {
            return input.option(name);
        }
        if let State::Four([Some(first), Some(second), third]) = self.state
            && name.eq_ignore_ascii_case("Radius")
        {
            if let Some(third) = third {
                let Ok(circle) = Circle3::try_from_three_points(
                    first,
                    second,
                    third,
                    Tolerance::NUMERICAL_VALIDATION,
                ) else {
                    return false;
                };
                let Ok(section) = CircleDefinition::from_circle(circle) else {
                    return false;
                };
                self.state = State::FourRadius {
                    section,
                    radius: None,
                };
            } else {
                let mut input = CircleInput::new(self.plane);
                input.option("3Point");
                if input
                    .point(first)
                    .and_then(|_| input.point(second))
                    .is_err()
                {
                    return false;
                }
                input.option("Radius");
                self.state = State::Circle(input);
            }
            return true;
        }
        if let State::Circle(c) = &mut self.state {
            if c.at_start() {
                if name.eq_ignore_ascii_case("4Point") {
                    self.state = State::Four([None; 3]);
                    return true;
                }
                if name.eq_ignore_ascii_case("2Point") {
                    self.state = State::Diameter(None);
                    return true;
                }
                // Sphere construction has no Vertical option.
                if name.eq_ignore_ascii_case("Vertical") {
                    return false;
                }
            }
            return c.option(name);
        }
        false
    }
    pub fn number(&mut self, value: Real) -> Result<Option<CircleDefinition>, GeometryError> {
        match &mut self.state {
            State::Circle(c) => c.number(value),
            State::Around(input) => input
                .number(value)
                .and_then(CircleDefinition::from_circle)
                .map(Some),
            State::FourRadius { section, radius } => {
                if !value.is_finite() || value < section.radius {
                    return Err(GeometryError::Degenerate {
                        context: "sphere radius below section radius",
                    });
                }
                *radius = Some(value);
                Ok(None)
            }
            _ => Err(GeometryError::Degenerate {
                context: "sphere point required",
            }),
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<CircleDefinition>, GeometryError> {
        let tol = Tolerance::NUMERICAL_VALIDATION;
        match &mut self.state {
            State::Circle(c) => c.point(point),
            State::Around(input) => input
                .point(point)
                .and_then(CircleDefinition::from_circle)
                .map(Some),
            State::FourRadius { section, radius } => {
                let delta = section.frame.origin().vector_to(point)?;
                let radius = radius.map_or_else(|| delta.length(), Ok)?;
                if radius < section.radius {
                    return Err(GeometryError::Degenerate {
                        context: "sphere radius below section radius",
                    });
                }
                let ratio = section.radius / radius;
                let height = radius * ((1. - ratio) * (1. + ratio)).sqrt();
                let side = delta.dot(section.frame.z_axis().as_vector())?;
                if side == 0. && height > 0. {
                    return Err(GeometryError::Degenerate {
                        context: "sphere center side required",
                    });
                }
                let center = section.frame.origin().translated(
                    section
                        .frame
                        .z_axis()
                        .as_vector()
                        .scaled(height.copysign(side))?,
                )?;
                Ok(Some(CircleDefinition {
                    frame: Frame3::try_from_unit_directions(
                        center,
                        section.frame.x_axis(),
                        section.frame.y_axis(),
                        tol,
                    )?,
                    radius,
                }))
            }
            State::Diameter(first) => {
                let Some(a) = *first else {
                    *first = Some(point);
                    return Ok(None);
                };
                let circle = Circle3::try_from_diameter_on_plane(a, point, self.plane, tol)
                    .or_else(|e| {
                        let direction = a.vector_to(point)?.normalized_nonzero()?;
                        if direction
                            .as_vector()
                            .cross(self.plane.z_axis().as_vector())?
                            .length()?
                            > tol.angular()
                        {
                            return Err(e);
                        }
                        let plane =
                            Frame3::try_from_unit_normal(self.plane.origin(), self.plane.x_axis())?;
                        Circle3::try_from_diameter_on_plane(a, point, plane, tol)
                    })?;
                Ok(Some(CircleDefinition::from_circle(circle)?))
            }
            State::Four(points) => {
                if let Some(i) = points.iter().position(Option::is_none) {
                    if points
                        .iter()
                        .flatten()
                        .any(|p| p.distance_to(point).is_ok_and(|d| d <= tol.absolute()))
                    {
                        return Err(GeometryError::Degenerate {
                            context: "sphere repeated point",
                        });
                    }
                    if i == 2 {
                        Circle3::try_from_three_points(
                            points[0].unwrap(),
                            points[1].unwrap(),
                            point,
                            tol,
                        )?;
                    }
                    points[i] = Some(point);
                    return Ok(None);
                }
                let [a, b, c] = points.map(Option::unwrap);
                let circle = Circle3::try_from_three_points(a, b, c, tol)?;
                let section = CircleDefinition::from_circle(circle)?;
                if circle
                    .center()
                    .vector_to(point)?
                    .dot(section.frame.z_axis().as_vector())?
                    .abs()
                    <= tol.absolute()
                {
                    return Err(GeometryError::Degenerate {
                        context: "coplanar sphere points",
                    });
                }
                let fit = SphereFit::try_fit_to_points(&[a, b, c, point])?.ok_or(
                    GeometryError::Degenerate {
                        context: "sphere points",
                    },
                )?;
                Ok(Some(CircleDefinition {
                    frame: Frame3::try_from_unit_directions(
                        fit.center(),
                        section.frame.x_axis(),
                        section.frame.y_axis(),
                        tol,
                    )?,
                    radius: fit.radius(),
                }))
            }
        }
    }
}

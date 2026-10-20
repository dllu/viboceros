//! Reference spheres reuse the circle getters and sphere fitting kernel.
use crate::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use viboceros_geometry::{Circle3, Frame3, GeometryError, Point3, Real, SphereFit, Tolerance};
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Circle(CircleInput),
    Diameter(Option<Point3>),
    Four([Option<Point3>; 3]),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceInput {
    state: State,
    plane: Frame3,
}
impl ReferenceInput {
    pub fn new(plane: Frame3) -> Self {
        Self {
            state: State::Circle(CircleInput::new(plane)),
            plane,
        }
    }
    pub fn anchor(self) -> Option<Point3> {
        match self.state {
            State::Circle(c) => c.anchor(),
            State::Diameter(p) => p,
            State::Four(p) => p.into_iter().flatten().last(),
        }
    }
    pub fn requests_size(self) -> bool {
        matches!(self.state,State::Circle(c) if c.requests_size())
    }
    pub fn default_size(self) -> Option<Real> {
        let State::Circle(c) = self.state else {
            return None;
        };
        c.requests_size().then(|| match c.size_mode() {
            CircleSizeMode::Radius => 1.,
            CircleSizeMode::Diameter => 2.,
            CircleSizeMode::Circumference => std::f64::consts::TAU,
            CircleSizeMode::Area => std::f64::consts::PI,
        })
    }
    pub fn prompt(self) -> String {
        match self.state {
            State::Circle(c) if c.at_start() => {
                "Center of reference sphere; 2Point, 3Point, 4Point".into()
            }
            State::Circle(c) => c
                .prompt()
                .replace("circle", "reference sphere")
                .replace("Circle", "Reference sphere"),
            State::Diameter(None) => "First end of sphere diameter".into(),
            State::Diameter(Some(_)) => "Second end of sphere diameter".into(),
            State::Four(p) => format!(
                "Point {} of reference sphere",
                p.into_iter().flatten().count() + 1
            ),
        }
    }
    pub fn option(&mut self, name: &str) -> bool {
        let name = name.trim().trim_start_matches(['_', '-']);
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
            _ => Err(GeometryError::Degenerate {
                context: "sphere point required",
            }),
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<CircleDefinition>, GeometryError> {
        let tol = Tolerance::NUMERICAL_VALIDATION;
        match &mut self.state {
            State::Circle(c) => c.point(point),
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
                Ok(Some(definition(circle)?))
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
                let section = definition(circle)?;
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
fn definition(circle: Circle3) -> Result<CircleDefinition, GeometryError> {
    Ok(CircleDefinition {
        frame: Frame3::try_from_directions(
            circle.center(),
            circle.x_axis().as_vector(),
            circle.y_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )?,
        radius: circle.radius(),
    })
}

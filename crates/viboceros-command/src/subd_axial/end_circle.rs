//! The end-circle getter projects points to the accepted axis's normal plane.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EndCircle {
    pub axis: Definition,
    input: crate::circle_around_input::Input,
}
impl EndCircle {
    pub fn new(axis: Definition, p: Preferences) -> Result<Self, GeometryError> {
        let center = axis.frame.point_at([0., 0., axis.height])?;
        Ok(Self {
            axis,
            input: crate::circle_around_input::Input::new(
                axis.frame.with_origin(center),
                crate::circle_around_input::Preferences {
                    diameter: p.diameter,
                    ..Default::default()
                },
                Tolerance::NUMERICAL_VALIDATION,
            ),
        })
    }
    pub fn frame(self) -> Frame3 {
        self.input.frame()
    }
    pub fn diameter(self) -> bool {
        self.input.size_mode() == CircleSizeMode::Diameter
    }
    pub fn option(&mut self, name: &str) -> bool {
        if !["Radius", "Diameter"]
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
        {
            return false;
        }
        self.input.option(name)
    }
    pub fn number(mut self, value: f64) -> Result<(Definition, f64), GeometryError> {
        let radius = if value == 0. {
            0.
        } else {
            self.input.number(value)?.radius()
        };
        Ok((
            Definition {
                end_radius: radius,
                ..self.axis
            },
            value,
        ))
    }
    pub fn point(self, point: Point3) -> Result<(Definition, f64), GeometryError> {
        let point = self.input.resolve_point(point)?;
        self.number(self.input.center().distance_to(point)?)
    }
}

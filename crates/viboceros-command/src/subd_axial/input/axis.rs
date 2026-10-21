//! Signed and unconstrained axis construction after accepting the base.
use super::*;
impl Input {
    pub(super) fn end(&mut self, value: f64, point: Option<Point3>) -> Result<(), CommandError> {
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD primitive height",
            }
            .into());
        }
        if value.abs() <= 2.3283064365386963e-10 {
            return Err(GeometryError::Degenerate {
                context: "SubD primitive height",
            }
            .into());
        }
        let circle = self.circle.ok_or(CommandError::Usage(self.kind.usage()))?;
        let mut frame = circle.frame;
        if self.preferences.direction == Direction::None
            && let Some(point) = point
        {
            let normal = circle
                .frame
                .origin()
                .vector_to(point)?
                .normalized_nonzero()?;
            frame = if self.kind == Kind::TruncatedCone {
                Frame3::try_from_unit_normal(circle.frame.origin(), normal)?
            } else {
                let basis = Frame3::try_from_unit_normal(
                    circle.frame.origin(),
                    normal.as_vector().scaled(-1.)?.normalized_nonzero()?,
                )?;
                Frame3::try_from_directions(
                    basis.origin(),
                    basis.x_axis().as_vector(),
                    basis.y_axis().as_vector().scaled(-1.)?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?
            };
        }
        if value < 0. {
            frame = Frame3::try_from_directions(
                frame.origin(),
                frame.x_axis().as_vector(),
                frame.y_axis().as_vector().scaled(-1.)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
        }
        let height = if self.kind == Kind::Cylinder && self.preferences.both_sides {
            let offset = frame.z_axis().as_vector().scaled(-value.abs())?;
            frame = frame.with_origin(frame.origin().translated(offset)?);
            2. * value.abs()
        } else {
            value.abs()
        };
        if !height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD primitive height",
            }
            .into());
        }
        self.preferences.height = value;
        let definition = Definition {
            kind: self.kind,
            circle,
            frame,
            height,
            end_radius: if self.kind == Kind::Cylinder {
                circle.radius
            } else {
                0.
            },
        };
        if self.kind == Kind::TruncatedCone {
            self.end_circle = Some(super::end_circle::EndCircle::new(
                definition,
                self.preferences,
            )?);
            self.cursor = None;
        } else {
            self.definition = Some(definition);
        }
        Ok(())
    }
}

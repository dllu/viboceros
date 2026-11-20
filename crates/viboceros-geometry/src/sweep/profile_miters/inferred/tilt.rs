//! Rotation used to place a section plane between inferred miter boundaries.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Turn {
    axis: UnitVector3,
    half_angle: Real,
}

impl Turn {
    pub(super) fn new(incoming: UnitVector3, outgoing: UnitVector3) -> Result<Self, GeometryError> {
        let a = incoming.as_vector();
        let b = outgoing.as_vector();
        let angle = a.angle_to(b)?;
        if std::f64::consts::PI - angle <= 64. * Real::EPSILON {
            return Err(invalid("a reversing rail has no miter plane"));
        }
        Ok(Self {
            axis: a.cross(b)?.normalized_nonzero()?,
            half_angle: angle * 0.5,
        })
    }

    fn normal(self, tangent: Vector3, fraction: Real) -> Result<Vector3, GeometryError> {
        let (sin, cos) = (self.half_angle * fraction).sin_cos();
        let lateral = self.axis.as_vector().cross(tangent)?;
        Vector3::try_from(std::array::from_fn(|i| {
            tangent.to_array()[i].mul_add(cos, lateral.to_array()[i] * sin)
        }))
    }
}

pub(super) struct LegTilt {
    start: Option<Turn>,
    end: Option<Turn>,
    common: Option<(UnitVector3, [Real; 2])>,
}

impl LegTilt {
    pub(super) fn new(start: Option<Turn>, end: Option<Turn>) -> Result<Self, GeometryError> {
        let common = if let (Some(a), Some(b)) = (start, end) {
            let direction = a.axis.as_vector().dot(b.axis.as_vector())?;
            let sign: Real = if direction < 0. { -1. } else { 1. };
            let axis = Vector3::try_from(std::array::from_fn(|i| {
                sign.mul_add(
                    b.axis.as_vector().to_array()[i],
                    a.axis.as_vector().to_array()[i],
                )
            }))?
            .normalized_nonzero()?;
            Some((axis, [-a.half_angle, sign * b.half_angle]))
        } else {
            None
        };
        Ok(Self { start, end, common })
    }

    pub(super) fn normal(
        &self,
        tangent: Vector3,
        station: Real,
        [lo, hi]: [Real; 2],
        authored: Option<[Real; 2]>,
    ) -> Result<Vector3, GeometryError> {
        // Pin the real bisectors at the corners. Interior samples reuse one
        // axis for the two tilts until an authored profile resets the tilt.
        if station == lo {
            return self
                .start
                .map_or(Ok(tangent), |turn| turn.normal(tangent, -1.));
        }
        if station == hi {
            return self
                .end
                .map_or(Ok(tangent), |turn| turn.normal(tangent, 1.));
        }
        if let Some([first, last]) = authored {
            if station < first
                && let Some(turn) = self.start
            {
                return turn.normal(tangent, (station - first) / (first - lo));
            }
            if station > last
                && let Some(turn) = self.end
            {
                return turn.normal(tangent, (station - last) / (hi - last));
            }
            return Ok(tangent);
        }
        let fraction = (station - lo) / (hi - lo);
        if let Some((axis, [a, b])) = self.common {
            let turn = Turn {
                axis,
                half_angle: a * (1. - fraction) + b * fraction,
            };
            return turn.normal(tangent, 1.);
        }
        if let Some(turn) = self.start {
            return turn.normal(tangent, fraction - 1.);
        }
        self.end
            .map_or(Ok(tangent), |turn| turn.normal(tangent, fraction))
    }
}

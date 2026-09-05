//! Rotation-minimizing frames from the path of the unit tangent. Transport
//! does not subtract world-space points or divide by parameter speed.

use crate::{
    CurveRef, Frame3, GeometryError, ParameterSide, Real, Tolerance, UnitVector3, Vector3,
};

mod breaks;
#[cfg(test)]
mod tests;
mod transport;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameTransportOptions {
    /// Absolute angular error target in radians for the complete traversal.
    /// This controls adaptive estimates, not a certified continuous bound.
    pub angular_tolerance: Real,
    pub maximum_evaluations: usize,
    /// Which exact limit is returned at a requested corner. Transport still
    /// traverses both sides when continuing beyond that parameter.
    pub side: ParameterSide,
}

impl Default for FrameTransportOptions {
    fn default() -> Self {
        Self {
            angular_tolerance: 1e-10,
            maximum_evaluations: 131_072,
            side: ParameterSide::Right,
        }
    }
}

impl CurveRef<'_> {
    /// Parallel-transports a perpendicular frame along the native curve.
    /// The first requested parameter seeds the transport; `initial_x` is
    /// projected into its normal plane. Omit it for a deterministic seed.
    /// Parameters must be strictly increasing, but need not cover the curve.
    /// Corners use their minimal tangent rotation; antiparallel corners and
    /// positional jumps are rejected. Closed paths retain their holonomy.
    pub fn rotation_minimizing_frames(
        self,
        parameters: &[Real],
        initial_x: Option<UnitVector3>,
        options: FrameTransportOptions,
    ) -> Result<Vec<Frame3>, GeometryError> {
        if !options.angular_tolerance.is_finite()
            || options.angular_tolerance <= 0.0
            || options.angular_tolerance > 1.0
            || options.maximum_evaluations == 0
        {
            return Err(GeometryError::InvalidCurveFrameOptions);
        }
        let domain = self.domain();
        if parameters.is_empty()
            || parameters
                .iter()
                .any(|t| !t.is_finite() || !domain.contains(t))
            || parameters.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(GeometryError::InvalidCurveFrameParameters);
        }
        if parameters.len() > options.maximum_evaluations {
            return Err(GeometryError::CurveFrameResourceLimit {
                maximum: options.maximum_evaluations,
            });
        }
        let mut evaluator = transport::Evaluator::new(self, options.maximum_evaluations);
        let first = evaluator.sample(parameters[0], options.side)?;
        let seed = match initial_x {
            Some(x) => perpendicular(x.as_vector(), first.tangent())?,
            None => Frame3::try_from_normal(
                first.point(),
                first.tangent().as_vector(),
                axes_tolerance(),
            )?
            .x_axis(),
        };
        let mut output = Vec::with_capacity(parameters.len());
        output.push(frame(first.point(), first.tangent(), seed)?);
        if parameters.len() == 1 {
            return Ok(output);
        }
        let end = *parameters.last().unwrap();
        let whole = end - parameters[0];
        if !whole.is_finite() {
            return Err(GeometryError::InvalidCurveFrameParameters);
        }
        let breaks = breaks::parameters(self, parameters[0], end, options.maximum_evaluations)?;
        let mut index = 0;
        let mut a = parameters[0];
        let mut tangent = first.tangent();
        let mut x = seed;
        if options.side == ParameterSide::Left {
            let right = evaluator.sample(a, ParameterSide::Right)?;
            if breaks::position_can_jump(self, a)? && first.point() != right.point() {
                return Err(GeometryError::DiscontinuousCurveFrame);
            }
            x = transport::minimal_rotation(x, tangent, right.tangent())
                .ok_or(GeometryError::DiscontinuousCurveFrame)??;
            tangent = right.tangent();
        }
        for &target in &parameters[1..] {
            let mut left_frame = None;
            while a < target {
                while index < breaks.len() && breaks[index] <= a {
                    index += 1;
                }
                let b = breaks.get(index).copied().unwrap_or(target).min(target);
                let left = evaluator.sample(b, ParameterSide::Left)?;
                let budget = options.angular_tolerance * ((b - a) / whole);
                // Natural polyline spans and lines have a constant tangent;
                // only their explicit corner rotations need work.
                if !matches!(self, Self::Line(_) | Self::Polyline(_)) {
                    x = evaluator.advance(a, b, tangent, left.tangent(), x, budget, 0)?;
                }
                if b == target && options.side == ParameterSide::Left {
                    left_frame = Some(frame(left.point(), left.tangent(), x)?);
                    if b == end {
                        output.push(left_frame.unwrap());
                        return Ok(output);
                    }
                }
                let right = evaluator.sample(b, ParameterSide::Right)?;
                if breaks::position_can_jump(self, b)? && left.point() != right.point() {
                    return Err(GeometryError::DiscontinuousCurveFrame);
                }
                x = transport::minimal_rotation(x, left.tangent(), right.tangent())
                    .ok_or(GeometryError::DiscontinuousCurveFrame)??;
                tangent = right.tangent();
                a = b;
            }
            output.push(if let Some(f) = left_frame {
                f
            } else {
                frame(
                    evaluator.sample(target, ParameterSide::Right)?.point(),
                    tangent,
                    x,
                )?
            });
        }
        Ok(output)
    }
}

fn axes_tolerance() -> Tolerance {
    Tolerance::try_new(
        Real::MIN_POSITIVE,
        64.0 * Real::EPSILON,
        64.0 * Real::EPSILON,
    )
    .unwrap()
}

fn perpendicular(x: Vector3, tangent: UnitVector3) -> Result<UnitVector3, GeometryError> {
    let y = tangent.as_vector().cross(x)?.normalized_nonzero()?;
    y.as_vector()
        .cross(tangent.as_vector())?
        .normalized_nonzero()
}

fn frame(
    origin: crate::Point3,
    tangent: UnitVector3,
    x: UnitVector3,
) -> Result<Frame3, GeometryError> {
    Frame3::try_from_directions(
        origin,
        x.as_vector(),
        tangent.as_vector().cross(x.as_vector())?,
        axes_tolerance(),
    )
}

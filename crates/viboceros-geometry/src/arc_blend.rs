//! Tangent biarcs from endpoint positions and directions.
use crate::{CircularArc3, GeometryError, Point3, PolyCurve3, Real, Tolerance, Vector3};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcBlendOptions {
    /// Ratio of the two arc control-polygon lengths; strictly positive.
    pub ratio: Real,
    /// Choose the other signed solution of the tangent-length equation.
    pub alternate: bool,
}
impl Default for ArcBlendOptions {
    fn default() -> Self {
        Self {
            ratio: 1.,
            alternate: false,
        }
    }
}
/// Construct two tangent circular arcs. Degenerate straight or zero-length
/// pieces fail rather than replacing the two-arc construction with a line.
pub fn try_arc_blend(
    start: Point3,
    start_direction: Vector3,
    end: Point3,
    end_direction: Vector3,
    options: ArcBlendOptions,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    if !options.ratio.is_finite() || options.ratio <= 0. {
        return Err(GeometryError::Degenerate {
            context: "arc blend control-polygon ratio",
        });
    }
    let first = start_direction.normalized_nonzero()?.as_vector();
    let second = end_direction.normalized_nonzero()?.as_vector();
    let chord = start.vector_to(end)?;
    let length = chord.length()?;
    if length == 0. {
        return Err(GeometryError::Degenerate {
            context: "arc blend endpoints",
        });
    }
    let direction = chord.normalized_nonzero()?.as_vector();
    let (a, b) = if options.ratio <= 1. {
        let b = 1. / (1. + options.ratio);
        (options.ratio * b, b)
    } else {
        let a = 1. / (1. + 1. / options.ratio);
        (a, a / options.ratio)
    };
    let delta = Vector3::try_from(std::array::from_fn(|i| {
        first.to_array()[i] - second.to_array()[i]
    }))?;
    // d0=a*d, d1=b*d. Tangency at the shared point requires
    // |chord-d0*t0-d1*t1|^2=(d0+d1)^2. Work in chord-length units.
    let quadratic = a * b * delta.dot(delta)?;
    let linear = 2.
        * direction.dot(Vector3::try_from(std::array::from_fn(|i| {
            a * first.to_array()[i] + b * second.to_array()[i]
        }))?)?;
    let d = if quadratic == 0. {
        if linear == 0. || options.alternate {
            return Err(GeometryError::Degenerate {
                context: "arc blend tangent-length equation",
            });
        }
        1. / linear
    } else {
        let discriminant = linear.hypot(2. * quadratic.sqrt());
        if options.alternate {
            if linear <= 0. {
                -2. / (discriminant - linear)
            } else {
                (-linear - discriminant) / (2. * quadratic)
            }
        } else if linear >= 0. {
            2. / (linear + discriminant)
        } else {
            (-linear + discriminant) / (2. * quadratic)
        }
    };
    if !d.is_finite() {
        return Err(GeometryError::Degenerate {
            context: "arc blend tangent lengths",
        });
    }
    let offset = Vector3::try_from(std::array::from_fn(|i| {
        a * chord.to_array()[i] + a * b * d * length * delta.to_array()[i]
    }))?;
    let joint = start.translated(offset)?;
    let construction = Tolerance::try_new(
        tolerance
            .absolute()
            .min((length * 1e-12).max(Real::MIN_POSITIVE)),
        tolerance.relative(),
        tolerance.angular().min(1e-12),
    )?;
    let first = CircularArc3::try_from_start_tangent_end(
        start,
        first.normalized_nonzero()?,
        joint,
        construction,
    )?;
    let second = CircularArc3::try_from_start_tangent_end(
        end,
        second.normalized_nonzero()?.opposite(),
        joint,
        construction,
    )?
    .reversed(construction)?;
    PolyCurve3::try_new(vec![first, second])
}

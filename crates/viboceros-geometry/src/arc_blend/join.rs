//! Shared endpoint and tangent-length equation for construction and search.
use super::ArcBlendOptions;
use crate::{GeometryError, Point3, Real, Vector3};
pub(super) struct Frame {
    pub first: Vector3,
    pub second: Vector3,
    pub length: Real,
    chord: Vector3,
    direction: Vector3,
    delta: Vector3,
}
impl Frame {
    pub fn new(
        start: Point3,
        first: Vector3,
        end: Point3,
        second: Vector3,
    ) -> Result<Self, GeometryError> {
        let first = first.normalized_nonzero()?.as_vector();
        let second = second.normalized_nonzero()?.as_vector();
        let chord = start.vector_to(end)?;
        let length = chord.length()?;
        if length == 0. {
            return Err(GeometryError::Degenerate {
                context: "arc blend endpoints",
            });
        }
        let direction = chord.normalized_nonzero()?.as_vector();
        let delta = Vector3::try_from(std::array::from_fn(|i| {
            first.to_array()[i] - second.to_array()[i]
        }))?;
        Ok(Self {
            first,
            second,
            length,
            chord,
            direction,
            delta,
        })
    }
    pub fn offset(&self, options: ArcBlendOptions) -> Result<Vector3, GeometryError> {
        let (a, b) = if options.ratio <= 1. {
            let b = 1. / (1. + options.ratio);
            (options.ratio * b, b)
        } else {
            let a = 1. / (1. + 1. / options.ratio);
            (a, a / options.ratio)
        };
        // d0=a*d, d1=b*d. Tangency requires
        // |chord-d0*t0-d1*t1|²=(d0+d1)², in chord-length units.
        let quadratic = a * b * self.delta.dot(self.delta)?;
        let linear = 2.
            * self
                .direction
                .dot(Vector3::try_from(std::array::from_fn(|i| {
                    a * self.first.to_array()[i] + b * self.second.to_array()[i]
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
        Vector3::try_from(std::array::from_fn(|i| {
            a * self.chord.to_array()[i] + a * b * d * self.length * self.delta.to_array()[i]
        }))
    }
    pub fn radius_difference(&self, a: Real, alternate: bool) -> Result<Real, GeometryError> {
        let offset = self.offset(ArcBlendOptions {
            ratio: a / (1. - a),
            alternate,
        })?;
        let other = Vector3::try_from(std::array::from_fn(|i| {
            offset.to_array()[i] - self.chord.to_array()[i]
        }))?;
        Ok(radius(self.first, offset)? - radius(self.second, other)?)
    }
}
fn radius(tangent: Vector3, chord: Vector3) -> Result<Real, GeometryError> {
    let length = chord.length()?;
    let height = tangent.cross(chord)?.length()?;
    if height <= 1e-12 * length {
        return Err(GeometryError::Degenerate {
            context: "arc blend radius search straight piece",
        });
    }
    let radius = (0.5 * length) * (length / height);
    crate::require_finite([radius], "arc blend search radius")?;
    Ok(radius)
}

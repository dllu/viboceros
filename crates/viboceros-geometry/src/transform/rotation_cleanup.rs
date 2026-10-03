//! OpenNURBS cardinal-angle cleanup shared by command and space-morph maps.
use super::*;
#[path = "../../../../third_party/opennurbs_rust/rotation.rs"]
mod native_noise;

impl AffineTransform3 {
    /// Rotation with the open-source OpenNURBS cardinal-angle noise policy.
    /// Use `try_rotation` when retaining all representable small angles is required.
    pub fn try_rotation_with_cardinal_cleanup(
        center: Point3,
        axis: UnitVector3,
        angle: Real,
    ) -> Result<Self, GeometryError> {
        let Some((sine, cosine)) = native_noise::snapped_components(angle) else {
            return Self::try_rotation(center, axis, angle);
        };
        if sine == 0. && cosine == 1. {
            return Ok(Self::identity());
        }
        let [x, y, z] = axis.as_vector().to_array();
        let complement = 1. - cosine;
        Self::try_with_fixed_point(
            [
                [
                    x * x * complement + cosine,
                    x * y * complement - z * sine,
                    x * z * complement + y * sine,
                ],
                [
                    y * x * complement + z * sine,
                    y * y * complement + cosine,
                    y * z * complement - x * sine,
                ],
                [
                    z * x * complement - y * sine,
                    z * y * complement + x * sine,
                    z * z * complement + cosine,
                ],
            ],
            center,
        )
    }
}

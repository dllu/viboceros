//! Native command angle cleanup, separate from the kernel's precise rotation.
use viboceros_geometry::{AffineTransform3, GeometryError, Point3, UnitVector3};

#[path = "../../../third_party/opennurbs_rust/rotation.rs"]
mod native_noise;

pub(super) fn command_rotation(
    center: Point3,
    axis: UnitVector3,
    angle: f64,
) -> Result<AffineTransform3, GeometryError> {
    let Some((sine, cosine)) = native_noise::snapped_components(angle) else {
        // Also validates nonfinite angles, which cannot match a snap branch.
        return AffineTransform3::try_rotation(center, axis, angle);
    };
    if sine == 0.0 && cosine == 1.0 {
        return Ok(AffineTransform3::identity());
    }
    let [x, y, z] = axis.as_vector().to_array();
    let complement = 1.0 - cosine;
    let linear = [
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
    ];
    AffineTransform3::try_with_fixed_point(linear, center)
}

//! Native command angle cleanup shared with space-morph compatibility.
use viboceros_geometry::{AffineTransform3, GeometryError, Point3, UnitVector3};

pub(super) fn command_rotation(
    center: Point3,
    axis: UnitVector3,
    angle: f64,
) -> Result<AffineTransform3, GeometryError> {
    AffineTransform3::try_rotation_with_cardinal_cleanup(center, axis, angle)
}

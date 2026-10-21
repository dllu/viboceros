//! Second-circle measurements stay in the accepted base circle's plane.
use super::*;
pub(super) fn radius(
    circle: CircleDefinition,
    radius: f64,
    p: Preferences,
) -> Result<Definition, CommandError> {
    if !radius.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "SubDTorus tube radius",
        }
        .into());
    }
    if radius <= 0. || !p.fix_inner_dimension && radius >= circle.radius {
        return Err(CommandError::Usage(
            "Second radius must be positive and, without FixInnerDimension, smaller than the first radius",
        ));
    }
    Ok(Definition {
        circle,
        tube_radius: radius,
        fix_inner_dimension: p.fix_inner_dimension,
    })
}
pub(super) fn number(
    circle: CircleDefinition,
    value: f64,
    p: Preferences,
) -> Result<Definition, CommandError> {
    radius(circle, if p.tube_diameter { 0.5 * value } else { value }, p)
}
pub(super) fn point(
    circle: CircleDefinition,
    point: Point3,
    p: Preferences,
) -> Result<Definition, CommandError> {
    let [x, y] = circle.frame.projected_coordinates_of(point)?;
    radius(circle, (x.hypot(y) - circle.radius).abs(), p)
}

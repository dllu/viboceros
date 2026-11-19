//! Reusable geometry stage; no document changes.
use super::*;

pub(super) fn offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    if options.distance == 0. && !options.solid {
        return Ok(geometry.clone());
    }
    if options.loose {
        return loose::offset(geometry, options, tolerance);
    }
    if let Some(offset) = spherical_offset(geometry, options, tolerance)? {
        return Ok(offset);
    }
    if let Some(offset) = cylindrical_offset(geometry, options, tolerance)? {
        return Ok(offset);
    }
    if let Some(offset) = toroidal_offset(geometry, options, tolerance)? {
        return Ok(offset);
    }
    if let Some(offset) = conical_offset(geometry, options, tolerance)? {
        return Ok(offset);
    }
    native::offset(geometry, options, tolerance)
}

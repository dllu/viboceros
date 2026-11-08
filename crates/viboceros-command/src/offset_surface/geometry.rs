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
    #[cfg(feature = "native-smlib")]
    {
        native::offset(geometry, options, tolerance)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let normal = planar_normal(geometry, tolerance)?;
        if options.solid {
            let solid = if let Some(box_solid) = rectangular_solid(
                geometry,
                normal,
                options.distance,
                options.both_sides,
                tolerance,
            )? {
                box_solid
            } else {
                let profile = solid_profile(geometry, tolerance)?;
                Brep::try_extruded_curve(
                    &profile,
                    normal.scaled(if options.both_sides {
                        -options.distance
                    } else {
                        0.0
                    })?,
                    normal.scaled(options.distance)?,
                    tolerance,
                )?
            };
            return Ok(Geometry::Brep(solid));
        }
        if options.both_sides {
            let mut parts = Vec::with_capacity(2);
            for signed_distance in [options.distance, -options.distance] {
                let offset = normal.scaled(signed_distance)?;
                let moved =
                    geometry.transformed(AffineTransform3::from_translation(offset), tolerance)?;
                parts.push(match moved {
                    Geometry::NurbsSurface(surface) => Brep::try_surface_face(surface, tolerance)?,
                    Geometry::Brep(brep) => brep,
                    _ => unreachable!("only surface geometry reaches an offset"),
                });
            }
            return Ok(Geometry::Brep(Brep::try_disjoint_union(parts, tolerance)?));
        }
        let offset = normal.scaled(options.distance)?;
        let result = geometry.transformed(AffineTransform3::from_translation(offset), tolerance)?;
        Ok(result)
    }
}

//! General sheet and solid offsets through the native geometry kernel.
use super::*;
use viboceros_smlib::Shape;
#[cfg(test)]
mod tests;

pub(super) fn offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let converted;
    let source = match geometry {
        Geometry::Brep(brep) => brep,
        Geometry::NurbsSurface(surface) => {
            converted = Brep::try_surface_face(surface.clone(), tolerance)?;
            &converted
        }
        _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
    };
    let shape = Shape::from_brep(source, tolerance)?;
    let approximation = tolerance.absolute().min(1e-8);
    let result = if options.both_sides && options.solid {
        shape
            .offset(-options.distance, false)?
            .offset(2. * options.distance, true)?
            .to_brep_approximated(tolerance, approximation)?
    } else if options.both_sides {
        let parts = [options.distance, -options.distance]
            .into_iter()
            .map(|d| {
                shape
                    .offset(d, false)?
                    .to_brep_approximated(tolerance, approximation)
            })
            .collect::<Result<Vec<_>, viboceros_smlib::Error>>()?;
        Brep::try_disjoint_union(parts, tolerance)?
    } else {
        shape
            .offset(options.distance, options.solid)?
            .to_brep_approximated(tolerance, approximation)?
    };
    if !options.solid
        && !options.both_sides
        && matches!(geometry, Geometry::NurbsSurface(_))
        && result.faces().len() == 1
        && result.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(Geometry::NurbsSurface(result.faces()[0].surface().clone()));
    }
    if options.solid && !result.is_solid() {
        return Err(viboceros_smlib::Error::Kernel(
            "Offset wall did not produce a closed solid".into(),
        )
        .into());
    }
    Ok(Geometry::Brep(result))
}

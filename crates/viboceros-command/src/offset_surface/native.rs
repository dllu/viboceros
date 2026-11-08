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
    let requested = options.effective_tolerance(tolerance);
    let approximation = if options.tolerance == 0. {
        requested.min(1e-8)
    } else {
        // The requested value is an upper bound. Retain the conservative cap
        // that keeps independently fitted surfaces and UV trims compatible;
        // a smaller explicit request refines it further.
        (requested * 0.25).min(tolerance.absolute().min(1e-8))
    };
    let accuracy = 1e-9_f64.min(requested * 0.25);
    let result = if options.both_sides && options.solid {
        shape
            .offset_with_accuracy(-options.distance, false, accuracy)?
            .offset_with_accuracy(2. * options.distance, true, accuracy)?
            .to_brep_approximated(tolerance, approximation)?
    } else if options.both_sides {
        let parts = [options.distance, -options.distance]
            .into_iter()
            .map(|d| {
                shape
                    .offset_with_accuracy(d, false, accuracy)?
                    .to_brep_approximated(tolerance, approximation)
            })
            .collect::<Result<Vec<_>, viboceros_smlib::Error>>()?;
        Brep::try_disjoint_union(parts, tolerance)?
    } else {
        shape
            .offset_with_accuracy(options.distance, options.solid, accuracy)?
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

//! Original-basis offsets, certified trim images and ruled boundary walls.
use super::*;
mod wall;

pub(super) fn offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let mut source = match geometry {
        Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), tolerance)?,
        Geometry::Brep(b) if b.faces().len() == 1 => b.clone(),
        _ => {
            return Err(
                viboceros_geometry::GeometryError::UnsupportedLooseSurfaceOffset {
                    context: "a single surface is required",
                }
                .into(),
            );
        }
    };
    if options.solid {
        source = wall::normalized_source_boundaries(&source, tolerance)?;
        // Ruled walls are separate patches at curvature breaks. Split the
        // cap edges first so every wall has a complete certified mating edge.
        let cuts = source
            .edges()
            .iter()
            .enumerate()
            .map(|(i, e)| -> Result<_, CommandError> {
                let c = e.curve();
                let values = c.discontinuity_parameters(
                    viboceros_geometry::CurveContinuity::G2,
                    tolerance.angular(),
                )?;
                Ok((i, values))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .collect::<Vec<_>>();
        source = source.try_split_edges_at_parameters(&cuts, tolerance)?;
    }
    let face = &source.faces()[0];
    let natural_distance = if face.is_reversed() {
        -options.distance
    } else {
        options.distance
    };
    // The public command clamps the active chart of an open sheet before
    // loose interpolation. A closed single-face shell retains cyclic axes.
    let basis = if source.is_solid() {
        face.surface().clone()
    } else {
        face.surface().try_clamped_to_active_domain()?
    };
    let skin = |distance| -> Result<Brep, CommandError> {
        Ok(source.try_with_edited_single_surface(
            basis.try_loose_offset(distance, tolerance)?,
            tolerance,
        )?)
    };
    let outer = skin(natural_distance)?;
    if !options.solid {
        return Ok(Geometry::Brep(outer));
    }
    let inner = if options.both_sides {
        skin(-natural_distance)?
    } else {
        source
    };
    Ok(Geometry::Brep(wall::assemble(&outer, &inner, tolerance)?))
}

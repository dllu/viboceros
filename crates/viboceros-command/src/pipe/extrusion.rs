//! Lightweight output for constant circular sections on a straight rail.
use super::*;

pub(super) fn straight_pipe(
    rail: CurveRef<'_>,
    radii: [Real; 2],
    stations: &[(Real, Real)],
    options: PipeOptions,
    preference: &use_extrusions::Preference,
    tolerance: Tolerance,
) -> Result<Option<Vec<Brep>>, CommandError> {
    if radii[0] != radii[1] || !stations.is_empty() || options.cap == PipeCap::Round {
        return Ok(None);
    }
    // Imported NURBS lines and two-point polylines qualify too.
    let straight = rail.to_nurbs()?;
    if !straight.is_linear(tolerance)? {
        return Ok(None);
    }
    let start = straight.evaluate(*straight.domain().start())?;
    let end = straight.evaluate(*straight.domain().end())?;
    let height = start.distance_to(end)?;
    let frame = Frame3::try_from_normal(start, start.vector_to(end)?, tolerance)?;
    let capped = options.cap == PipeCap::Flat;
    let mut outputs = Vec::with_capacity(2);
    if let Some(thickness) = options.wall_thickness {
        let second = radii[0] + thickness;
        if capped {
            outputs.push(Brep::try_tube_extrusion(
                frame,
                [radii[0], second],
                height,
                true,
                tolerance,
            )?);
        } else {
            // Rhino creates disconnected open walls as separate objects; its
            // explicit constructor emits the second wall first.
            let order = if preference.get().0 {
                [radii[0], second]
            } else {
                [second, radii[0]]
            };
            for radius in order {
                outputs.push(Brep::try_cylinder_extrusion(
                    frame,
                    radius,
                    [0., height],
                    false,
                    tolerance,
                )?);
            }
        }
    } else {
        outputs.push(Brep::try_cylinder_extrusion(
            frame,
            radii[0],
            [0., height],
            capped,
            tolerance,
        )?);
    }
    Ok(Some(
        outputs
            .into_iter()
            .enumerate()
            .map(|(index, brep)| {
                let domain = straight.domain();
                // Native open thick Pipe rebuilds its second wall with a
                // distance-based interval, even when the rail uses another one.
                let second_open_wall = index > 0 && !capped && options.wall_thickness.is_some();
                let brep = if second_open_wall || (*domain.start() == 0. && *domain.end() == height)
                {
                    brep
                } else {
                    brep.try_with_extrusion_path_domain(
                        [*domain.start(), *domain.end()],
                        tolerance,
                    )?
                };
                use_extrusions::output(brep, preference, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

//! Cut a flat sweep at the picked face's intersection with the source plane.
use super::*;
use viboceros_geometry::{
    BoundingBox3, CoplanarFaceExtrusion, Plane, SurfaceBrepIntersectionEvent,
    SurfaceIntersectionOutput, curve_brep_intersection_events,
    surface_brep_intersection_events_with_operation,
};

pub(super) fn truncate(
    source: &Brep,
    cutter: &Brep,
    apex: Point3,
    split_at_tangents: bool,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    let [base] = source.faces() else {
        return Ok(None);
    };
    let Some(plane) = base.surface().plane(tolerance)? else {
        return Ok(None);
    };
    let extent = source.bounds().union(BoundingBox3::from_points([apex])?)?;
    let size = extent.min().distance_to(extent.max())?;
    let rounding = (64. * f64::EPSILON * size).min(tolerance.absolute());
    if plane.signed_distance_to(apex)?.abs() > rounding {
        return Ok(None);
    }
    for control in base.surface().control_points() {
        if plane.signed_distance_to(control.point())?.abs() > rounding {
            return Ok(None);
        }
    }
    if cutter.faces()[0].surface().plane(tolerance)?.is_some() {
        return Ok(None);
    }

    let support = section_plane(plane, extent, tolerance)?;
    let events = surface_brep_intersection_events_with_operation(
        &support,
        cutter,
        tolerance,
        &mut |a, b, tol| -> Result<_, CommandError> {
            Ok(SurfaceIntersectionOutput::Clipped(
                viboceros_smlib::surface_face_intersection_events(a, b, tol)?,
            ))
        },
    )?;
    let mut sections: Vec<_> = events
        .into_iter()
        .filter_map(|event| match event {
            SurfaceBrepIntersectionEvent::Curve(curve) => Some(curve),
            SurfaceBrepIntersectionEvent::Point(_) => None,
        })
        .collect();
    if sections.len() > 1 {
        // The enclosing rectangle can meet cutter branches outside the fan.
        // Use the actual flat sweep's trimmed faces to distinguish them from
        // additional cuts through the sweep.
        let Some(CoplanarFaceExtrusion::Brep(flat_sweep)) =
            source.try_extruded_face_to_coplanar_point(apex, true, split_at_tangents, tolerance)?
        else {
            return Err(CommandError::Usage(NO_SECTION));
        };
        let mut relevant = Vec::new();
        for section in sections {
            if !curve_brep_intersection_events(&section, &flat_sweep, tolerance)?.is_empty() {
                relevant.push(section);
            }
        }
        sections = relevant;
    }
    let [section] = sections.as_slice() else {
        return Err(CommandError::Usage(NO_SECTION));
    };

    // Extend the true plane section in the normal direction. Raising the
    // apex then leaves every radial crossing unchanged in the source plane.
    // The native cut can construct regular topology at a useful height.
    let lift = plane.normal().as_vector().scaled(size)?;
    let curtain = NurbsSurface::try_extruded_curve(section, lift.scaled(-1.)?, lift.scaled(2.)?)?;
    let curtain = Brep::try_surface_face(curtain, tolerance)?;
    let lifted_apex = apex.translated(lift)?;
    let sweep = cone(source, lifted_apex, split_at_tangents, tolerance)?;
    let cut = super::truncate(&sweep, &curtain, lifted_apex, tolerance)?;
    Ok(Some(cut.try_projected_onto_plane(plane, tolerance)?))
}

fn section_plane(
    plane: Plane,
    bounds: BoundingBox3,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    let frame = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let low = bounds.min().to_array();
    let high = bounds.max().to_array();
    let mut minimum = [f64::INFINITY; 2];
    let mut maximum = [f64::NEG_INFINITY; 2];
    for x in [low[0], high[0]] {
        for y in [low[1], high[1]] {
            for z in [low[2], high[2]] {
                let [u, v, _] = frame.coordinates_of(Point3::try_new(x, y, z)?)?;
                minimum[0] = minimum[0].min(u);
                maximum[0] = maximum[0].max(u);
                minimum[1] = minimum[1].min(v);
                maximum[1] = maximum[1].max(v);
            }
        }
    }
    let padding = 8. * tolerance.absolute();
    let [left, bottom] = minimum.map(|x| x - padding);
    let [right, top] = maximum.map(|x| x + padding);
    Ok(NurbsSurface::try_bilinear([
        frame.point_at([left, bottom, 0.])?,
        frame.point_at([right, bottom, 0.])?,
        frame.point_at([right, top, 0.])?,
        frame.point_at([left, top, 0.])?,
    ])?)
}

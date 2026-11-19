//! Cut a regular ruled strip across all its connected faces.
use super::*;
use crate::curve_intersections::brep_events;

pub(super) fn trim(
    stock: &Brep,
    cutter: &Brep,
    plane: Plane,
    displacement: Vector3,
    section: impl Iterator<Item = Point3>,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    // The finite cutter has to span the wall at the cut. The rest of the
    // retained wall, including the base rail, can extend past its footprint.
    let support = covering_support(stock, plane, displacement, tolerance)?;
    let cut = clip_component(stock, &support, displacement, section, tolerance)?;
    let mut sections = 0;
    for edge in cut.edges() {
        let curve = edge.curve();
        if !curve.control_points().iter().all(|control| {
            plane
                .signed_distance_to(control.point())
                .is_ok_and(|distance| distance.abs() <= tolerance.absolute())
        }) {
            continue;
        }
        sections += 1;
        let events = brep_events(curve, cutter, None, tolerance)?;
        let domain = curve.domain();
        if !events.iter().any(|event| {
            matches!(event, CurveBrepIntersectionEvent::Overlap(overlap)
                if overlap.curve_interval() == domain)
        }) {
            return Err(REJECTED);
        }
    }
    if sections != stock.faces().len() {
        return Err(REJECTED);
    }
    Ok(cut)
}

fn covering_support(
    stock: &Brep,
    plane: Plane,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let chart = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let rise = displacement.dot(plane.normal().as_vector())?;
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    // Projection along the sweep is affine. The nonrational stock's control
    // hull encloses its projection, so the tool's sides miss the entire strip.
    for face in stock.faces() {
        for control in face.surface().control_points() {
            let point = control.point();
            let fraction = -plane.signed_distance_to(point)? / rise;
            let projected = point.translated(displacement.scaled(fraction)?)?;
            let xy = chart.projected_coordinates_of(projected)?;
            for axis in 0..2 {
                min[axis] = min[axis].min(xy[axis]);
                max[axis] = max[axis].max(xy[axis]);
            }
        }
    }
    let pad = displacement.length()?.max(tolerance.absolute() * 16.);
    let [left, bottom] = min.map(|v| v - pad);
    let [right, top] = max.map(|v| v + pad);
    let surface = NurbsSurface::try_bilinear([
        chart.point_at([left, bottom, 0.])?,
        chart.point_at([right, bottom, 0.])?,
        chart.point_at([right, top, 0.])?,
        chart.point_at([left, top, 0.])?,
    ])?;
    Ok(Brep::try_surface_face(surface, tolerance)?)
}

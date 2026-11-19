//! Cut the finite swept solid with a tool grown from the selected face.
use super::*;
use crate::extrude_surface::{
    apply_staged_products, capture_source_face, surface_extrusion_targets,
};
use crate::surface_boundary_cut::{BoundaryDifference, hole_crosses_sweep, on_plane};
use viboceros_smlib::KernelCurve;

const REJECTED: CommandError = CommandError::SurfaceExtrusionBoundaryRejected;

pub(super) fn apply(
    doc: &mut Document,
    input: &SurfacePathInput,
    rail: ObjectId,
    path: &NurbsCurve,
    native: &KernelCurve,
    reference: BoundaryRef,
    cutter: &Brep,
) -> Result<usize, CommandError> {
    let tolerance = doc.tolerance();
    let selection = doc
        .selected_object_ids()
        .chain([reference.object, rail])
        .collect::<Vec<_>>();
    let mut staged = Vec::new();
    let mut used = Vec::new();
    let targets = surface_extrusion_targets(doc, &input.faces)?;
    let mut count = 0;
    for &(id, face) in &targets {
        let source = capture_source_face(doc, id, face)?;
        match sweep_face(
            &source,
            path,
            native,
            Some(cutter),
            true,
            input.options.split_at_tangents,
            tolerance,
        ) {
            Ok(product) => {
                staged.push(Geometry::Brep(product.brep));
                if product.finished {
                    count += 1;
                    if input.options.delete_input {
                        used.push((id, face));
                    }
                }
            }
            Err(CommandError::SurfaceExtrusionBoundaryRejected) => {}
            Err(error) => return Err(error),
        }
    }
    if staged.is_empty() {
        doc.set_transient_selection(selection)?;
        return Err(REJECTED);
    }
    let mut release = BTreeSet::from([rail, reference.object]);
    if input.options.subcurve && input.options.path_range.is_some() {
        release.extend(targets.into_iter().map(|(id, _)| id));
    }
    let release = selection.iter().copied().filter(|id| release.contains(id));
    doc.release_command_selection_on_history_replay(release)?;
    apply_staged_products(doc, staged, used.into_iter())?;
    let selection = selection
        .into_iter()
        .filter(|&id| doc.object(id).is_some())
        .collect::<Vec<_>>();
    doc.select_command_results(selection)?;
    if count == 0 { Err(REJECTED) } else { Ok(count) }
}

/// Build the same clipped edges used by the command for an interactive draft.
pub fn preview_boundary_edges(
    sources: &[Brep],
    path: &NurbsCurve,
    cutter: &Brep,
    split_at_tangents: bool,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    let native = KernelCurve::from_nurbs_for_modeling(path)?;
    let mut edges = Vec::new();
    for source in sources {
        if let Ok(product) = sweep_face(
            source,
            path,
            &native,
            Some(cutter),
            true,
            split_at_tangents,
            tolerance,
        ) {
            edges.extend(product.brep.edges().iter().map(|edge| edge.curve().clone()));
        }
    }
    Ok(edges)
}

pub(super) fn limit(
    source: &Brep,
    rail: &NurbsCurve,
    sweep: Brep,
    boundary: &Brep,
    tolerance: Tolerance,
) -> Result<(Brep, bool), CommandError> {
    match cut(source, rail, &sweep, boundary, tolerance) {
        Ok(result) => Ok((result, true)),
        Err(error) => {
            if hole_crosses_sweep(boundary, &sweep, tolerance)? {
                let filled = boundary.try_remove_all_holes(tolerance)?.ok_or(REJECTED)?;
                cut(source, rail, &sweep, &filled, tolerance)?;
                Ok((sweep, false))
            } else {
                Err(error)
            }
        }
    }
}

fn cut(
    source: &Brep,
    rail: &NurbsCurve,
    sweep: &Brep,
    boundary: &Brep,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let domain = rail.domain();
    let travel = rail
        .evaluate(*domain.start())?
        .vector_to(rail.evaluate(*domain.end())?)?;
    let sweep_bounds = sweep.padded_trim_bounds()?;
    let boundary_bounds = boundary.padded_trim_bounds()?;
    let [sweep_min, sweep_max] = [sweep_bounds.min().to_array(), sweep_bounds.max().to_array()];
    let [cut_min, cut_max] = [
        boundary_bounds.min().to_array(),
        boundary_bounds.max().to_array(),
    ];
    let epsilon = tolerance.absolute();
    if (0..3).any(|axis| {
        sweep_max[axis] + epsilon < cut_min[axis] || cut_max[axis] + epsilon < sweep_min[axis]
    }) {
        return Err(REJECTED);
    }
    let bounds = sweep_bounds.union(boundary_bounds)?;
    let tool_length = bounds.min().distance_to(bounds.max())? * 2.;
    let carrier = boundary.faces()[0].surface();
    let base = source.faces()[0].surface();
    let to_end = AffineTransform3::from_translation(travel);
    let end = base.transformed(to_end)?;
    let has_holes = boundary.faces()[0].loops().len() > 1;
    let mut coincident = false;
    let direction = if let Some(plane) = carrier.plane(tolerance)? {
        let normal = plane.normal().as_vector();
        let side = plane.signed_distance_to(source.padded_trim_bounds()?.center()?)?;
        let normal_travel = travel.dot(normal)?;
        let at_start = on_plane(base, plane, tolerance);
        let at_end = on_plane(&end, plane, tolerance);
        coincident = at_start || at_end;
        if coincident && !rim_hits_cutter(source, boundary, at_end.then_some(to_end), tolerance)? {
            return Err(REJECTED);
        }
        let flip = if at_start {
            normal_travel > 0.
        } else {
            side > 0.
        };
        if has_holes && !coincident && normal_travel.abs() > tolerance.absolute() {
            // A cutter swept along a straight rail keeps the hole's extension
            // at the same location within each section of the stock.
            let axis = travel.normalized_nonzero()?.as_vector();
            if flip == (normal_travel < 0.) {
                axis
            } else {
                axis.scaled(-1.)?
            }
        } else if flip {
            normal.scaled(-1.)?
        } else {
            normal
        }
    } else {
        travel.normalized_nonzero()?.as_vector()
    };
    let difference =
        BoundaryDifference::subtract(sweep, boundary, direction.scaled(tool_length)?, tolerance)?
            .ok_or(REJECTED)?;
    if coincident {
        return if difference.result.is_solid() {
            Ok(difference.result)
        } else {
            Err(REJECTED)
        };
    }
    difference.keep_source_section(source, &end, boundary, tolerance)
}

fn rim_hits_cutter(
    source: &Brep,
    boundary: &Brep,
    translate: Option<AffineTransform3>,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let face = &boundary.faces()[0];
    let surface = face.surface();
    for edge in source.edges() {
        let transformed;
        let curve = if let Some(translation) = translate {
            transformed = edge.curve().transformed(translation)?;
            &transformed
        } else {
            edge.curve()
        };
        let point = curve.evaluate(*curve.domain().start())?;
        let (u, v) = surface.closest_parameters(point, tolerance)?;
        if surface.evaluate(u, v)?.is_near(point, tolerance)
            && face.contains_parameters(u, v, tolerance)?
        {
            return Ok(true);
        }
        if !viboceros_geometry::curve_brep_intersection_events(curve, boundary, tolerance)?
            .is_empty()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

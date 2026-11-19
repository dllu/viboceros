use super::*;

pub(super) fn enclosed<'a>(
    source: &BrepFace,
    boundary: &'a Brep,
    inverse: SectionProjection,
    tolerance: Tolerance,
) -> Result<Vec<&'a BrepLoop>, GeometryError> {
    // The caller has checked the whole section rim against the cutter,
    // including all source holes. Each cutter hole can now be classified
    // by one point on its loop.
    let mut holes = Vec::new();
    for hole in &boundary.faces[0].loops[1..] {
        let point = boundary.vertices[hole.trims[0].vertices[0]].point;
        let factor = 1. - inverse.plane.signed_distance_to(point)? / inverse.distance;
        if 0. < factor && factor < 1. {
            let on_source = inverse.project(point, factor)?;
            let (u, v) = source.surface.closest_parameters(on_source, tolerance)?;
            if source.surface.evaluate(u, v)?.is_near(on_source, tolerance)
                && source.contains_parameters(u, v, tolerance)?
            {
                holes.push(hole);
            }
        }
    }
    Ok(holes)
}

/// Transfer exact spatial hole edges into the projected cap chart. If that
/// chart has no supported exact pullback, the caller can use its native cut.
pub(super) fn append(
    sweep: &mut Brep,
    cap: &mut BrepFace,
    boundary: &Brep,
    holes: &[&BrepLoop],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if holes.is_empty() {
        return Ok(true);
    }
    let vertex_offset = sweep.vertices.len();
    let edge_offset = sweep.edges.len();
    for &hole in holes {
        let mut hole = hole.clone();
        for trim in &mut hole.trims {
            let Some(edge) = trim.edge else {
                return Ok(false);
            };
            let Ok(parameter) = cap
                .surface
                .try_pullback_exact_curve(&boundary.edges[edge].curve, tolerance)
            else {
                return Ok(false);
            };
            trim.curve = if trim.reversed_3d {
                parameter.reversed()?
            } else {
                parameter
            };
            trim.vertices = trim.vertices.map(|vertex| vertex + vertex_offset);
            trim.edge = Some(edge + edge_offset);
            trim.iso = SurfaceIso::NotIso;
            trim.tolerance = [0.; 2];
        }
        if sampled_loop_signed_area(&hole)? > 0. {
            hole.trims.reverse();
            for trim in &mut hole.trims {
                trim.vertices.swap(0, 1);
                trim.reversed_3d = !trim.reversed_3d;
                trim.curve = trim.curve.reversed()?;
            }
        }
        cap.loops.push(hole);
    }
    // The sweep's face compaction discards cutter edges unused by the cap.
    sweep.vertices.extend_from_slice(&boundary.vertices);
    sweep
        .edges
        .extend(boundary.edges.iter().cloned().map(|mut edge| {
            edge.vertices = edge.vertices.map(|vertex| vertex + vertex_offset);
            edge
        }));
    Ok(true)
}

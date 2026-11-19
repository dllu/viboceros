//! Refine long chords inside a trimmed face without changing shared edges.
use std::collections::HashSet;

use super::audit::key as vertex_location;
use super::*;

fn edge(a: u32, b: u32) -> [u32; 2] {
    [a.min(b), a.max(b)]
}

fn distance_to_chord(point: Point3, a: Point3, b: Point3) -> Result<Real, GeometryError> {
    let axis = a.vector_to(b)?;
    let length_squared = axis.dot(axis)?;
    if length_squared == 0. {
        return point.distance_to(a);
    }
    let position = (a.vector_to(point)?.dot(axis)? / length_squared).clamp(0., 1.);
    point.distance_to(a.translated(axis.scaled(position)?)?)
}

pub(super) fn refinement_weight(
    surface: &NurbsSurface,
    parameters: &[Point2],
    points: &[Point3],
    triangles: &[[u32; 3]],
    chord_height: Real,
) -> Result<u64, GeometryError> {
    const PROBE_TRIANGLES: usize = 96;
    let stride = triangles.len().div_ceil(PROBE_TRIANGLES).max(1);
    let mut work = 0.;
    for &[a, b, c] in triangles.iter().step_by(stride) {
        for [i, j] in [[a, b], [b, c], [c, a]] {
            let [i, j] = [i, j].map(|index| index as usize);
            let [u, v] = [parameters[i], parameters[j]];
            let point = surface.evaluate(u.x().midpoint(v.x()), u.y().midpoint(v.y()))?;
            let ratio = distance_to_chord(point, points[i], points[j])? / chord_height;
            if ratio > 1. {
                // A smooth arc's deviation decreases quadratically as
                // its station spacing decreases.
                work += ratio.sqrt().min(16.);
            }
        }
    }
    Ok((work * stride as Real * 0.5).ceil() as u64)
}

pub(super) fn refine_interior(
    surface: &NurbsSurface,
    parameters: &mut Vec<Point2>,
    points: &mut Vec<Point3>,
    triangles: &mut Vec<[u32; 3]>,
    chord_height: Real,
    limit: usize,
    locations: &mut HashSet<[u64; 3]>,
) -> Result<Option<Real>, GeometryError> {
    loop {
        let mut edges = triangles
            .iter()
            .flat_map(|&[a, b, c]| [edge(a, b), edge(b, c), edge(c, a)])
            .collect::<Vec<_>>();
        edges.sort_unstable();
        let mut proposals = Vec::new();
        for incidence in edges.chunk_by(|a, b| a == b) {
            // A one-sided edge is an installed face boundary. Inserting a
            // vertex there would make a T-junction on the mating face.
            if incidence.len() != 2 {
                continue;
            }
            let key = incidence[0];
            let [a, b] = key.map(|i| i as usize);
            let [ua, ub] = [parameters[a], parameters[b]];
            let uv = Point2::try_new(ua.x().midpoint(ub.x()), ua.y().midpoint(ub.y()))?;
            if uv == ua || uv == ub {
                continue;
            }
            let point = surface.evaluate(uv.x(), uv.y())?;
            if locations.contains(&vertex_location(point)) {
                continue;
            }
            let error = distance_to_chord(point, points[a], points[b])?;
            if error > chord_height {
                proposals.push((error, key, uv, point));
            }
        }
        if proposals.is_empty() {
            return Ok(None);
        }
        let available = limit.saturating_sub(parameters.len());
        if available == 0 {
            return Ok(proposals
                .iter()
                .map(|proposal| proposal.0)
                .max_by(Real::total_cmp));
        }
        proposals.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        // The final topology welds exactly coincident locations. Two new
        // splits at the same location can accidentally weld unrelated sides.
        let mut proposed_locations = HashSet::new();
        proposals.retain(|&(_, _, _, point)| proposed_locations.insert(vertex_location(point)));
        proposals.truncate(available);
        let coarse = points.len();
        let mut cuts = BTreeMap::new();
        for &(_, key, _, point) in &proposals {
            cuts.insert(key, points.len() as u32);
            points.push(point);
        }
        keep_nondegenerate_cuts(points, triangles, &mut cuts)?;
        points.truncate(coarse);
        proposals.retain(|(_, key, _, _)| cuts.contains_key(key));
        if proposals.is_empty() {
            return Ok(None);
        }
        cuts.clear();
        for (_, key, uv, point) in proposals {
            cuts.insert(key, parameters.len() as u32);
            parameters.push(uv);
            points.push(point);
            locations.insert(vertex_location(point));
        }
        let original = std::mem::take(triangles);
        triangles.reserve(original.len() + cuts.len() * 2);
        for triangle in original {
            let mids = triangle_edges(triangle).map(|key| cuts.get(&key).copied());
            split(triangle, mids, triangles);
        }
    }
}

fn keep_nondegenerate_cuts(
    points: &[Point3],
    triangles: &[[u32; 3]],
    cuts: &mut BTreeMap<[u32; 2], u32>,
) -> Result<(), GeometryError> {
    use crate::nurbs_surface::tessellation_triangle_is_nondegenerate as valid;
    let tolerance = Tolerance::MESH_VALIDATION;
    let mut children = Vec::with_capacity(4);
    for &triangle in triangles {
        let edges = triangle_edges(triangle);
        let mids = edges.map(|key| cuts.get(&key).copied());
        let active = usize::from(mids[0].is_some())
            | usize::from(mids[1].is_some()) << 1
            | usize::from(mids[2].is_some()) << 2;
        if active == 0 {
            continue;
        }
        let mut preserves_topology = valid(points, triangle, tolerance)?;
        // A neighboring triangle may reject a shared cut. Check each
        // possible subset so its removal cannot leave an invalid split.
        for mask in 1..=active {
            if mask & active != mask || !preserves_topology {
                continue;
            }
            children.clear();
            let selected =
                std::array::from_fn(|i| if mask & (1 << i) != 0 { mids[i] } else { None });
            split(triangle, selected, &mut children);
            for &child in &children {
                preserves_topology &= valid(points, child, tolerance)?;
            }
        }
        if !preserves_topology {
            for key in edges {
                cuts.remove(&key);
            }
        }
    }
    Ok(())
}

fn triangle_edges([a, b, c]: [u32; 3]) -> [[u32; 2]; 3] {
    [edge(a, b), edge(b, c), edge(c, a)]
}

fn split(triangle: [u32; 3], mut mids: [Option<u32>; 3], output: &mut Vec<[u32; 3]>) {
    let [a, b, c] = triangle;
    if let [Some(ab), Some(bc), Some(ca)] = mids {
        output.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        return;
    }
    let mut t = triangle;
    // Rotate to place the only split first, or the two splits consecutively.
    while mids[0].is_none() && (mids[1].is_some() || mids[2].is_some())
        || mids[0].is_some() && mids[1].is_none() && mids[2].is_some()
    {
        t.rotate_left(1);
        mids.rotate_left(1);
    }
    let [a, b, c] = t;
    match mids {
        [Some(ab), Some(bc), None] => output.extend([[a, ab, c], [ab, bc, c], [ab, b, bc]]),
        [Some(ab), None, None] => output.extend([[a, ab, c], [ab, b, c]]),
        _ => output.push(triangle),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_split_that_collapses_a_child_is_rejected_on_both_incident_triangles() {
        let points = [[0., 0.], [2., 0.], [1., 1.], [1., 1.], [1., -1.]]
            .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap());
        let mut cuts = BTreeMap::from([([0, 1], 3)]);
        keep_nondegenerate_cuts(&points, &[[0, 1, 2], [1, 0, 4]], &mut cuts).unwrap();
        assert!(cuts.is_empty());
    }

    #[test]
    fn interior_chord_refines_both_incident_triangles_without_splitting_trims() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 8., 0., 3.).unwrap();
        let [start, end] = [*surface.domain_u().start(), *surface.domain_u().end()];
        let quarter = start + (end - start) * 0.25;
        let mut uv = [[start, 0.], [quarter, 0.], [quarter, 3.], [start, 3.]]
            .map(|[u, v]| Point2::try_new(u, v).unwrap())
            .to_vec();
        let mut points: Vec<_> = uv
            .iter()
            .map(|p| surface.evaluate(p.x(), p.y()).unwrap())
            .collect();
        let mut triangles = vec![[0, 1, 2], [0, 2, 3]];
        let mut locations: HashSet<_> = points.iter().copied().map(vertex_location).collect();
        let shared_location =
            vertex_location(surface.evaluate(start.midpoint(quarter), 1.5).unwrap());
        locations.insert(shared_location);
        refine_interior(
            &surface,
            &mut uv,
            &mut points,
            &mut triangles,
            0.1,
            5,
            &mut locations,
        )
        .unwrap();
        assert_eq!(uv.len(), 4);
        assert_eq!(triangles, [[0, 1, 2], [0, 2, 3]]);

        locations.remove(&shared_location);
        refine_interior(
            &surface,
            &mut uv,
            &mut points,
            &mut triangles,
            0.1,
            5,
            &mut locations,
        )
        .unwrap();
        assert_eq!(uv.len(), 5);
        assert_eq!(triangles.len(), 4);
        assert!(triangles.iter().all(|triangle| triangle.contains(&4)));
        let mut uses = BTreeMap::new();
        for [a, b, c] in triangles {
            for side in [edge(a, b), edge(b, c), edge(c, a)] {
                *uses.entry(side).or_insert(0) += 1;
            }
        }
        assert_eq!(
            uses.iter()
                .filter(|&(_, &n)| n == 1)
                .map(|(&k, _)| k)
                .collect::<Vec<_>>(),
            [[0, 1], [0, 3], [1, 2], [2, 3]]
        );
    }
}

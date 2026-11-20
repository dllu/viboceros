//! Refine long chords on shared edges and within trimmed faces.
use std::collections::{HashMap, HashSet};

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

pub(super) fn refine_display_edge(
    curve: &NurbsCurve,
    stations: Vec<Real>,
    tolerance: Tolerance,
) -> Result<Vec<Real>, GeometryError> {
    if curve.degree() == 1 {
        return Ok(stations);
    }
    let bounds = curve.control_point_bounds();
    let height = (bounds.min().distance_to(bounds.max())? * 0.001).max(tolerance.absolute());
    let mut refined = Vec::with_capacity(stations.len());
    if let Some(&first) = stations.first() {
        refined.push(first);
        let mut start = (first, curve.evaluate(first)?);
        for end in stations.into_iter().skip(1) {
            let end = (end, curve.evaluate(end)?);
            append_edge_stations(curve, start, end, height, 0, &mut refined)?;
            start = end;
        }
    }
    Ok(refined)
}

fn append_edge_stations(
    curve: &NurbsCurve,
    start: (Real, Point3),
    end: (Real, Point3),
    height: Real,
    depth: u8,
    output: &mut Vec<Real>,
) -> Result<(), GeometryError> {
    const MAX_DEPTH: u8 = 12;
    let middle = start.0.midpoint(end.0);
    if depth < MAX_DEPTH && start.0 < middle && middle < end.0 {
        let point = curve.evaluate(middle)?;
        // Quarter stations detect bends on both sides of an inflection even
        // if the curve midpoint lies on the chord.
        if distance_to_chord(point, start.1, end.1)? > height
            || distance_to_chord(curve.evaluate(start.0.midpoint(middle))?, start.1, end.1)?
                > height
            || distance_to_chord(curve.evaluate(middle.midpoint(end.0))?, start.1, end.1)? > height
        {
            append_edge_stations(curve, start, (middle, point), height, depth + 1, output)?;
            return append_edge_stations(curve, (middle, point), end, height, depth + 1, output);
        }
    }
    if output.len() >= MAX_CONSTRAINED_TRIM_VERTICES {
        return Err(GeometryError::MeshSubdivisionLimit);
    }
    output.push(end.0);
    Ok(())
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
    let mut evaluate = surface.point_evaluator();
    let mut work = 0.;
    for &[a, b, c] in triangles.iter().step_by(stride) {
        for [i, j] in [[a, b], [b, c], [c, a]] {
            let [i, j] = [i, j].map(|index| index as usize);
            let [u, v] = [parameters[i], parameters[j]];
            let point = evaluate(u.x().midpoint(v.x()), u.y().midpoint(v.y()))?;
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

#[derive(Clone, Copy)]
struct InteriorProbe {
    error: Real,
    uv: Point2,
    point: Point3,
}

fn probe_interior_edge(
    evaluate: &mut impl FnMut(Real, Real) -> Result<Point3, GeometryError>,
    [ua, ub]: [Point2; 2],
    [a, b]: [Point3; 2],
    height: Real,
    locations: &HashSet<[u64; 3]>,
) -> Result<Option<InteriorProbe>, GeometryError> {
    let uv = Point2::try_new(ua.x().midpoint(ub.x()), ua.y().midpoint(ub.y()))?;
    if uv == ua || uv == ub {
        return Ok(None);
    }
    let point = evaluate(uv.x(), uv.y())?;
    if locations.contains(&vertex_location(point)) {
        return Ok(None);
    }
    let mut error = distance_to_chord(point, a, b)?;
    // A long edge can cross an inflection with its midpoint on the chord.
    // Probe both halves before accepting it, and bisect at the midpoint so
    // each of the two new edges addresses a different part of the bend.
    if error <= height && a.distance_to(b)? > 32. * height {
        for end in [ua, ub] {
            let sample = evaluate(end.x().midpoint(uv.x()), end.y().midpoint(uv.y()))?;
            error = error.max(distance_to_chord(sample, a, b)?);
        }
    }
    Ok((error > height).then_some(InteriorProbe { error, uv, point }))
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
    const CACHED_EDGE_LIMIT: usize = 65_536;
    // Vertex indices and UV values are stable during refinement. Reuse probes
    // for edges that survive several rounds, especially on large supports.
    let cache_limit = if triangles.len() >= 512 {
        CACHED_EDGE_LIMIT
    } else {
        0
    };
    let mut evaluate = surface.point_evaluator();
    let mut probes = HashMap::new();
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
            let probe = if let Some(&saved) = probes.get(&key) {
                saved
            } else {
                let probe = probe_interior_edge(
                    &mut evaluate,
                    [parameters[a], parameters[b]],
                    [points[a], points[b]],
                    chord_height,
                    locations,
                )?;
                if probes.len() < cache_limit {
                    probes.insert(key, probe);
                }
                probe
            };
            if let Some(probe) = probe
                && !locations.contains(&vertex_location(probe.point))
            {
                proposals.push((probe.error, key, probe.uv, probe.point));
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
            split(points, triangle, mids, triangles)?;
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
        let mut active = usize::from(mids[0].is_some())
            | usize::from(mids[1].is_some()) << 1
            | usize::from(mids[2].is_some()) << 2;
        if active == 0 {
            continue;
        }
        if !valid(points, triangle, tolerance)? {
            for key in edges {
                cuts.remove(&key);
            }
            continue;
        }
        // A neighboring triangle may reject a shared cut. Check each
        // possible subset so its removal cannot leave an invalid split.
        for mask in 1..=active {
            if mask & active != mask {
                continue;
            }
            children.clear();
            let selected =
                std::array::from_fn(|i| if mask & (1 << i) != 0 { mids[i] } else { None });
            split(points, triangle, selected, &mut children)?;
            let mut preserves_topology = true;
            for &child in &children {
                preserves_topology &= valid(points, child, tolerance)?;
            }
            if !preserves_topology {
                // The provisional vertices are ordered by descending chord
                // error. Remove the lowest-priority cut in the bad subset.
                let (slot, _) = selected
                    .into_iter()
                    .enumerate()
                    .filter_map(|(slot, index)| index.map(|index| (slot, index)))
                    .max_by_key(|&(_, index)| index)
                    .expect("nonempty active split subset");
                active &= !(1 << slot);
                cuts.remove(&edges[slot]);
            }
        }
    }
    Ok(())
}

fn triangle_edges([a, b, c]: [u32; 3]) -> [[u32; 2]; 3] {
    [edge(a, b), edge(b, c), edge(c, a)]
}

fn split(
    points: &[Point3],
    triangle: [u32; 3],
    mut mids: [Option<u32>; 3],
    output: &mut Vec<[u32; 3]>,
) -> Result<(), GeometryError> {
    let [a, b, c] = triangle;
    if let [Some(ab), Some(bc), Some(ca)] = mids {
        output.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
        return Ok(());
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
        [Some(ab), Some(bc), None] => {
            use crate::nurbs_surface::tessellation_triangle_is_nondegenerate as valid;
            let [pa, pab, pbc, pc] = [a, ab, bc, c].map(|index| points[index as usize]);
            let forward = [[a, ab, c], [ab, bc, c]];
            let backward = [[a, ab, bc], [a, bc, c]];
            let [preferred, other] = if pa.distance_to(pbc)? < pab.distance_to(pc)? {
                [backward, forward]
            } else {
                [forward, backward]
            };
            // Use the shorter diagonal when both lifted triangles are valid.
            // If it collapses a triangle, try the other diagonal before
            // letting the topology checks reject either shared cut.
            let tolerance = Tolerance::MESH_VALIDATION;
            if valid(points, preferred[0], tolerance)? && valid(points, preferred[1], tolerance)? {
                output.extend(preferred);
            } else {
                output.extend(other);
            }
            output.push([ab, b, bc]);
        }
        [Some(ab), None, None] => output.extend([[a, ab, c], [ab, b, c]]),
        _ => output.push(triangle),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_cut_leaves_a_valid_cut_on_the_same_triangle() {
        let points = [[0., 0.], [1., 0.], [0., 1.], [0.5, 0.], [0.75, 0.]]
            .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap());
        let mut cuts = BTreeMap::from([([0, 1], 3), ([1, 2], 4)]);
        keep_nondegenerate_cuts(&points, &[[0, 1, 2]], &mut cuts).unwrap();
        assert_eq!(cuts, BTreeMap::from([([0, 1], 3)]));
    }

    #[test]
    fn display_refines_curved_seam_chords_and_retains_existing_stations() {
        let tolerance = Tolerance::DEFAULT;
        let circle = crate::Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            50.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized(tolerance)
                .unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let domain = circle.domain();
        let start = *domain.start();
        let end = *domain.end();
        let original: Vec<_> = circle
            .spans()
            .flat_map(|(a, b)| [a, a.midpoint(b)])
            .chain([end])
            .collect();
        let refined = refine_display_edge(&circle, original.clone(), tolerance).unwrap();
        assert_eq!((refined[0], refined[refined.len() - 1]), (start, end));
        assert!(original.iter().all(|t| refined.contains(t)));
        assert!(refined.len() > original.len());
        for pair in refined.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            let middle = circle.evaluate(a.midpoint(b)).unwrap();
            let gap = distance_to_chord(
                middle,
                circle.evaluate(a).unwrap(),
                circle.evaluate(b).unwrap(),
            )
            .unwrap();
            assert!(gap <= 0.142);
        }
    }

    #[test]
    fn a_split_that_collapses_a_child_is_rejected_on_both_incident_triangles() {
        let points = [[0., 0.], [2., 0.], [1., 1.], [1., 1.], [1., -1.]]
            .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap());
        let mut cuts = BTreeMap::from([([0, 1], 3)]);
        keep_nondegenerate_cuts(&points, &[[0, 1, 2], [1, 0, 4]], &mut cuts).unwrap();
        assert!(cuts.is_empty());
    }

    #[test]
    fn two_cuts_choose_the_shorter_physical_diagonal() {
        let points = [[0., 0.], [4., 0.], [3., 5.], [2., 0.], [3.5, 2.5]]
            .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap());
        let mut children = Vec::new();
        split(&points, [0, 1, 2], [Some(3), Some(4), None], &mut children).unwrap();
        assert_eq!(children, [[0, 3, 4], [0, 4, 2], [3, 1, 4]]);
    }

    #[test]
    fn two_cuts_keep_the_valid_diagonal_when_the_shorter_one_collapses() {
        let points = [[-1., 0.], [4., 0.], [2., 3.], [2., 0.], [2., 1.5]]
            .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap());
        let mut cuts = BTreeMap::from([([0, 1], 3), ([1, 2], 4)]);
        keep_nondegenerate_cuts(&points, &[[0, 1, 2]], &mut cuts).unwrap();
        assert_eq!(cuts, BTreeMap::from([([0, 1], 3), ([1, 2], 4)]));
        let mut children = Vec::new();
        split(&points, [0, 1, 2], [Some(3), Some(4), None], &mut children).unwrap();
        assert_eq!(children, [[0, 3, 4], [0, 4, 2], [3, 1, 4]]);
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

    #[test]
    fn interior_chord_detects_an_inflection_whose_midpoint_is_on_the_chord() {
        let net = [0., 2.]
            .into_iter()
            .flat_map(|z| {
                [[0., 0.], [2., 5.], [4., -5.], [6., 0.]]
                    .map(move |[x, y]| Point3::try_new(x, y, z).unwrap())
            })
            .collect();
        let surface = NurbsSurface::try_new(
            3,
            1,
            4,
            2,
            net,
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let mut uv = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
            .map(|[u, v]| Point2::try_new(u, v).unwrap())
            .to_vec();
        let mut points: Vec<_> = uv
            .iter()
            .map(|p| surface.evaluate(p.x(), p.y()).unwrap())
            .collect();
        let midpoint = surface.evaluate(0.5, 0.5).unwrap();
        assert!(distance_to_chord(midpoint, points[0], points[2]).unwrap() < 1e-12);
        let mut locations = points.iter().copied().map(vertex_location).collect();
        let mut triangles = vec![[0, 1, 2], [0, 2, 3]];
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
        assert_eq!(points[4], midpoint);
        assert_eq!(triangles, [[2, 4, 1], [4, 0, 1], [0, 4, 3], [4, 2, 3]]);
    }
}

//! Union actual triangle-pair overlaps, excluding each face's own coverage.
use super::*;

pub(in crate::native::mesh_intersections) fn append(
    vertices: &[Point3],
    faces: &[u32],
    pairs: &[u32],
    result: &mut MeshIntersections,
) -> Result<(), Error> {
    let count = faces.len() / 3;
    let mut graph = vec![Vec::new(); count];
    for (i, pair) in pairs.chunks_exact(4).enumerate() {
        let (a, b) = (pair[0] as usize, pair[1] as usize);
        if a >= b || b >= count || pair[2] > 7 || pair[3] > 7 {
            return Err(Error::InvalidOutput);
        }
        graph[a].push((b, i));
        graph[b].push((a, i));
    }
    let mut visited = vec![false; count];
    let mut used_pair = vec![false; pairs.len() / 4];
    let mut boundary_count = 0;
    for start in 0..count {
        if visited[start] || graph[start].is_empty() {
            continue;
        }
        let a = triangle(vertices, faces, start)?;
        let normal = a[0]
            .vector_to(a[1])?
            .cross(a[0].vector_to(a[2])?)?
            .normalized_nonzero()?;
        let frame = Frame3::try_from_unit_normal(a[0], normal)?;
        let mut regions = Vec::new();
        let mut region_points = 0;
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        while let Some(index) = queue.pop_front() {
            for &(next, pair_index) in &graph[index] {
                if !used_pair[pair_index] {
                    used_pair[pair_index] = true;
                    let pair = &pairs[4 * pair_index..4 * pair_index + 4];
                    let a = project(triangle(vertices, faces, pair[0] as usize)?, frame)?;
                    let b = project(triangle(vertices, faces, pair[1] as usize)?, frame)?;
                    let overlap = intersection(&[a], &[b]);
                    region_points += overlap.iter().flatten().map(Vec::len).sum::<usize>();
                    if region_points > 1_000_000 {
                        return Err(Error::InvalidOutput);
                    }
                    regions.extend(overlap.into_iter().flatten());
                }
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        let mut options = OverlayOptions::<f64, i64>::default();
        options.preserve_input_collinear = true;
        options.preserve_output_collinear = true;
        let empty: Vec<Path> = Vec::new();
        let shapes = FloatOverlay::<[f64; 2], i64>::from_subj_and_clip_custom(
            &regions,
            &empty,
            options,
            Default::default(),
        )
        .overlay(OverlayRule::Subject, FillRule::NonZero);
        for path in shapes.into_iter().flatten() {
            if path.len() < 3 {
                continue;
            }
            boundary_count += path.len();
            if boundary_count > 1_000_000 || result.coplanar.len() >= 1_000_000 {
                return Err(Error::InvalidOutput);
            }
            let mut points = path
                .into_iter()
                .map(|[x, y]| frame.point_at([x, y, 0.]))
                .collect::<Result<Vec<_>, _>>()?;
            points.push(points[0]);
            result
                .coplanar
                .push(Polyline3::try_new(points, Tolerance::MESH_VALIDATION)?);
        }
    }
    Ok(())
}

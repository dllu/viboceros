use super::*;

/// Rhino leaves an entirely planar B-rep alone, but can cap flat disconnected
/// pieces inside a nonplanar container (including zero-volume double sheets).
pub(super) fn is_planar(
    brep: &Brep,
    frame: Frame3,
    tolerance: Tolerance,
    budget: &mut WorkBudget,
) -> Result<bool, GeometryError> {
    for face in &brep.faces {
        budget.charge(face.surface.control_points().len())?;
        for p in face.surface.control_points() {
            if frame.coordinates_of(p.point())?[2].abs() > tolerance.absolute() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Follow only unambiguous cycles opposite the adjacent oriented face trims.
/// Ambiguous, open or inconsistently oriented components are not cap boundaries.
pub(super) fn cycles(brep: &Brep) -> Vec<Vec<(usize, bool)>> {
    let counts = brep.edge_use_counts();
    let mut direction = vec![None; brep.edges.len()];
    let mut outgoing = vec![Vec::new(); brep.vertices.len()];
    let mut incoming = vec![0; brep.vertices.len()];
    for face in &brep.faces {
        for trim in face.loops.iter().flat_map(|l| &l.trims) {
            if let Some(edge) = trim.edge
                && counts[edge] == 1
            {
                let reversed = !(trim.reversed_3d ^ face.reversed);
                direction[edge] = Some(reversed);
                let vertices = oriented_edge_vertices(&brep.edges[edge], reversed);
                outgoing[vertices[0]].push(edge);
                incoming[vertices[1]] += 1;
            }
        }
    }
    let mut visited = vec![false; brep.edges.len()];
    let mut result = Vec::new();
    for seed in 0..brep.edges.len() {
        if direction[seed].is_none() || visited[seed] {
            continue;
        }
        let mut edge = seed;
        let mut chain = Vec::new();
        loop {
            if visited[edge] {
                break;
            }
            visited[edge] = true;
            let reversed = direction[edge].expect("boundary edge");
            let vertices = oriented_edge_vertices(&brep.edges[edge], reversed);
            if vertices
                .iter()
                .any(|&v| outgoing[v].len() != 1 || incoming[v] != 1)
            {
                break;
            }
            chain.push((edge, reversed));
            edge = outgoing[vertices[1]][0];
            if edge == seed {
                result.push(chain);
                break;
            }
        }
    }
    result
}

pub(super) fn frame(
    points: &[Point3],
    tolerance: Tolerance,
) -> Result<Option<Frame3>, GeometryError> {
    let Some(&origin) = points.first() else {
        return Ok(None);
    };
    let offsets = points
        .iter()
        .map(|&p| origin.vector_to(p))
        .collect::<Result<Vec<_>, _>>()?;
    let scale = offsets
        .iter()
        .flat_map(|v| v.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    let normalized = offsets
        .iter()
        .map(|v| Vector3::try_from(v.to_array().map(|x| x / scale)))
        .collect::<Result<Vec<_>, _>>()?;
    let first = *normalized
        .iter()
        .max_by(|a, b| a.length().unwrap().total_cmp(&b.length().unwrap()))
        .expect("nonempty offsets");
    let mut best = None;
    let mut area = 0.0;
    for second in normalized {
        let cross = first.cross(second)?;
        let length = cross.length()?;
        if length > area {
            area = length;
            best = Some(cross);
        }
    }
    let Some(normal) = best else {
        return Ok(None);
    };
    Frame3::try_from_normal(origin, normal.normalized_nonzero()?.as_vector(), tolerance).map(Some)
}

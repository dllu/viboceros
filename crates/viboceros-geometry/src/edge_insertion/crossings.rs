//! Trace only face-connected crossings in a projected segment's forward direction.
use super::*;

pub(super) fn trace(
    plan: &Plan,
    faces: &[Vec<u32>],
    pair: &[PolygonEdgePoint],
    ends: &[u32],
    normal: Option<UnitVector3>,
) -> Result<Vec<PolygonEdgePoint>, GeometryError> {
    let origin = plan.points[ends[0] as usize];
    let normal = match normal {
        Some(n) => n,
        None => {
            let face = &faces[plan.edge_faces[pair[0].edge][0]];
            let first = plan.points[face[0] as usize];
            face[1..]
                .windows(2)
                .find_map(|v| {
                    first
                        .vector_to(plan.points[v[0] as usize])
                        .ok()?
                        .cross(first.vector_to(plan.points[v[1] as usize]).ok()?)
                        .ok()?
                        .normalized_nonzero()
                        .ok()
                })
                .ok_or_else(|| invalid("degenerate insertion projection"))?
        }
    };
    let delta = origin.vector_to(plan.points[ends[1] as usize])?;
    let y = normal.as_vector().cross(delta)?.normalized_nonzero()?;
    let x = y
        .as_vector()
        .cross(normal.as_vector())?
        .normalized_nonzero()?;
    let length = delta.dot(x.as_vector())?;
    if length <= 0. {
        return Err(invalid("degenerate insertion projection"));
    }
    let project = |p: Point3| -> Result<[Real; 2], GeometryError> {
        let d = origin.vector_to(p)?;
        Ok([
            d.dot(x.as_vector())? / length,
            d.dot(y.as_vector())? / length,
        ])
    };
    let mut nodes = vec![(0., pair[0]), (1., pair[1])];
    let mut owners = vec![
        plan.edge_faces[pair[0].edge].clone(),
        plan.edge_faces[pair[1].edge].clone(),
    ];
    let mut vertex_nodes = BTreeMap::new();
    for (edge, &[va, vb]) in plan.edges.iter().enumerate() {
        if edge == pair[0].edge || edge == pair[1].edge {
            continue;
        }
        let a = project(plan.points[va as usize])?;
        let b = project(plan.points[vb as usize])?;
        if a[1] == b[1] {
            continue;
        }
        let t = -a[1] / (b[1] - a[1]);
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let along = (b[0] - a[0]).mul_add(t, a[0]);
        if along > 0. && along < 1. {
            let vertex = if t == 0. {
                Some(va)
            } else if t == 1. {
                Some(vb)
            } else {
                None
            };
            if let Some(node) = vertex.and_then(|v| vertex_nodes.get(&v).copied()) {
                let row: &mut Vec<usize> = &mut owners[node];
                row.extend(&plan.edge_faces[edge]);
                row.sort_unstable();
                row.dedup();
                continue;
            }
            if let Some(vertex) = vertex {
                vertex_nodes.insert(vertex, nodes.len());
            }
            nodes.push((along, PolygonEdgePoint { edge, parameter: t }));
            owners.push(plan.edge_faces[edge].clone());
        }
    }
    let mut face_nodes = vec![Vec::new(); faces.len()];
    for (node, owner) in owners.iter().enumerate() {
        for &face in owner {
            face_nodes[face].push(node);
        }
    }
    let mut graph = vec![Vec::new(); nodes.len()];
    for (face, mut row) in face_nodes.into_iter().enumerate() {
        if row.len() < 2 {
            continue;
        }
        row.sort_by(|&a, &b| nodes[a].0.total_cmp(&nodes[b].0));
        let polygon = faces[face]
            .iter()
            .map(|&i| project(plan.points[i as usize]))
            .collect::<Result<Vec<_>, _>>()?;
        for pair in row.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            if nodes[b].0 - nodes[a].0 <= 64. * Real::EPSILON {
                continue;
            }
            let midpoint = [(nodes[a].0 + nodes[b].0) * 0.5, 0.];
            if crate::mesh::point_in_mesh_hole_polygon(midpoint, &polygon, 64. * Real::EPSILON) {
                graph[a].push(b);
            }
        }
    }
    let mut previous = vec![None; nodes.len()];
    previous[0] = Some(0);
    let mut queue = VecDeque::from([0]);
    while let Some(a) = queue.pop_front() {
        for &b in &graph[a] {
            if previous[b].is_none() {
                previous[b] = Some(a);
                queue.push_back(b);
            }
        }
    }
    if previous[1].is_none() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut i = previous[1].unwrap();
    while i != 0 {
        out.push(nodes[i].1);
        i = previous[i].unwrap();
    }
    out.reverse();
    Ok(out)
}

use super::*;

pub(super) fn assemble(
    source: &Brep,
    pairs: &[(usize, usize, bool)],
    bounds: &[Real],
    distance: Real,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let mut uses = vec![Vec::new(); source.edges.len()];
    for usage in source.trim_uses() {
        if let Some(edge) = usage.trim.edge {
            uses[edge].push(usage);
        }
    }
    let mut adjacency = vec![Vec::new(); source.faces.len()];
    let mut connect = |a: TrimUse<'_>, b: TrimUse<'_>, reversed: bool| {
        let parity = true
            ^ a.trim.reversed_3d
            ^ b.trim.reversed_3d
            ^ reversed
            ^ source.faces[a.face].reversed
            ^ source.faces[b.face].reversed;
        adjacency[a.face].push((b.face, parity));
        adjacency[b.face].push((a.face, parity));
    };
    for edge_uses in &uses {
        if let [a, b] = edge_uses.as_slice() {
            connect(*a, *b, false);
        }
    }
    let mut roots = (0..source.vertices.len()).collect::<Vec<_>>();
    let mut edge_target = (0..source.edges.len())
        .map(|i| (i, false))
        .collect::<Vec<_>>();
    let mut edge_tolerances = source.edges.iter().map(|e| e.tolerance).collect::<Vec<_>>();
    let mut joined_type = vec![None; source.edges.len()];
    for (&(a, b, reversed), &bound) in pairs.iter().zip(bounds) {
        connect(uses[a][0], uses[b][0], reversed);
        edge_target[b] = (a, reversed);
        let previous = if bound == 0. {
            edge_tolerances[b]
        } else {
            edge_tolerances[b].max(tolerance.absolute())
        };
        edge_tolerances[a] = edge_tolerances[a].max(certificate::add_bound(previous, bound)?);
        let seam =
            uses[a][0].face == uses[b][0].face && uses[a][0].face_loop == uses[b][0].face_loop;
        joined_type[a] = Some(if seam {
            BrepTrimType::Seam
        } else {
            BrepTrimType::Mated
        });
        let av = source.edges[a].vertices;
        let bv = oriented_edge_vertices(&source.edges[b], reversed);
        for (a, b) in av.into_iter().zip(bv) {
            let a = root(&mut roots, a);
            let b = root(&mut roots, b);
            roots[a.max(b)] = a.min(b);
        }
    }
    let flips = orientation(&adjacency)?;
    let mut vertices = Vec::new();
    let mut vertex_map = vec![0; source.vertices.len()];
    for (i, vertex) in source.vertices.iter().enumerate() {
        let r = root(&mut roots, i);
        if r == i {
            vertex_map[i] = vertices.len();
            vertices.push(*vertex);
        } else {
            vertex_map[i] = vertex_map[r];
            let retained = &mut vertices[vertex_map[i]];
            let bound = certificate::point_bound(vertex.point, retained.point, distance)
                .ok_or_else(|| invalid("joined vertex cluster exceeds the join distance"))?;
            let previous = if bound == 0. {
                vertex.tolerance
            } else {
                vertex.tolerance.max(tolerance.absolute())
            };
            retained.tolerance = retained
                .tolerance
                .max(certificate::add_bound(previous, bound)?);
        }
    }
    let mut edges = Vec::new();
    let mut edge_map = vec![0; source.edges.len()];
    for (i, edge) in source.edges.iter().enumerate() {
        if edge_target[i].0 == i {
            edge_map[i] = edges.len();
            let mut edge = edge.clone();
            edge.vertices = edge.vertices.map(|v| vertex_map[v]);
            edge.tolerance = edge_tolerances[i];
            edges.push(edge);
        }
    }
    let mut faces = source.faces.clone();
    for (i, face) in faces.iter_mut().enumerate() {
        face.reversed ^= flips[i];
        for face_loop in &mut face.loops {
            for trim in &mut face_loop.trims {
                trim.vertices = trim.vertices.map(|v| vertex_map[v]);
                if let Some(edge) = trim.edge {
                    let (target, reversed) = edge_target[edge];
                    trim.edge = Some(edge_map[target]);
                    trim.reversed_3d ^= reversed;
                    if let Some(kind) = joined_type[target] {
                        trim.trim_type = kind;
                    }
                }
            }
        }
    }
    Brep::try_new(vertices, edges, faces, tolerance)
}

fn root(roots: &mut [usize], mut i: usize) -> usize {
    while roots[i] != i {
        roots[i] = roots[roots[i]];
        i = roots[i];
    }
    i
}

fn orientation(adjacency: &[Vec<(usize, bool)>]) -> Result<Vec<bool>, GeometryError> {
    let mut senses = vec![None; adjacency.len()];
    let mut pending = Vec::new();
    for seed in 0..senses.len() {
        if senses[seed].is_some() {
            continue;
        }
        senses[seed] = Some(false);
        pending.push(seed);
        while let Some(face) = pending.pop() {
            let flip = senses[face].expect("queued face has a sense");
            for &(next, parity) in &adjacency[face] {
                let expected = flip ^ parity;
                if let Some(actual) = senses[next] {
                    if actual != expected {
                        return Err(invalid(
                            "joined faces have an inconsistent orientation cycle",
                        ));
                    }
                } else {
                    senses[next] = Some(expected);
                    pending.push(next);
                }
            }
        }
    }
    Ok(senses.into_iter().map(Option::unwrap).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_cycles_and_self_seams_are_checked() {
        assert!(
            orientation(&[
                vec![(1, true), (2, true)],
                vec![(0, true), (2, true)],
                vec![(0, true), (1, true)]
            ])
            .is_err()
        );
        assert!(orientation(&[vec![(0, true)]]).is_err());
        assert_eq!(
            orientation(&[vec![(0, false)], vec![], vec![(3, true)], vec![(2, true)]]).unwrap(),
            vec![false, false, false, true]
        );
    }
}

//! A corner sees its two face edges and the next edge across each smooth side.
use super::*;
use std::collections::HashMap;

pub(in super::super) fn continuity_enabled(source: &SubD) -> Vec<bool> {
    let mut sharp = vec![0_f64; source.control_points().len()];
    for (edge, ends) in source.edges().iter().zip(source.edge_sharpness()) {
        for (&v, &s) in edge.iter().zip(ends) {
            sharp[v as usize] = sharp[v as usize].max(s);
        }
    }
    let mut enabled = vec![true; sharp.len()];
    for &[a, b] in source.edges() {
        let (a, b) = (a as usize, b as usize);
        enabled[a] &= sharp[b] <= 3.max(sharp[a].ceil() as u32 + 1) as f64 - 1.;
        enabled[b] &= sharp[a] <= 3.max(sharp[b].ceil() as u32 + 1) as f64 - 1.;
    }
    enabled
}

pub(super) fn corners(source: &SubD) -> Result<Vec<Vec<u32>>, GeometryError> {
    let key = |a: u32, b: u32| (a.min(b), a.max(b));
    let mut lookup = HashMap::with_capacity(source.edges().len());
    for (i, &[a, b]) in source.edges().iter().enumerate() {
        if lookup.insert(key(a, b), i).is_some() {
            return Err(failure("Ambiguous soft SubD control edges"));
        }
    }
    let mut uses = vec![Vec::new(); source.edges().len()];
    let mut face_edges = Vec::with_capacity(source.faces().len());
    for (i, face) in source.faces().iter().enumerate() {
        let mut edges = Vec::with_capacity(face.len());
        for j in 0..face.len() {
            let e = *lookup
                .get(&key(face[j], face[(j + 1) % face.len()]))
                .ok_or_else(|| failure("Missing soft SubD edge"))?;
            uses[e].push((i, j));
            if uses[e].len() > 2 {
                return Err(failure("Nonmanifold soft SubD control edges"));
            }
            edges.push(e);
        }
        face_edges.push(edges);
    }
    let sharpness =
        |e: usize, v: u32| source.edge_sharpness()[e][usize::from(source.edges()[e][0] != v)];
    source
        .faces()
        .iter()
        .enumerate()
        .map(|(i, face)| {
            face.iter()
                .enumerate()
                .map(|(corner, &vertex)| {
                    let mut value = 0_f64;
                    for e in [
                        face_edges[i][corner],
                        face_edges[i][(corner + face.len() - 1) % face.len()],
                    ] {
                        value = value.max(sharpness(e, vertex));
                        if source.edge_tags()[e] == 2 {
                            continue;
                        }
                        for &(other, side) in &uses[e] {
                            if other == i {
                                continue;
                            }
                            let f = &source.faces()[other];
                            let adjacent = if f[side] == vertex {
                                (side + f.len() - 1) % f.len()
                            } else {
                                (side + 1) % f.len()
                            };
                            value = value.max(sharpness(face_edges[other][adjacent], vertex));
                        }
                    }
                    Ok(3.max(value.ceil() as u32 + 1))
                })
                .collect()
        })
        .collect()
}

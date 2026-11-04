//! A corner sees its two face edges and the next edge across each smooth side.
use super::*;
use std::collections::HashMap;

pub(in super::super) fn continuity_enabled(source: &SubD) -> Result<Vec<bool>, GeometryError> {
    let counts = viboceros_opennurbs::subd_sharpness_refinement(source.edge_sharpness())?;
    let mut resolution = vec![0_u32; source.control_points().len()];
    for (edge, ends) in source.edges().iter().zip(&counts) {
        for (&v, &s) in edge.iter().zip(&ends.subdivision_counts) {
            resolution[v as usize] = resolution[v as usize].max(s);
        }
    }
    let mut enabled = vec![true; resolution.len()];
    for &[a, b] in source.edges() {
        let (a, b) = (a as usize, b as usize);
        enabled[a] &= resolution[b] <= resolution[a].max(2);
        enabled[b] &= resolution[a] <= resolution[b].max(2);
    }
    Ok(enabled)
}

pub(super) struct Plan {
    pub corners: Vec<Vec<u32>>,
    face_edges: Vec<Vec<usize>>,
    profiles: Vec<viboceros_opennurbs::SubDSharpnessRefinement>,
}
impl Plan {
    pub fn protected_breaks(
        &self,
        source: &SubD,
        face: usize,
        corner: Option<usize>,
    ) -> [Vec<f64>; 2] {
        let mut out = [Vec::new(), Vec::new()];
        if let Some(corner) = corner {
            let f = &source.faces()[face];
            let v = f[corner];
            for (axis, side) in [corner, (corner + f.len() - 1) % f.len()]
                .into_iter()
                .enumerate()
            {
                let e = self.face_edges[face][side];
                for t in self.profiles[e].required_breaks() {
                    let t = if source.edges()[e][0] == v { t } else { 1. - t };
                    if t < 0.5 {
                        out[axis].push(2. * t);
                    }
                }
            }
        } else {
            for side in 0..4 {
                let e = self.face_edges[face][side];
                for t in self.profiles[e].required_breaks() {
                    let clockwise = if source.edges()[e][0] == source.faces()[face][side] {
                        t
                    } else {
                        1. - t
                    };
                    out[side % 2].push(if side < 2 { clockwise } else { 1. - clockwise });
                }
            }
        }
        out
    }
}

pub(super) fn plan(source: &SubD) -> Result<Plan, GeometryError> {
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
    let profiles = viboceros_opennurbs::subd_sharpness_refinement(source.edge_sharpness())?;
    let depth = |e: usize, v: u32| {
        3.max(profiles[e].subdivision_counts[usize::from(source.edges()[e][0] != v)] + 1)
    };
    let corners = source
        .faces()
        .iter()
        .enumerate()
        .map(|(i, face)| {
            face.iter()
                .enumerate()
                .map(|(corner, &vertex)| {
                    let mut level = 3;
                    for e in [
                        face_edges[i][corner],
                        face_edges[i][(corner + face.len() - 1) % face.len()],
                    ] {
                        level = level.max(depth(e, vertex));
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
                            level = level.max(depth(face_edges[other][adjacent], vertex));
                        }
                    }
                    Ok(level)
                })
                .collect()
        })
        .collect::<Result<_, GeometryError>>()?;
    Ok(Plan {
        corners,
        face_edges,
        profiles,
    })
}

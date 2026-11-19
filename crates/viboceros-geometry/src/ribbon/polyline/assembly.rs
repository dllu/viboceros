//! Source-corner identities survive edge assembly and point-only contacts.
use super::*;
use std::collections::BTreeMap;
use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum VertexKey {
    Source(usize),
    Offset {
        side: usize,
        corner: usize,
        end: usize,
    },
}

pub(super) struct Patch {
    pub brep: Brep,
    /// Natural UV corner order: source start, offset start, offset end, source end.
    pub keys: [Option<VertexKey>; 4],
}

fn invalid() -> GeometryError {
    GeometryError::InvalidBrepTopology {
        context: "ribbon patch provenance was not preserved",
    }
}

fn corner_vertices(face: &crate::BrepFace) -> Result<[Option<usize>; 4], GeometryError> {
    let u = face.surface().domain_u();
    let v = face.surface().domain_v();
    let corners = [
        [*u.start(), *v.start()],
        [*u.end(), *v.start()],
        [*u.end(), *v.end()],
        [*u.start(), *v.end()],
    ];
    let mut vertices = [None; 4];
    for trim in face.loops().iter().flat_map(|loop_| loop_.trims()) {
        let curve = trim.curve();
        for (vertex, t) in trim
            .vertices()
            .into_iter()
            .zip([*curve.domain().start(), *curve.domain().end()])
        {
            let uv = curve.evaluate(t)?.to_array();
            if let Some(corner) = corners.iter().position(|&corner| corner == uv) {
                vertices[corner] = Some(vertex);
            }
        }
    }
    Ok(vertices)
}

pub(super) fn sew(patches: Vec<Patch>, tolerance: Tolerance) -> Result<Vec<Brep>, GeometryError> {
    let components = join_breps(
        &patches.iter().map(|p| &p.brep).collect::<Vec<_>>(),
        tolerance.absolute(),
        tolerance,
    )?;
    let mut first = BTreeMap::new();
    let mut pairs = Vec::new();
    let mut offset = 0;
    let mut parts = Vec::new();
    for component in components {
        // Each input here contains one face. The shared assembler retains
        // input face order within each sorted component, and never refits it.
        if component.source_indices.len() != component.brep.faces().len() {
            return Err(invalid());
        }
        for (face, &source) in component.brep.faces().iter().zip(&component.source_indices) {
            let patch = &patches[source];
            if face.surface() != patch.brep.faces()[0].surface() {
                return Err(invalid());
            }
            for (key, vertex) in patch.keys.into_iter().zip(corner_vertices(face)?) {
                let Some(key) = key else { continue };
                let vertex = offset + vertex.ok_or_else(invalid)?;
                if let Some(&previous) = first.get(&key) {
                    pairs.push((previous, vertex));
                } else {
                    first.insert(key, vertex);
                }
            }
        }
        offset += component.brep.vertices().len();
        parts.push(component.brep);
    }
    Ok(vec![
        Brep::try_combine(parts, tolerance)?.try_identify_vertices(&pairs, tolerance)?,
    ])
}

pub(super) fn sew_untrimmed(
    patches: Vec<Patch>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let (breps, keys): (Vec<_>, Vec<_>) = patches.into_iter().map(|p| (p.brep, p.keys)).unzip();
    let brep = Brep::try_combine(breps, tolerance)?;
    let mut vertex_keys = vec![None; brep.vertices().len()];
    let mut first = BTreeMap::new();
    let mut vertex_pairs = Vec::new();
    for (face, keys) in brep.faces().iter().zip(keys) {
        for (key, vertex) in keys.into_iter().zip(corner_vertices(face)?) {
            let key = key.ok_or_else(invalid)?;
            let vertex = vertex.ok_or_else(invalid)?;
            vertex_keys[vertex] = Some(key);
            if let Some(&prior) = first.get(&key) {
                if vertex != prior {
                    vertex_pairs.push((prior, vertex));
                }
            } else {
                first.insert(key, vertex);
            }
        }
    }

    // Only corner rulings are sewn. Overlapping strips can also have
    // coincident pieces, and the two source rails remain separate edges.
    use VertexKey::{Offset, Source};
    let mut rulings = BTreeMap::new();
    let mut edge_pairs = Vec::new();
    for (index, edge) in brep.edges().iter().enumerate() {
        let ends = edge.vertices().map(|vertex| vertex_keys[vertex]);
        let (keys, reversed) = match ends {
            [Some(a @ Source(_)), Some(b @ Offset { .. })] => ([a, b], false),
            [Some(b @ Offset { .. }), Some(a @ Source(_))] => ([a, b], true),
            _ => continue,
        };
        match rulings.entry(keys) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Some((index, reversed)));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let (prior, prior_reversed) = entry.get_mut().take().ok_or_else(invalid)?;
                edge_pairs.push((prior, index, prior_reversed != reversed));
            }
        }
    }
    brep.try_identify_vertices(&vertex_pairs, tolerance)?
        .try_join_edge_pairs(&edge_pairs, tolerance.absolute(), tolerance)
}

struct Ruling {
    edge: Option<(usize, bool)>,
    vertices: [usize; 2],
}

fn rulings(brep: &Brep, face: &crate::BrepFace) -> Result<[Ruling; 2], GeometryError> {
    let vertices = corner_vertices(face)?;
    let ruling = |a: usize, b: usize| {
        let [Some(source), Some(offset)] = [vertices[a], vertices[b]] else {
            return Err(invalid());
        };
        let edge = face
            .loops()
            .iter()
            .flat_map(|loop_| loop_.trims())
            .filter_map(|trim| trim.edge())
            .find_map(|id| {
                let ends = brep.edges()[id].vertices();
                if ends == [source, offset] {
                    Some((id, false))
                } else if ends == [offset, source] {
                    Some((id, true))
                } else {
                    None
                }
            });
        Ok(Ruling {
            edge,
            vertices: [source, offset],
        })
    };
    Ok([ruling(0, 1)?, ruling(3, 2)?])
}

fn joins(a: &Ruling, b: &Ruling) -> Option<(usize, usize, bool)> {
    let (after, invert_after) = a.edge?;
    let (before, invert_before) = b.edge?;
    Some((after, before, invert_after != invert_before))
}

/// Join consecutive source sections across their common ruling. Several
/// singular faces can overlap after a lobe collapses, so geometric coincidence
/// alone cannot decide which boundaries belong together.
pub(super) fn sew_sections(
    patches: Vec<Patch>,
    source_domain: RangeInclusive<Real>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let brep = Brep::try_combine(
        patches.into_iter().map(|patch| patch.brep).collect(),
        tolerance,
    )?;
    let mut sections = brep
        .faces()
        .iter()
        .map(|face| Ok::<_, GeometryError>((face.surface().domain_v(), rulings(&brep, face)?)))
        .collect::<Result<Vec<_>, _>>()?;
    sections.sort_unstable_by(|a, b| a.0.start().total_cmp(b.0.start()));
    let mut pairs = Vec::new();
    for i in 0..sections.len() {
        let (domain, [_, end]) = &sections[i];
        let j = (i + 1) % sections.len();
        let (next, [start, _]) = &sections[j];
        let adjacent = if j == 0 {
            domain.end() == source_domain.end() && next.start() == source_domain.start()
        } else {
            domain.end() == next.start()
        };
        if adjacent
            && end.vertices.into_iter().zip(start.vertices).all(|(a, b)| {
                brep.vertices()[a]
                    .point()
                    .is_near(brep.vertices()[b].point(), tolerance)
            })
            && let Some(pair) = joins(end, start)
        {
            pairs.push(pair);
        }
    }
    brep.try_join_edge_pairs(&pairs, tolerance.absolute(), tolerance)
}

/// A crossing open offset follows the offset rail walk, which can revisit a
/// source segment. Consecutive sections can share just one vertex of a ruling.
pub(super) fn sew_walk(
    patches: Vec<Patch>,
    close: bool,
    coincident_offsets: Option<[(usize, usize); 2]>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let brep = Brep::try_combine(
        patches.into_iter().map(|patch| patch.brep).collect(),
        tolerance,
    )?;
    let rulings = brep
        .faces()
        .iter()
        .map(|face| rulings(&brep, face))
        .collect::<Result<Vec<_>, _>>()?;
    let mut edges = Vec::new();
    let mut vertices = Vec::new();
    if let Some(ends) = coincident_offsets {
        let [a, b] = ends.map(|(section, end)| rulings[section][end].vertices[1]);
        vertices.push((a, b));
    }
    let count = if close {
        rulings.len()
    } else {
        rulings.len().saturating_sub(1)
    };
    for i in 0..count {
        let end = &rulings[i][1];
        let start = &rulings[(i + 1) % rulings.len()][0];
        let pairs = [0, 1].map(|i| (end.vertices[i], start.vertices[i]));
        let coincident = pairs.map(|(a, b)| {
            let a = brep.vertices()[a].point();
            let b = brep.vertices()[b].point();
            crate::curve_points_coincident(a, b) && a.is_near(b, tolerance)
        });
        if coincident == [true, true]
            && let Some(pair) = joins(end, start)
        {
            edges.push(pair);
        } else {
            vertices.extend(
                pairs
                    .into_iter()
                    .zip(coincident)
                    .filter_map(|(pair, equal)| equal.then_some(pair)),
            );
        }
    }
    brep.try_identify_vertices(&vertices, tolerance)?
        .try_join_edge_pairs(&edges, tolerance.absolute(), tolerance)
}

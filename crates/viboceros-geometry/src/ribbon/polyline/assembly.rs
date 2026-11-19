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
        .map(|face| {
            let vertices = corner_vertices(face)?;
            let domain = face.surface().domain_v();
            let ruling = |a: usize, b: usize| -> Result<_, GeometryError> {
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
                Ok((edge, [source, offset].map(|id| brep.vertices()[id].point())))
            };
            Ok::<_, GeometryError>((domain, [ruling(0, 1)?, ruling(3, 2)?]))
        })
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
            && end
                .1
                .into_iter()
                .zip(start.1)
                .all(|(a, b)| a.is_near(b, tolerance))
            && let (Some((after, invert_after)), Some((before, invert_before))) = (end.0, start.0)
        {
            pairs.push((after, before, invert_after != invert_before));
        }
    }
    brep.try_join_edge_pairs(&pairs, tolerance.absolute(), tolerance)
}

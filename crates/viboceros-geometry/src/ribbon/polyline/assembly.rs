//! Source-corner identities survive edge assembly and point-only contacts.
use super::*;
use std::collections::BTreeMap;

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

pub(super) fn sew(patches: Vec<Patch>, tolerance: Tolerance) -> Result<Vec<Brep>, GeometryError> {
    let invalid = || GeometryError::InvalidBrepTopology {
        context: "ribbon patch provenance was not preserved",
    };
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
            let u = face.surface().domain_u();
            let v = face.surface().domain_v();
            let uv = [
                [*u.start(), *v.start()],
                [*u.end(), *v.start()],
                [*u.end(), *v.end()],
                [*u.start(), *v.end()],
            ];
            for (key, uv) in patch.keys.into_iter().zip(uv) {
                let Some(key) = key else { continue };
                let mut vertex = None;
                for trim in face.loops().iter().flat_map(|l| l.trims()) {
                    let c = trim.curve();
                    for (i, t) in [*c.domain().start(), *c.domain().end()]
                        .into_iter()
                        .enumerate()
                    {
                        if c.evaluate(t)?.to_array() == uv {
                            vertex = Some(offset + trim.vertices()[i]);
                        }
                    }
                }
                let vertex = vertex.ok_or_else(invalid)?;
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

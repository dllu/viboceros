use super::*;

pub(super) fn collect(
    joined: Brep,
    before: &Brep,
    pairs: &[(usize, usize, bool)],
    sources: &[usize],
    tolerance: Tolerance,
) -> Result<Vec<BrepJoinComponent>, GeometryError> {
    let components = joined.edge_connected_face_components();
    let mut membership = vec![usize::MAX; joined.faces.len()];
    for (component, faces) in components.iter().enumerate() {
        for &face in faces {
            membership[face] = component;
        }
    }
    let mut counts = vec![0; components.len()];
    let mut edge_face = vec![0; before.edges.len()];
    for usage in before.trim_uses() {
        if let Some(e) = usage.trim.edge {
            edge_face[e] = usage.face;
        }
    }
    for &(a, _, _) in pairs {
        counts[membership[edge_face[a]]] += 1;
    }
    let mut outputs = Vec::with_capacity(components.len());
    let extracted = joined.copy_face_components(&components, tolerance)?;
    for ((i, faces), mut brep) in components.into_iter().enumerate().zip(extracted) {
        let source_indices = faces
            .iter()
            .map(|&f| sources[f])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if counts[i] > 0 {
            // Orientation does not require a representable mass integral:
            // finite, regular shells can have underflowing/overflowing volume.
            // Every extracted result here is one edge-connected component.
            // Unsupported spatial witnesses retain the numerical fallback;
            // it is neither a certificate nor a solid-validity test.
            let inward = match brep.solid_orientation()? {
                BrepSolidOrientation::Inward => true,
                BrepSolidOrientation::Unknown => brep.signed_volume(tolerance)? < 0.,
                BrepSolidOrientation::Outward | BrepSolidOrientation::NotSolid => false,
            };
            if inward {
                brep = brep.reversed();
            }
        }
        outputs.push(BrepJoinComponent {
            brep,
            source_indices,
            joined_edge_count: counts[i],
        });
    }
    Ok(outputs)
}

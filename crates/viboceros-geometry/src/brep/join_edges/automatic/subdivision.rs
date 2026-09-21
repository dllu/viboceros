//! Source-local edge order and assembly-wide original-vertex precedence.
use super::*;

pub(super) fn apply(
    sources: &[&Brep],
    cuts: &[(usize, Vec<Real>)],
    tolerance: Tolerance,
) -> Result<(Brep, Vec<bool>), GeometryError> {
    let mut edge_offset = 0;
    let mut vertex_offset = 0;
    let mut cut_offset = 0;
    let mut pieces = Vec::with_capacity(sources.len());
    let mut split_edges = Vec::new();
    let mut original_vertices = Vec::new();
    let mut inserted_vertices = Vec::new();
    for source in sources {
        let next_offset = edge_offset + source.edges.len();
        let count = cuts[cut_offset..].partition_point(|(edge, _)| *edge < next_offset);
        let local = cuts[cut_offset..cut_offset + count]
            .iter()
            .map(|(edge, parameters)| (edge - edge_offset, parameters.clone()))
            .collect::<Vec<_>>();
        let piece = if local.is_empty() {
            (*source).clone()
        } else {
            source.try_split_edges_at_parameters(&local, tolerance)?
        };
        let mut flags = vec![false; piece.edges.len()];
        for (edge, _) in local {
            flags[edge] = true;
        }
        flags[source.edges.len()..].fill(true);
        split_edges.extend(flags);
        original_vertices.extend(vertex_offset..vertex_offset + source.vertices.len());
        inserted_vertices
            .extend(vertex_offset + source.vertices.len()..vertex_offset + piece.vertices.len());
        vertex_offset += piece.vertices.len();
        pieces.push(piece);
        edge_offset = next_offset;
        cut_offset += count;
    }
    let mut combined = Brep::try_combine(pieces, tolerance)?;
    original_vertices.extend(inserted_vertices);
    let mut vertex_map = vec![0; combined.vertices.len()];
    let vertices = original_vertices
        .into_iter()
        .enumerate()
        .map(|(new, old)| {
            vertex_map[old] = new;
            combined.vertices[old]
        })
        .collect();
    // This permutation changes no geometry or incidence. Keeping every
    // original vertex before inserted ones makes endpoint unions retain a
    // genuine source endpoint instead of a rounded subdivision evaluation.
    combined.vertices = vertices;
    for edge in &mut combined.edges {
        edge.vertices = edge.vertices.map(|v| vertex_map[v]);
    }
    for trim in combined
        .faces
        .iter_mut()
        .flat_map(|f| &mut f.loops)
        .flat_map(|l| &mut l.trims)
    {
        trim.vertices = trim.vertices.map(|v| vertex_map[v]);
    }
    Ok((combined, split_edges))
}

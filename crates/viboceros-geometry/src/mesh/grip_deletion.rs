//! Native face-set dissolution preserves stored facets and n-gon overlays.
use super::*;

impl TriangleMesh {
    /// Dissolve the face sets incident to these ordinary vertex indices using
    /// OpenNURBS. The result retains native facet order, n-gons and vertex colors.
    /// None represents an empty mesh; the source remains unchanged on failure.
    pub fn dissolved_vertices(&self, vertices: &[usize]) -> Result<Option<Self>, GeometryError> {
        if let Some(&index) = vertices.iter().find(|&&i| i >= self.vertices.len()) {
            return Err(GeometryError::InvalidControlPointIndex {
                index,
                count: self.vertices.len(),
            });
        }
        if vertices.is_empty() {
            return Ok(Some(self.clone()));
        }
        // Regenerating mesh grips welds exact duplicates in source order. Reuse
        // the common parent compactor so face/color/n-gon mappings stay together.
        let mut first = std::collections::BTreeMap::new();
        let parents = self
            .vertices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                *first
                    .entry(p.to_array().map(canonical_coordinate_bits))
                    .or_insert(i)
            })
            .collect();
        let (welded, _) = self.compacted_with_vertex_parents(parents);
        let index_by_location = welded
            .vertices
            .iter()
            .enumerate()
            .map(|(i, p)| (p.to_array().map(canonical_coordinate_bits), i))
            .collect::<std::collections::BTreeMap<_, _>>();
        let picked = vertices
            .iter()
            .filter_map(|&i| {
                index_by_location
                    .get(&self.vertices[i].to_array().map(canonical_coordinate_bits))
                    .copied()
            })
            .collect::<Vec<_>>();
        let source = welded.open_nurbs_data();
        let result = viboceros_opennurbs::dissolve_mesh_vertices(&source, &picked)?;
        if result == source {
            Ok(Some(welded))
        } else if result.faces.is_empty() {
            Ok(None)
        } else {
            Self::from_open_nurbs_data(result).map(Some)
        }
    }
}

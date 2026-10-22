//! Whole-mesh seam welding and incremental patch welding.
use super::*;

impl TriangleMesh {
    /// Welds coincident edge endpoints whose incident face normals fall
    /// within the supplied angular tolerance.
    ///
    /// Only vertices paired along an exact-location topology edge can merge;
    /// a coincident vertex-only contact remains distinct. Unions retain the
    /// later raw representative and compact unused vertices in source order.
    /// Non-manifold edges consider their first two face uses. Stored UVs,
    /// colors and normal directions are blended; opposing stored normals leave
    /// the input unchanged. Native connected-face traversal remains incomplete.
    pub fn welded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        self.welded_vertices_after(angle_tolerance_radians, 0)
    }

    /// Weld only seams incident to faces at or after `first_new_face`.
    /// Existing unrelated unwelded edges retain their raw vertex sharing.
    pub fn welded_vertices_after(
        &self,
        angle_tolerance_radians: Real,
        first_new_face: usize,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshWeldAngle);
        }
        let data = self.topology_data();
        let mut face_normals = vec![None; self.faces.len()];
        let mut normal = |face: usize| -> Result<UnitVector3, GeometryError> {
            if let Some(value) = face_normals[face] {
                return Ok(value);
            }
            let value = self.polygon_face_normal(face)?;
            face_normals[face] = Some(value);
            Ok(value)
        };
        let minimum_dot = angle_tolerance_radians.cos();
        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        let mut channels = self.vertex_channels.clone();
        let mut colors = self.vertex_colors.clone();
        for incidence in data.edges.values() {
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            let Some(second) = uses.next() else {
                continue;
            };
            if first.face < first_new_face && second.face < first_new_face {
                continue;
            }
            let dot = normal(first.face)?
                .as_vector()
                .dot(normal(second.face)?.as_vector())?
                .clamp(-1., 1.);
            if dot < minimum_dot {
                continue;
            }
            for endpoint in 0..2 {
                let a = index_root(&mut parents, first.raw_vertices[endpoint] as usize);
                let b = index_root(&mut parents, second.raw_vertices[endpoint] as usize);
                if a == b {
                    continue;
                }
                if !blend_channels(&mut channels, a, b)? {
                    // An opposing stored-normal pair has no average direction.
                    return Ok((self.clone(), 0));
                }
                if let Some(colors) = &mut colors {
                    let blended = std::array::from_fn(|i| {
                        // Match public ON_Color fractional storage and rounding.
                        let factor = 0.003_921_568_627_450_98;
                        (((colors[a][i] as Real * factor + colors[b][i] as Real * factor) * 0.5)
                            * 255.)
                            .round() as u8
                    });
                    colors[a] = blended;
                    colors[b] = blended;
                }
                union_indices_keep_later(&mut parents, a, b);
            }
        }

        Ok(self.compacted_with_vertex_parents_and_channels(parents, &channels, colors.as_deref()))
    }
}

/// Stored seam values are blended before either source representative disappears.
fn blend_channels(
    channels: &mut VertexChannels,
    a: usize,
    b: usize,
) -> Result<bool, GeometryError> {
    if let Some(normals) = &mut channels.normals {
        let x = normals[a].to_array();
        let y = normals[b].to_array();
        let sum = Vector3::try_from(std::array::from_fn::<_, 3, _>(|i| x[i] + y[i]))?;
        let Ok(normal) = sum.normalized_nonzero() else {
            return Ok(false);
        };
        let normal = Vector3::try_from(normal.as_vector().to_array().map(|n| n as f32 as Real))?;
        normals[a] = normal;
        normals[b] = normal;
    }
    if let Some(uv) = &mut channels.uv {
        let blended = std::array::from_fn(|i| ((uv[a][i] + uv[b][i]) * 0.5) as f32 as Real);
        uv[a] = blended;
        uv[b] = blended;
    }
    Ok(true)
}

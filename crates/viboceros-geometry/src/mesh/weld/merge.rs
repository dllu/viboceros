//! Directed seam joins retain previously established representatives.
use super::*;

pub(super) struct WeldState {
    parents: Vec<usize>,
    members: Vec<usize>,
    channels: VertexChannels,
    colors: Option<Vec<[u8; 4]>>,
}
impl WeldState {
    pub fn new(source: &TriangleMesh) -> Self {
        Self {
            parents: (0..source.vertices.len()).collect(),
            members: vec![1; source.vertices.len()],
            channels: source.vertex_channels.clone(),
            colors: source.vertex_colors.clone(),
        }
    }
    /// Return false when stored normals have no average direction. The caller
    /// then discards the entire staged operation, including preceding joins.
    pub fn merge_edge(
        &mut self,
        uses: [EdgeUse; 2],
        descending: [bool; 2],
    ) -> Result<bool, GeometryError> {
        let roots = uses.map(|u| {
            u.raw_vertices
                .map(|v| index_root(&mut self.parents, v as usize))
        });
        let direction = match descending {
            [true, false] => Some(0),
            [false, true] => Some(1),
            _ => None,
        };
        // An endpoint joined through another seam can identify the established
        // side of this edge. Originally shared raw endpoints need no such join.
        let established = (0..2).find_map(|end| {
            (roots[0][end] == roots[1][end]
                && uses[0].raw_vertices[end] != uses[1].raw_vertices[end])
                .then(|| {
                    usize::from(
                        roots[1][end] == uses[1].raw_vertices[end] as usize
                            && roots[0][end] != uses[0].raw_vertices[end] as usize,
                    )
                })
        });
        for (end, (&a, &b)) in roots[0].iter().zip(&roots[1]).enumerate() {
            if a == b {
                continue;
            }
            let preferred = direction.unwrap_or_else(|| {
                let first = uses[0].raw_vertices[end];
                let second = uses[1].raw_vertices[end];
                usize::from(if descending[0] {
                    first > second
                } else {
                    first < second
                })
            });
            let mut side = established.unwrap_or(preferred);
            if self.members[a] > 1 && self.members[b] == 1 {
                side = 0;
            } else if self.members[b] > 1 && self.members[a] == 1 {
                side = 1;
            }
            let average =
                direction.is_none() || established.is_none() || established == Some(preferred);
            if average && !self.blend(a, b)? {
                return Ok(false);
            }
            let keep = [a, b][side];
            let remove = [b, a][side];
            self.parents[remove] = keep;
            self.members[keep] += self.members[remove];
        }
        Ok(true)
    }
    pub fn finish(self, source: &TriangleMesh) -> (TriangleMesh, usize) {
        source.compacted_with_vertex_parents_and_channels(
            self.parents,
            &self.channels,
            self.colors.as_deref(),
        )
    }
    fn blend(&mut self, a: usize, b: usize) -> Result<bool, GeometryError> {
        if let Some(normals) = &mut self.channels.normals {
            let x = normals[a].to_array();
            let y = normals[b].to_array();
            let sum = Vector3::try_from(std::array::from_fn::<_, 3, _>(|i| x[i] + y[i]))?;
            let Ok(normal) = sum.normalized_nonzero() else {
                return Ok(false);
            };
            let normal =
                Vector3::try_from(normal.as_vector().to_array().map(|n| n as f32 as Real))?;
            normals[a] = normal;
            normals[b] = normal;
        }
        if let Some(uv) = &mut self.channels.uv {
            let blended = std::array::from_fn(|i| ((uv[a][i] + uv[b][i]) * 0.5) as f32 as Real);
            uv[a] = blended;
            uv[b] = blended;
        }
        if let Some(colors) = &mut self.colors {
            let blended = std::array::from_fn(|i| {
                // Public ON_Color fractional storage and nearest-byte rounding.
                let factor = 0.003_921_568_627_450_98;
                (((colors[a][i] as Real * factor + colors[b][i] as Real * factor) * 0.5) * 255.)
                    .round() as u8
            });
            colors[a] = blended;
            colors[b] = blended;
        }
        Ok(true)
    }
}

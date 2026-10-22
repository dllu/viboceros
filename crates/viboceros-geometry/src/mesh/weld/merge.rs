//! Directed seam joins retain previously established representatives.
use super::*;

pub(super) struct WeldState {
    parents: Vec<usize>,
    members: Vec<usize>,
    attributes: super::attributes::WeldAttributes,
}
impl WeldState {
    pub fn new(source: &TriangleMesh) -> Self {
        Self {
            parents: (0..source.vertices.len()).collect(),
            members: vec![1; source.vertices.len()],
            attributes: super::attributes::WeldAttributes::new(source, source.vertex_normals()),
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
            if average && !self.attributes.blend(a, b, false)? {
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
            &self.attributes.channels,
            self.attributes.colors.as_deref(),
        )
    }
}

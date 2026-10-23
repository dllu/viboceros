use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexSeamMode {
    Weld,
    Unweld { modify_normals: bool },
}
impl VertexSeamMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::Weld => "WeldVertices",
            Self::Unweld { .. } => "UnweldVertex",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeSeamMode {
    Weld,
    Unweld { modify_normals: bool },
}
impl EdgeSeamMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::Weld => "WeldEdge",
            Self::Unweld { .. } => "UnweldEdge",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeamMode {
    Vertex(VertexSeamMode),
    Edge(EdgeSeamMode),
}
impl From<VertexSeamMode> for SeamMode {
    fn from(mode: VertexSeamMode) -> Self {
        Self::Vertex(mode)
    }
}
impl From<EdgeSeamMode> for SeamMode {
    fn from(mode: EdgeSeamMode) -> Self {
        Self::Edge(mode)
    }
}
impl SeamMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::Vertex(m) => m.command(),
            Self::Edge(m) => m.command(),
        }
    }
    pub(super) fn is_edge(self) -> bool {
        matches!(self, Self::Edge(_))
    }
    pub(super) fn weld(self) -> bool {
        matches!(
            self,
            Self::Vertex(VertexSeamMode::Weld) | Self::Edge(EdgeSeamMode::Weld)
        )
    }
    pub(super) fn clear_vertices(self) -> bool {
        self == Self::Vertex(VertexSeamMode::Weld)
    }
    pub(super) fn stale(self) -> CommandError {
        if self.is_edge() {
            CommandError::EdgeSeamStale
        } else {
            CommandError::VertexSeamStale
        }
    }
    pub(super) fn resource(self) -> CommandError {
        if self.is_edge() {
            CommandError::EdgeSeamResourceLimit
        } else {
            CommandError::VertexSeamResourceLimit
        }
    }
    pub(super) fn unsupported(self) -> CommandError {
        match self {
            Self::Vertex(VertexSeamMode::Weld) => CommandError::UnsupportedWeldVerticesGeometry,
            Self::Vertex(_) => CommandError::UnsupportedUnweldVertexGeometry,
            Self::Edge(EdgeSeamMode::Weld) => CommandError::UnsupportedWeldEdgeGeometry,
            Self::Edge(_) => CommandError::UnsupportedUnweldEdgeGeometry,
        }
    }
    pub(super) fn empty(self) -> CommandError {
        match self {
            Self::Vertex(VertexSeamMode::Weld) => CommandError::NoSelectedMeshVerticesToWeld,
            Self::Vertex(_) => CommandError::NoSelectedMeshVerticesToUnweld,
            Self::Edge(EdgeSeamMode::Weld) => CommandError::NoSelectedMeshEdgesToWeld,
            Self::Edge(_) => CommandError::NoSelectedMeshEdgesToUnweld,
        }
    }
    pub(super) fn index_error(self, index: usize, count: usize) -> CommandError {
        match self {
            Self::Vertex(VertexSeamMode::Weld) => CommandError::WeldVertexIndexOutOfRange {
                vertex: index,
                vertex_count: count,
            },
            Self::Vertex(_) => CommandError::UnweldVertexIndexOutOfRange {
                vertex: index,
                vertex_count: count,
            },
            Self::Edge(EdgeSeamMode::Weld) => CommandError::WeldEdgeIndexOutOfRange {
                edge: index,
                edge_count: count,
            },
            Self::Edge(_) => CommandError::UnweldEdgeIndexOutOfRange {
                edge: index,
                edge_count: count,
            },
        }
    }
    pub(super) fn mesh_edit(
        self,
        mesh: &TriangleMesh,
        indices: &[usize],
    ) -> Result<(TriangleMesh, usize), GeometryError> {
        match self {
            Self::Vertex(VertexSeamMode::Weld) => {
                mesh.welded_topology_vertices_with_normal_generation(indices, true)
            }
            Self::Vertex(VertexSeamMode::Unweld { modify_normals }) => {
                mesh.unwelded_topology_vertices_with_normal_update(indices, modify_normals)
            }
            Self::Edge(EdgeSeamMode::Weld) => {
                mesh.welded_topology_edges_with_normal_generation(indices, true)
            }
            Self::Edge(EdgeSeamMode::Unweld { modify_normals }) => {
                mesh.unwelded_topology_edges_with_normal_update(indices, modify_normals)
            }
        }
    }
    pub(super) fn mesh_message(
        self,
        selected: usize,
        changed: usize,
        operations: usize,
        removed: usize,
        unchanged: usize,
    ) -> String {
        match self {
            Self::Vertex(VertexSeamMode::Weld) => format!(
                "Processed {selected} selected mesh topology vertex index(es) in {changed} changed mesh(es): welded {operations} incident seam edge(s) and removed {removed} raw vertex occurrence(s); {unchanged} mesh(es) unchanged"
            ),
            Self::Vertex(_) => format!(
                "Processed {selected} selected mesh topology vertex index(es) in {changed} changed mesh(es): {operations} previously shared vertex set(s) separated; {unchanged} mesh(es) unchanged"
            ),
            Self::Edge(EdgeSeamMode::Weld) => format!(
                "Processed {selected} selected mesh edge(s) in {changed} changed mesh(es): welded {operations} divided edge(s) and removed {removed} raw vertex occurrence(s); {unchanged} mesh(es) unchanged"
            ),
            Self::Edge(_) => format!(
                "Processed {selected} selected mesh edge(s) in {changed} changed mesh(es): {operations} new seam(s); {unchanged} mesh(es) unchanged"
            ),
        }
    }
    pub(super) fn usage(self) -> &'static str {
        match self {
            Self::Vertex(VertexSeamMode::Weld) => {
                "WeldVertices (point|Vertices=All|Vertices=0,2,...)"
            }
            Self::Vertex(_) => {
                "UnweldVertex (point|Vertices=All|Vertices=0,2,...) [ModifyNormals=Yes|No]"
            }
            Self::Edge(EdgeSeamMode::Weld) => "WeldEdge (point|Edges=All|Edges=0,2,...)",
            Self::Edge(_) => "UnweldEdge (point|Edges=All|Edges=0,2,...) [ModifyNormals=Yes|No]",
        }
    }
    pub(super) fn with_normals(self, modify_normals: bool) -> Self {
        match self {
            Self::Vertex(_) => Self::Vertex(VertexSeamMode::Unweld { modify_normals }),
            Self::Edge(_) => Self::Edge(EdgeSeamMode::Unweld { modify_normals }),
        }
    }
}

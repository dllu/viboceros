//! Seam command registrations share component staging and parsing.
use super::*;
pub(super) struct UnweldEdgeCommand;
impl Command for UnweldEdgeCommand {
    fn name(&self) -> &'static str {
        "UnweldEdge"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["UnweldMeshEdge"]
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        component_seams::run(
            document,
            component_seams::EdgeSeamMode::Unweld {
                modify_normals: true,
            }
            .into(),
            arguments,
        )
    }
}
pub(super) struct UnweldVertexCommand;
impl Command for UnweldVertexCommand {
    fn name(&self) -> &'static str {
        "UnweldVertex"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["UnweldVertices", "UnweldMeshVertex"]
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        vertex_seams::run(
            document,
            vertex_seams::VertexSeamMode::Unweld {
                modify_normals: true,
            },
            arguments,
        )
    }
}

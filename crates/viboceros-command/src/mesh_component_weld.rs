//! Seam command registrations share component staging and parsing.
use super::*;
pub(super) struct WeldEdgeCommand;
impl Command for WeldEdgeCommand {
    fn name(&self) -> &'static str {
        "WeldEdge"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["WeldMeshEdge"]
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        component_seams::run(
            document,
            component_seams::EdgeSeamMode::Weld.into(),
            arguments,
        )
    }
}
pub(super) struct WeldVerticesCommand;
impl Command for WeldVerticesCommand {
    fn name(&self) -> &'static str {
        "WeldVertices"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["WeldVertex", "WeldMeshVertex"]
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        vertex_seams::run(document, vertex_seams::VertexSeamMode::Weld, arguments)
    }
}

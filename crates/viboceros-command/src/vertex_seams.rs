//! Compatibility names for staged vertex seams, shared with edge command admission.
pub use crate::component_seams::{
    ComponentSeamEdit as VertexSeamEdit, SeamOutcome as VertexSeamOutcome,
    SeamTarget as VertexSeamTarget, VertexSeamMode,
};
pub(super) fn run(
    document: &mut crate::Document,
    mode: VertexSeamMode,
    args: &[&str],
) -> Result<String, crate::CommandError> {
    crate::component_seams::run(document, mode.into(), args)
}

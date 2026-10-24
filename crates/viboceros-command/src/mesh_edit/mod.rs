//! Mesh-edit command adapters. Geometry algorithms remain in the kernel.

mod edge_curves;
mod edges;
mod extraction;
mod faces;
mod holes;
mod rebuild_mesh;
mod rebuild_normals;
mod replacement;
mod triangulate;
mod unify_normals;

pub(super) use edge_curves::ExtractMeshEdgesCommand;
pub(super) use edges::{CollapseMeshEdgeCommand, SplitMeshEdgeCommand, SwapMeshEdgeCommand};
pub(super) use extraction::{ExtractDuplicateMeshFacesCommand, ExtractNonManifoldMeshEdgesCommand};
pub(super) use faces::{DeleteFacesCommand, ExtractMeshFacesCommand};
pub(super) use holes::{FillMeshHoleCommand, FillMeshHolesCommand};
pub use rebuild_mesh::MeshRebuildOptions;
pub(super) use rebuild_mesh::RebuildMeshCommand;
pub(super) use rebuild_normals::RebuildMeshNormalsCommand;
pub(super) use triangulate::TriangulateMeshCommand;
pub(super) use unify_normals::UnifyMeshNormalsCommand;

#[cfg(test)]
use extraction::stage_selected_mesh_face_extractions;
#[cfg(test)]
use faces::selected_mesh_face_sources;
#[cfg(test)]
mod selection_tests;

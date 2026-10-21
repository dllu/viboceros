//! Mesh-edit command adapters. Geometry algorithms remain in the kernel.

mod edges;
mod extraction;
mod faces;
mod holes;
mod triangulate;

pub(super) use edges::{CollapseMeshEdgeCommand, SplitMeshEdgeCommand, SwapMeshEdgeCommand};
pub(super) use extraction::{ExtractDuplicateMeshFacesCommand, ExtractNonManifoldMeshEdgesCommand};
pub(super) use faces::{DeleteFacesCommand, ExtractMeshFacesCommand};
pub(super) use holes::{FillMeshHoleCommand, FillMeshHolesCommand};
pub(super) use triangulate::TriangulateMeshCommand;

#[cfg(test)]
use extraction::stage_selected_mesh_face_extractions;
#[cfg(test)]
use faces::selected_mesh_face_sources;
#[cfg(test)]
mod selection_tests;

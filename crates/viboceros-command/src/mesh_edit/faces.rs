//! Mesh face commands share selectors while retaining separate edit plans.
mod delete;
mod extract;
mod selection;

pub(crate) use delete::DeleteFacesCommand;
pub(crate) use extract::ExtractMeshFacesCommand;
#[cfg(test)]
pub(super) use selection::selected_mesh_face_sources;

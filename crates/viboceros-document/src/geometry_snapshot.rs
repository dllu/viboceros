//! Shared immutable geometry for document states and derived-data caches.
use std::{ops::Deref, sync::Arc};

use crate::{EdgeBlendRecipe, Geometry};
use viboceros_geometry::TriangleMesh;

/// An owned, immutable geometry value with inexpensive cloning.
///
/// Document edits install a new snapshot; clones and history share existing
/// snapshots. There is deliberately no mutable access, even for unique owners.
/// Kernel-internal memoization does not change the geometric value.
#[derive(Clone, Debug)]
pub struct GeometrySnapshot(
    Arc<Geometry>,
    Option<Arc<(TriangleMesh, Vec<usize>)>>,
    Option<Arc<EdgeBlendRecipe>>,
);

impl GeometrySnapshot {
    pub(crate) fn with_archive_render_mesh(
        geometry: Geometry,
        render_mesh: Option<(TriangleMesh, Vec<usize>)>,
    ) -> Self {
        Self(Arc::new(geometry), render_mesh.map(Arc::new), None)
    }

    pub(crate) fn with_edge_blend_recipe(mut self, recipe: Arc<EdgeBlendRecipe>) -> Self {
        self.2 = Some(recipe);
        self
    }

    pub fn edge_blend_recipe(&self) -> Option<&EdgeBlendRecipe> {
        self.2.as_deref()
    }

    /// Saved display polygons and the source B-rep face for each polygon.
    pub fn archive_render_mesh(&self) -> Option<(&TriangleMesh, &[usize])> {
        self.1
            .as_deref()
            .map(|(mesh, faces)| (mesh, faces.as_slice()))
    }

    /// Constant-time identity check for derived-data invalidation. False does
    /// not imply unequal geometry: independently created equal values have
    /// distinct storage. Retaining the snapshot prevents address reuse, unlike
    /// caching a raw address or an object's ID. This is not a persistent ID.
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl From<Geometry> for GeometrySnapshot {
    fn from(geometry: Geometry) -> Self {
        Self(Arc::new(geometry), None, None)
    }
}

impl Deref for GeometrySnapshot {
    type Target = Geometry;

    fn deref(&self) -> &Geometry {
        &self.0
    }
}

impl PartialEq for GeometrySnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.shares_storage_with(other) || self.0 == other.0
    }
}

#[cfg(test)]
mod tests;

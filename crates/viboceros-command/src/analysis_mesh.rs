//! Shared analysis tessellation and channels for display and extraction.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::{GeometrySnapshot, Object};
use viboceros_geometry::{CurvatureMesh, DraftAngleMapping, GeometryError};
mod copy;
mod options;
#[cfg(test)]
mod tests;
pub use copy::copy_mesh;
pub(crate) use options::MeshOptions;

struct Entry {
    source: GeometrySnapshot,
    tolerance: Tolerance,
    density: viboceros_geometry::SubDMeshDensity,
    mesh: Result<Option<Arc<CurvatureMesh>>, GeometryError>,
}
#[derive(Default)]
pub(crate) struct Cache(Mutex<BTreeMap<ObjectId, Entry>>);
impl Cache {
    pub(crate) fn get(
        &self,
        object: &Object,
        doc: &Document,
    ) -> Result<Option<Arc<CurvatureMesh>>, GeometryError> {
        let tolerance = doc.tolerance();
        // Only SubD geometry depends on this setting; changing its level must
        // not discard expensive NURBS/B-rep curvature samples.
        let density = if matches!(object.geometry(), Geometry::SubD(_)) {
            doc.analysis_subd_density()
        } else {
            Default::default()
        };
        let mut cache = self.0.lock().unwrap();
        let entry = cache.entry(object.id()).or_insert_with(|| Entry {
            source: object.geometry_snapshot().clone(),
            tolerance,
            density,
            mesh: generate(object.geometry(), tolerance, density),
        });
        if entry.tolerance != tolerance
            || entry.density != density
            || !entry.source.shares_storage_with(object.geometry_snapshot())
        {
            *entry = Entry {
                source: object.geometry_snapshot().clone(),
                tolerance,
                density,
                mesh: generate(object.geometry(), tolerance, density),
            };
        }
        entry.mesh.clone()
    }
    pub(crate) fn retain(&self, doc: &Document) {
        self.0
            .lock()
            .unwrap()
            .retain(|id, _| doc.object(*id).is_some());
    }
}
pub(crate) fn generate(
    geometry: &Geometry,
    tolerance: Tolerance,
    density: viboceros_geometry::SubDMeshDensity,
) -> Result<Option<Arc<CurvatureMesh>>, GeometryError> {
    Ok(match geometry {
        Geometry::NurbsSurface(s) => Some(s.curvature_mesh(8, tolerance)?),
        Geometry::Brep(b) => Some(b.curvature_mesh(8, tolerance)?),
        Geometry::SubD(s) => Some(s.curvature_mesh_with_density(density)?),
        _ => None,
    }
    .map(Arc::new))
}

pub fn normals(mesh: &CurvatureMesh) -> impl Iterator<Item = [Vector3; 3]> + '_ {
    mesh.normals.iter().enumerate().map(|(i, normals)| {
        let fallback = || {
            mesh.mesh
                .face_normal(i)
                .map(|n| n.as_vector())
                .unwrap_or_else(|_| Vector3::try_from([0., 0., 1.]).unwrap())
        };
        normals.map(|n| n.map_or_else(fallback, |n| n.as_vector()))
    })
}
pub fn draft_colors(mesh: &CurvatureMesh, mapping: DraftAngleMapping) -> Vec<[[u8; 3]; 3]> {
    normals(mesh).map(|n| n.map(|n| mapping.color(n))).collect()
}

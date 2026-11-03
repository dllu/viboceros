//! Shared analysis tessellation and channels for display and extraction.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::{GeometrySnapshot, Object};
use viboceros_geometry::AnalysisMeshSettings;
use viboceros_geometry::{CurvatureMesh, DraftAngleMapping, GeometryError};
mod copy;
#[cfg(test)]
mod nurbs_tests;
mod options;
#[cfg(test)]
mod tests;
pub use copy::copy_mesh;
pub(crate) use options::MeshOptions;

struct Entry {
    source: GeometrySnapshot,
    tolerance: Tolerance,
    settings: AnalysisMeshSettings,
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
        let settings = settings_for_geometry(object.geometry(), doc.analysis_mesh_settings());
        let mut cache = self.0.lock().unwrap();
        let entry = cache.entry(object.id()).or_insert_with(|| Entry {
            source: object.geometry_snapshot().clone(),
            tolerance,
            settings,
            mesh: generate(object.geometry(), tolerance, settings),
        });
        if entry.tolerance != tolerance
            || entry.settings != settings
            || !entry.source.shares_storage_with(object.geometry_snapshot())
        {
            *entry = Entry {
                source: object.geometry_snapshot().clone(),
                tolerance,
                settings,
                mesh: generate(object.geometry(), tolerance, settings),
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
    settings: AnalysisMeshSettings,
) -> Result<Option<Arc<CurvatureMesh>>, GeometryError> {
    Ok(match geometry {
        Geometry::NurbsSurface(s) => {
            Some(s.curvature_mesh_with_density(settings.nurbs_density, tolerance)?)
        }
        Geometry::Brep(b) => {
            Some(b.curvature_mesh_with_density(settings.nurbs_density, tolerance)?)
        }
        Geometry::SubD(s) => Some(s.curvature_mesh_with_density(settings.subd_density)?),
        _ => None,
    }
    .map(Arc::new))
}

/// Cache only settings which affect this geometry family.
pub fn settings_for_geometry(
    geometry: &Geometry,
    settings: AnalysisMeshSettings,
) -> AnalysisMeshSettings {
    match geometry {
        Geometry::NurbsSurface(_) | Geometry::Brep(_) => AnalysisMeshSettings {
            nurbs_density: settings.nurbs_density,
            ..Default::default()
        },
        Geometry::SubD(_) => AnalysisMeshSettings {
            subd_density: settings.subd_density,
            ..Default::default()
        },
        _ => Default::default(),
    }
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

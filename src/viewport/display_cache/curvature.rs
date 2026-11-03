//! Differential colors are independent of cameras and selection highlights.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{AffineTransform3, CurvatureColorMapping, CurvatureMesh};

pub(super) struct CachedMesh {
    settings: viboceros_geometry::AnalysisMeshSettings,
    mesh: Option<Arc<CurvatureMesh>>,
}

pub(super) struct Drawing {
    source: Option<Arc<CurvatureMesh>>,
    mapping: CurvatureColorMapping,
    transform: Option<AffineTransform3>,
    colors: DraftColors,
}
impl DisplayGeometry {
    pub(in crate::viewport) fn curvature_mesh_with_settings(
        &self,
        settings: viboceros_geometry::AnalysisMeshSettings,
    ) -> Option<Arc<CurvatureMesh>> {
        let settings =
            viboceros_command::analysis_mesh::settings_for_geometry(&self.geometry, settings);
        if let Some(old) = &*self.curvature_mesh.borrow()
            && old.settings == settings
        {
            return old.mesh.clone();
        }
        let mesh = match &*self.geometry {
            Geometry::NurbsSurface(s) => s
                .curvature_mesh_with_analysis_settings(settings, self.tolerance)
                .ok(),
            Geometry::Brep(b) => b
                .curvature_mesh_with_analysis_settings(settings, self.tolerance)
                .ok(),
            Geometry::SubD(s) => s.curvature_mesh_with_density(settings.subd_density).ok(),
            _ => None,
        }
        .map(Arc::new);
        *self.curvature_mesh.borrow_mut() = Some(CachedMesh {
            settings,
            mesh: mesh.clone(),
        });
        mesh
    }
    pub(in crate::viewport) fn curvature_colors(
        &self,
        source: Option<&Arc<CurvatureMesh>>,
        mapping: CurvatureColorMapping,
        transform: Option<AffineTransform3>,
        triangles: usize,
    ) -> DraftColors {
        let mut cached = self.curvature_colors.borrow_mut();
        if let Some(c) = cached.as_ref()
            && c.mapping == mapping
            && c.transform == transform
            && c.colors.len() == triangles
            && match (source, c.source.as_ref()) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
        {
            return c.colors.clone();
        }
        let colors: DraftColors = source
            .map_or_else(
                || vec![[[128; 3]; 3]; triangles],
                |m| {
                    m.principal_curvatures(transform)
                        .into_iter()
                        .map(|c| c.map(|k| k.and_then(|k| mapping.color(k)).unwrap_or([128; 3])))
                        .collect()
                },
            )
            .into();
        *cached = Some(Drawing {
            source: source.cloned(),
            mapping,
            transform,
            colors: colors.clone(),
        });
        colors
    }
}

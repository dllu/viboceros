//! Curvature mesh submission shares the standard depth and transform pipeline.
use super::*;

impl Viewport {
    pub(super) fn add_curvature_faces(
        &self,
        scene: &mut GpuSceneBuilder,
        object: &DisplayObject,
        display: &DisplayGeometry,
        curvature: &CurvatureFaces,
        normal_transform: Option<AffineNormalTransform3>,
    ) {
        let mesh = curvature
            .mesh
            .as_ref()
            .map(|m| &m.mesh)
            .or_else(|| display.mesh());
        if let Some(mesh) = mesh {
            let Ok(mapping) = curvature.options.mapping() else {
                return;
            };
            let normals = curvature.mesh.as_ref().map(|m| &m.normals);
            let colors = display.curvature_colors(
                curvature.mesh.as_ref(),
                mapping,
                object.transform,
                mesh.triangles().len(),
            );
            self.add_gpu_mesh_faces_with_normals(
                scene,
                mesh,
                (0..mesh.triangles().len()).map(|i| {
                    let fallback = || {
                        mesh.face_normal(i)
                            .map(|n| NaVector3::new(n.x(), n.y(), n.z()))
                            .unwrap_or_else(|_| NaVector3::new(0., 0., 1.))
                    };
                    normals.map_or_else(
                        || [fallback(); 3],
                        |normals| {
                            normals[i].map(|n| {
                                n.map_or_else(fallback, |n| NaVector3::new(n.x(), n.y(), n.z()))
                            })
                        },
                    )
                }),
                FaceAppearance {
                    color: object.face_color,
                    member_colors: false,
                    draft: None,
                    analysis_colors: Some(colors),
                },
                object.transform.map(|transform| AffineMeshTransform {
                    transform,
                    normals: normal_transform.unwrap(),
                    reverse_normals: object.reversing
                        && matches!(&*display.geometry, Geometry::Brep(_)),
                }),
            );
        }
    }
}

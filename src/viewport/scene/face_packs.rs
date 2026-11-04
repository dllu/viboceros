//! Temporary per-pack colors use the cached SubD display mesh and source map.
use super::*;
impl Viewport {
    pub(super) fn add_face_pack_faces(
        &self,
        scene: &mut GpuSceneBuilder,
        object: &DisplayObject,
        display: &DisplayGeometry,
        colors: &[[u8; 3]],
        normal_transform: Option<AffineNormalTransform3>,
    ) {
        let (Some(mesh), Some(sources)) = (display.mesh(), display.face_sources()) else {
            return;
        };
        let colors = mesh
            .faces()
            .iter()
            .zip(sources)
            .flat_map(|(face, &source)| {
                let color = colors.get(source).copied().unwrap_or([192; 3]);
                std::iter::repeat_n([color; 3], face.indices().len() - 2)
            })
            .collect::<Vec<_>>();
        if colors.len() != mesh.triangles().len() {
            return;
        }
        self.add_gpu_mesh_faces_with_normals(
            scene,
            mesh,
            display.normals().iter().copied(),
            FaceAppearance {
                color: object.face_color,
                member_colors: false,
                draft: None,
                analysis_colors: Some(colors.into()),
            },
            object.transform.map(|transform| AffineMeshTransform {
                transform,
                normals: normal_transform.unwrap(),
                reverse_normals: false,
            }),
        );
    }
}

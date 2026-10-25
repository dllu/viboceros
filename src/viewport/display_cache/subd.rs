//! Flat control nets and smooth native sector normals share source topology.
use super::*;
impl DisplayGeometry {
    pub(super) fn subd_flat(subd: &viboceros_geometry::SubD) -> Option<GeneratedDisplayMesh> {
        let (mesh, face_sources) = subd
            .control_net_mesh_with_face_sources(
                viboceros_geometry::SubDControlNetPriority::Geometry,
            )
            .ok()?;
        let normals = (0..mesh.triangles().len())
            .map(|i| {
                let n = mesh
                    .face_normal(i)
                    .map(|n| NaVector3::new(n.x(), n.y(), n.z()))
                    .unwrap_or_else(|_| NaVector3::new(0., 0., 1.));
                [n; 3]
            })
            .collect();
        Some(GeneratedDisplayMesh {
            mesh,
            normals: Some(normals),
            face_sources: Some(face_sources),
        })
    }
    pub(super) fn subd_display(subd: &viboceros_geometry::SubD) -> Option<GeneratedDisplayMesh> {
        let display = subd.display_mesh_with_normals().ok()?;
        let mesh = display.mesh;
        if display.face_corner_normals.len() != mesh.faces().len()
            || display.face_sources.len() != mesh.face_count()
        {
            return None;
        }
        let mut normals = Vec::with_capacity(mesh.triangles().len());
        for (face_index, (face, native)) in mesh
            .faces()
            .iter()
            .zip(display.face_corner_normals)
            .enumerate()
        {
            let indices = face.indices();
            for (offset, triangle) in mesh.triangles()[2 * face_index..2 * face_index + 2]
                .iter()
                .enumerate()
            {
                let fallback = mesh
                    .face_normal(2 * face_index + offset)
                    .map(|n| NaVector3::new(n.x(), n.y(), n.z()))
                    .unwrap_or_else(|_| NaVector3::new(0., 0., 1.));
                normals.push(triangle.map(|vertex| {
                    let n = native[indices.iter().position(|i| *i == vertex).unwrap()].to_array();
                    NaVector3::new(n[0], n[1], n[2])
                        .try_normalize(Real::EPSILON)
                        .unwrap_or(fallback)
                }));
            }
        }
        Some(GeneratedDisplayMesh {
            mesh,
            normals: Some(normals),
            face_sources: Some(display.face_sources),
        })
    }
}

//! Decode optional B-rep display data owned by the archive bridge.
use super::*;
use viboceros_geometry::Vector3;

pub(super) fn decode(
    handle: &ModelHandle,
    index: usize,
    brep: &Brep,
    scale: f64,
) -> Option<(TriangleMesh, Vec<usize>)> {
    let mut raw = ffi::ViboRenderMesh::default();
    // SAFETY: the handle owns the returned arrays and outlives this copy.
    if unsafe { ffi::vibo_3dm_render_mesh(handle.0.as_ptr(), index, &mut raw) } == 0 {
        return None;
    }
    let (vertices, normals, polygons, sources) = {
        (
            ffi_slice(handle, raw.vertices, raw.vertex_count.checked_mul(3)?).ok()?,
            ffi_slice(
                handle,
                raw.normals,
                if raw.has_normals != 0 {
                    raw.vertex_count.checked_mul(3)?
                } else {
                    0
                },
            )
            .ok()?,
            ffi_slice(handle, raw.faces, raw.face_count.checked_mul(4)?).ok()?,
            ffi_slice(handle, raw.face_sources, raw.face_count).ok()?,
        )
    };
    let vertices = vertices
        .chunks_exact(3)
        .map(|p| Point3::try_new(p[0] * scale, p[1] * scale, p[2] * scale))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let normals = if normals.is_empty() {
        None
    } else {
        Some(
            normals
                .chunks_exact(3)
                .map(|n| Vector3::try_new(n[0], n[1], n[2]))
                .collect::<Result<Vec<_>, _>>()
                .ok()?,
        )
    };
    let sources = sources
        .iter()
        .map(|&index| index as usize)
        .collect::<Vec<_>>();
    if sources.iter().any(|&index| index >= brep.faces().len()) {
        return None;
    }
    let faces = polygons
        .chunks_exact(4)
        .map(|face| {
            if face[2] == face[3] {
                MeshFace::Triangle([face[0], face[1], face[2]])
            } else {
                MeshFace::Quad([face[0], face[1], face[2], face[3]])
            }
        })
        .collect();
    let mesh = TriangleMesh::try_from_face_records(vertices, faces)
        .ok()?
        .try_with_vertex_normals(normals)
        .ok()?;
    Some((mesh, sources))
}

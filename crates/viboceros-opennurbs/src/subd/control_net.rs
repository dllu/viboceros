//! Control-net mesh attributes share the lifetime of their owned native result.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum SubDControlNetPriority {
    #[default]
    Geometry = 0,
    TextureCoordinates = 1,
}

pub fn subd_control_mesh_with_priority(
    bytes: &[u8],
    priority: SubDControlNetPriority,
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    call(|out, error, n| unsafe {
        vibo_subd_control_mesh(bytes.as_ptr(), bytes.len(), priority as u32, out, error, n)
    })
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct NgonSpan {
    first_vertex: usize,
    vertex_count: usize,
    first_face: usize,
    face_count: usize,
}

/// `view` buffers belong to the native owner retained by the caller.
pub(super) unsafe fn ngons(view: &View) -> Result<Vec<crate::mesh::Ngon>, SubDError> {
    if view.mesh_ngon_count > view.face_count
        || view.mesh_ngon_vertex_count > 8_000_000
        || view.mesh_ngon_face_count > view.face_count
        || (view.mesh_texture_count != 0 && view.mesh_texture_count != view.point_count)
        || (view.mesh_normal_count != 0 && view.mesh_normal_count != view.point_count)
        || (view.mesh_ngon_count != 0
            && (view.mesh_ngons.is_null()
                || view.mesh_ngon_vertices.is_null()
                || view.mesh_ngon_faces.is_null()))
        || (view.mesh_texture_count != 0 && view.mesh_texture_coordinates.is_null())
        || (view.mesh_normal_count != 0 && view.mesh_vertex_normals.is_null())
    {
        return Err(SubDError("Invalid control mesh attribute buffers".into()));
    }
    if view.mesh_ngon_count == 0 {
        return Ok(Vec::new());
    }
    let spans = unsafe { std::slice::from_raw_parts(view.mesh_ngons, view.mesh_ngon_count) };
    let vertices =
        unsafe { std::slice::from_raw_parts(view.mesh_ngon_vertices, view.mesh_ngon_vertex_count) };
    let faces =
        unsafe { std::slice::from_raw_parts(view.mesh_ngon_faces, view.mesh_ngon_face_count) };
    spans
        .iter()
        .map(|g| {
            let end_vertex = g
                .first_vertex
                .checked_add(g.vertex_count)
                .filter(|&n| n <= vertices.len());
            let end_face = g
                .first_face
                .checked_add(g.face_count)
                .filter(|&n| n <= faces.len());
            match (end_vertex, end_face) {
                (Some(a), Some(b)) if g.vertex_count >= 3 && g.face_count > 0 => {
                    Ok(crate::mesh::Ngon {
                        vertices: vertices[g.first_vertex..a].to_vec(),
                        faces: faces[g.first_face..b].to_vec(),
                    })
                }
                _ => Err(SubDError("Invalid control mesh n-gon span".into())),
            }
        })
        .collect()
}

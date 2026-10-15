use std::ffi::{c_char, c_int};
use std::ptr::NonNull;
/// Native mesh data without a dependency on the Rust geometry kernel.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MeshData {
    pub vertices: Vec<[f64; 3]>,
    /// Triangles repeat their third index in the fourth slot.
    pub faces: Vec<[u32; 4]>,
    pub colors: Option<Vec<[u8; 4]>>,
    pub ngons: Vec<Ngon>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ngon {
    pub vertices: Vec<u32>,
    pub faces: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("OpenNURBS mesh: {0}")]
pub struct MeshError(pub String);
#[repr(C)]
#[derive(Clone, Copy)]
struct NgonSpan {
    first_vertex: usize,
    vertex_count: usize,
    first_face: usize,
    face_count: usize,
}
#[repr(C)]
struct MeshView {
    vertex_count: usize,
    face_count: usize,
    color_count: usize,
    ngon_count: usize,
    ngon_vertex_count: usize,
    ngon_face_count: usize,
    vertices: *const f64,
    faces: *const u32,
    colors: *const u8,
    ngons: *const NgonSpan,
    ngon_vertices: *const u32,
    ngon_faces: *const u32,
}
#[repr(C)]
struct NativeResult {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vibo_mesh_edit(
        input: *const MeshView,
        op: u32,
        distance: f64,
        angle: f64,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_mesh_result_view(
        result: *const NativeResult,
        out: *mut MeshView,
        changed: *mut usize,
    ) -> c_int;
    fn vibo_mesh_result_free(result: *mut NativeResult);
}
struct Owner(NonNull<NativeResult>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_mesh_result_free(self.0.as_ptr()) }
    }
}
/// Merge coplanar logical mesh regions using OpenNURBS topology and face sets.
pub fn merge_coplanar_mesh(
    data: &MeshData,
    distance: f64,
    angle: f64,
) -> Result<(MeshData, usize), MeshError> {
    edit(data, 1, distance, angle)
}
/// Call OpenNURBS' planar n-gon finder, retaining existing overlays.
pub fn add_planar_mesh_ngons(
    data: &MeshData,
    distance: f64,
) -> Result<(MeshData, usize), MeshError> {
    edit(data, 2, distance, 0.)
}
/// Find an oriented outer boundary, with inner boundaries retained in the faces.
pub fn mesh_outer_boundary(data: &MeshData) -> Result<Vec<u32>, MeshError> {
    let (result, _) = edit(data, 3, 0., 0.)?;
    result
        .ngons
        .first()
        .map(|g| g.vertices.clone())
        .ok_or_else(|| MeshError("No valid outer boundary".into()))
}
fn edit(
    data: &MeshData,
    op: u32,
    distance: f64,
    angle: f64,
) -> Result<(MeshData, usize), MeshError> {
    if data.vertices.len() > 4_000_000
        || data.faces.len() > 1_000_000
        || data.ngons.len() > data.faces.len()
        || data
            .colors
            .as_ref()
            .is_some_and(|c| c.len() != data.vertices.len())
    {
        return Err(MeshError(
            "Invalid mesh dimensions or resource limit".into(),
        ));
    }
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut spans = Vec::new();
    for g in &data.ngons {
        spans.push(NgonSpan {
            first_vertex: vertices.len(),
            vertex_count: g.vertices.len(),
            first_face: faces.len(),
            face_count: g.faces.len(),
        });
        vertices.extend_from_slice(&g.vertices);
        faces.extend_from_slice(&g.faces);
    }
    let view = MeshView {
        vertex_count: data.vertices.len(),
        face_count: data.faces.len(),
        color_count: data.colors.as_ref().map_or(0, Vec::len),
        ngon_count: spans.len(),
        ngon_vertex_count: vertices.len(),
        ngon_face_count: faces.len(),
        vertices: data.vertices.as_ptr().cast(),
        faces: data.faces.as_ptr().cast(),
        colors: data
            .colors
            .as_ref()
            .map_or(std::ptr::null(), |v| v.as_ptr().cast()),
        ngons: spans.as_ptr(),
        ngon_vertices: vertices.as_ptr(),
        ngon_faces: faces.as_ptr(),
    };
    let mut error = [0u8; 512];
    let mut raw = std::ptr::null_mut();
    let status = unsafe {
        vibo_mesh_edit(
            &view,
            op,
            distance,
            angle,
            &mut raw,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(raw).map(Owner);
    if status == 0 {
        let end = error.iter().position(|b| *b == 0).unwrap_or(error.len());
        return Err(MeshError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| MeshError("Missing result".into()))?;
    let mut out = std::mem::MaybeUninit::<MeshView>::uninit();
    let mut changed = 0;
    if unsafe { vibo_mesh_result_view(owner.0.as_ptr(), out.as_mut_ptr(), &mut changed) } == 0 {
        return Err(MeshError("Missing result view".into()));
    }
    let out = unsafe { out.assume_init() };
    // The owned native result keeps all validated spans alive while copying.
    unsafe fn copy<T: Copy>(p: *const T, n: usize) -> Result<Vec<T>, MeshError> {
        if n == 0 {
            return Ok(vec![]);
        }
        if p.is_null() || n > 32_000_000 {
            return Err(MeshError("Invalid result array".into()));
        }
        Ok(unsafe { std::slice::from_raw_parts(p, n) }.to_vec())
    }
    let mut result = MeshData {
        vertices: unsafe { copy(out.vertices.cast::<[f64; 3]>(), out.vertex_count)? },
        faces: unsafe { copy(out.faces.cast::<[u32; 4]>(), out.face_count)? },
        colors: None,
        ngons: Vec::new(),
    };
    if out.color_count > 0 {
        result.colors = Some(unsafe { copy(out.colors.cast::<[u8; 4]>(), out.color_count)? });
    }
    let vertex_indices = unsafe { copy(out.ngon_vertices, out.ngon_vertex_count)? };
    let face_indices = unsafe { copy(out.ngon_faces, out.ngon_face_count)? };
    for g in unsafe { copy(out.ngons, out.ngon_count)? } {
        let vertex_end = g
            .first_vertex
            .checked_add(g.vertex_count)
            .ok_or_else(|| MeshError("Invalid n-gon span".into()))?;
        let face_end = g
            .first_face
            .checked_add(g.face_count)
            .ok_or_else(|| MeshError("Invalid n-gon span".into()))?;
        result.ngons.push(Ngon {
            vertices: vertex_indices
                .get(g.first_vertex..vertex_end)
                .ok_or_else(|| MeshError("Invalid n-gon vertices".into()))?
                .to_vec(),
            faces: face_indices
                .get(g.first_face..face_end)
                .ok_or_else(|| MeshError("Invalid n-gon faces".into()))?
                .to_vec(),
        });
    }
    Ok((result, changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mesh() -> MeshData {
        MeshData {
            vertices: vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            faces: vec![[0, 1, 2, 2], [0, 2, 3, 3]],
            ..Default::default()
        }
    }
    #[test]
    fn malformed_native_inputs_fail_without_mutating_the_source() {
        for stage in 0..5 {
            let mut m = mesh();
            match stage {
                0 => m.vertices[0][0] = f64::NAN,
                1 => m.faces[0][0] = 99,
                2 => m.colors = Some(vec![]),
                3 => {
                    m.ngons = vec![Ngon {
                        vertices: vec![0, 1, 2],
                        faces: vec![99],
                    }]
                }
                _ => {
                    m.ngons = vec![Ngon {
                        vertices: vec![0, 1, 2],
                        faces: vec![0, 0],
                    }]
                }
            }
            assert!(
                merge_coplanar_mesh(&m, 1e-9, 1e-8).is_err(),
                "stage {stage}"
            );
        }
        let m = mesh();
        let before = m.clone();
        assert!(merge_coplanar_mesh(&m, f64::NAN, 1e-8).is_err());
        assert_eq!(m, before);
    }
}

#[cfg(test)]
mod hole_tests {
    use super::*;
    #[test]
    fn collinear_inner_boundary_starts_choose_the_outer_loop_in_all_planes() {
        for axis in 0..3 {
            let vertices = (0..5)
                .flat_map(|y| {
                    (0..5).map(move |x| match axis {
                        0 => [x as f64, y as f64, 0.],
                        1 => [0., x as f64, y as f64],
                        _ => [y as f64, 0., x as f64],
                    })
                })
                .collect();
            let mut faces = vec![[1, 7, 6, 6], [1, 2, 7, 7]];
            for y in 0..4 {
                for x in 0..4 {
                    if (x == 1 || x == 2) && (y == 1 || y == 2) || (x == 1 && y == 0) {
                        continue;
                    }
                    let a = (y * 5 + x) as u32;
                    faces.push([a, a + 1, a + 6, a + 6]);
                    faces.push([a, a + 6, a + 5, a + 5]);
                }
            }
            let data = MeshData {
                vertices,
                faces,
                ..Default::default()
            };
            let (result, n) = merge_coplanar_mesh(&data, 1e-9, 1e-8).unwrap();
            assert_eq!(n, 23);
            assert_eq!(result.ngons.len(), 1);
            assert_eq!(result.ngons[0].vertices.len(), 16, "plane {axis}");
        }
    }
}

#[cfg(test)]
mod precision_tests {
    use super::*;
    #[test]
    fn sub_float_gaps_at_remote_origins_do_not_become_shared_edges() {
        let s = 1e6;
        let gap = 1e-4;
        let data = MeshData {
            vertices: vec![
                [s, s, 0.],
                [s + 4., s, 0.],
                [s + 4., s + 3., 0.],
                [s + gap, s, 0.],
                [s + 4. + gap, s + 3., 0.],
                [s + gap, s + 3., 0.],
            ],
            faces: vec![[0, 1, 2, 2], [3, 4, 5, 5]],
            ..Default::default()
        };
        let (result, n) = merge_coplanar_mesh(&data, 1e-9, 1e-8).unwrap();
        assert_eq!(n, 0);
        assert_eq!(result, data);
    }
}

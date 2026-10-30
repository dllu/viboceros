//! Append cap drafts without rebuilding the source SubD or its component data.
use super::*;
unsafe extern "C" {
    fn vibo_subd_append_caps(
        bytes: *const u8,
        count: usize,
        points: *const f64,
        point_count: usize,
        sizes: *const u32,
        face_count: usize,
        corners: *const u32,
        corner_count: usize,
        edges: *const usize,
        edge_count: usize,
        crease: c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_append_caps(
    bytes: &[u8],
    points: &[[f64; 3]],
    faces: &[Vec<u32>],
    boundary_edges: &[usize],
    crease: bool,
) -> Result<SubDData, SubDError> {
    let (sizes, corners) = append_arrays(bytes, points, faces)?;
    if boundary_edges.is_empty() || boundary_edges.len() > 400_000 {
        return Err(SubDError("Invalid cap boundary selection".into()));
    }
    call(|out, error, n| unsafe {
        vibo_subd_append_caps(
            bytes.as_ptr(),
            bytes.len(),
            points.as_flattened().as_ptr(),
            points.len(),
            sizes.as_ptr(),
            sizes.len(),
            corners.as_ptr(),
            corners.len(),
            boundary_edges.as_ptr(),
            boundary_edges.len(),
            i32::from(crease),
            out,
            error,
            n,
        )
    })
}

//! Exact smooth-sector limit normals in requested source vertex order.
use super::*;

#[repr(C)]
struct NativeNormals {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default)]
struct NormalsView {
    count: usize,
    normals: *const [f64; 3],
}
unsafe extern "C" {
    fn vibo_subd_smooth_vertex_normals(
        bytes: *const u8,
        count: usize,
        indices: *const usize,
        index_count: usize,
        out: *mut *mut NativeNormals,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_vertex_normals_view(result: *const NativeNormals, out: *mut NormalsView) -> c_int;
    fn vibo_subd_vertex_normals_free(result: *mut NativeNormals);
}
struct NormalsOwner(NonNull<NativeNormals>);
impl Drop for NormalsOwner {
    fn drop(&mut self) {
        unsafe { vibo_subd_vertex_normals_free(self.0.as_ptr()) }
    }
}

/// Unit limit normals for smooth control vertices, in caller order. Repeated
/// indices are allowed. Crease/corner/dart vertices and undefined normals reject.
/// The archive remains immutable; no surface tessellation is constructed.
pub fn subd_smooth_vertex_normals(
    bytes: &[u8],
    vertices: &[usize],
) -> Result<Vec<[f64; 3]>, SubDError> {
    check_archive(bytes)?;
    if vertices.len() > 100_000 {
        return Err(SubDError("SubD normal query budget exceeded".into()));
    }
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_smooth_vertex_normals(
            bytes.as_ptr(),
            bytes.len(),
            vertices.as_ptr(),
            vertices.len(),
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(NormalsOwner);
    if status != 1 {
        let end = error.iter().position(|&v| v == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing SubD normal result".into()))?;
    let mut view = NormalsView::default();
    if unsafe { vibo_subd_vertex_normals_view(owner.0.as_ptr(), &mut view) } != 1
        || view.count != vertices.len()
        || (view.count > 0 && view.normals.is_null())
    {
        return Err(SubDError("Invalid SubD normal view".into()));
    }
    if view.count == 0 {
        return Ok(Vec::new());
    }
    let normals = unsafe { std::slice::from_raw_parts(view.normals, view.count) };
    if normals.iter().any(|v| {
        v.iter().any(|c| !c.is_finite()) || (v[0].hypot(v[1]).hypot(v[2]) - 1.).abs() > 1e-12
    }) {
        return Err(SubDError("Invalid SubD limit normal".into()));
    }
    Ok(normals.to_vec())
}

//! Active component IDs can have gaps after native mesh conversion or edits.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubDComponentIds {
    pub vertices: Vec<u32>,
    pub edges: Vec<u32>,
    pub faces: Vec<u32>,
}
#[repr(C)]
struct NativeIds {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default)]
struct View {
    vertex_count: usize,
    edge_count: usize,
    face_count: usize,
    vertices: *const u32,
    edges: *const u32,
    faces: *const u32,
}
unsafe extern "C" {
    fn vibo_subd_component_ids(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativeIds,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_component_ids_view(value: *const NativeIds, out: *mut View) -> c_int;
    fn vibo_subd_component_ids_free(value: *mut NativeIds);
}
struct Owner(NonNull<NativeIds>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_subd_component_ids_free(self.0.as_ptr()) }
    }
}
pub fn subd_component_ids(bytes: &[u8]) -> Result<SubDComponentIds, SubDError> {
    check_archive(bytes)?;
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_component_ids(
            bytes.as_ptr(),
            bytes.len(),
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(Owner);
    if status != 1 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing SubD component IDs".into()))?;
    let mut view = View::default();
    if unsafe { vibo_subd_component_ids_view(owner.0.as_ptr(), &mut view) } != 1 {
        return Err(SubDError("Invalid SubD ID view".into()));
    }
    let result = SubDComponentIds {
        vertices: owned_ids(view.vertices, view.vertex_count, 100_000)?,
        edges: owned_ids(view.edges, view.edge_count, 400_000)?,
        faces: owned_ids(view.faces, view.face_count, 100_000)?,
    };
    for ids in [&result.vertices, &result.edges, &result.faces] {
        if ids.first() == Some(&0) || ids.windows(2).any(|p| p[0] >= p[1]) {
            return Err(SubDError("Invalid SubD component ID order".into()));
        }
    }
    Ok(result)
}

fn owned_ids(pointer: *const u32, count: usize, limit: usize) -> Result<Vec<u32>, SubDError> {
    if count > limit || (count > 0 && pointer.is_null()) {
        return Err(SubDError("Invalid SubD ID buffer".into()));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    // The owned native result keeps this checked buffer alive for the copy.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count) }.to_vec())
}

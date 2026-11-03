//! Read retained face-pack identity and charts through public OpenNURBS APIs.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubDFacePack {
    pub id: u32,
    pub origin: [f64; 2],
    pub size: [f64; 2],
    /// Clockwise rotation in OpenNURBS packing-rectangle convention.
    pub quarter_turns: u8,
}
#[repr(C)]
struct NativePacks {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Record {
    id: u32,
    rotation: u32,
    set: u32,
    rectangle: [f64; 4],
}
#[repr(C)]
#[derive(Default)]
struct View {
    count: usize,
    records: *const Record,
}
unsafe extern "C" {
    fn vibo_subd_face_packs(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativePacks,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_face_packs_view(result: *const NativePacks, out: *mut View) -> c_int;
    fn vibo_subd_face_packs_free(result: *mut NativePacks);
}
struct Owner(NonNull<NativePacks>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_subd_face_packs_free(self.0.as_ptr()) }
    }
}

/// One entry per source face, ordered by native component ID. Unset IDs or
/// missing pack rectangles return None. This reads metadata without assigning
/// packs, modifying the archive, or constructing a tessellation.
pub fn subd_face_packs(bytes: &[u8]) -> Result<Vec<Option<SubDFacePack>>, SubDError> {
    check_archive(bytes)?;
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_face_packs(
            bytes.as_ptr(),
            bytes.len(),
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(Owner);
    if status != 1 {
        let end = error.iter().position(|&v| v == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing SubD face-pack result".into()))?;
    let mut view = View::default();
    if unsafe { vibo_subd_face_packs_view(owner.0.as_ptr(), &mut view) } != 1
        || view.count > 100_000
        || (view.count > 0 && view.records.is_null())
    {
        return Err(SubDError("Invalid SubD face-pack view".into()));
    }
    if view.count == 0 {
        return Ok(Vec::new());
    }
    let records = unsafe { std::slice::from_raw_parts(view.records, view.count) };
    records
        .iter()
        .map(|r| {
            if r.id == 0 || r.set == 0 {
                return Ok(None);
            }
            let [x, y, w, h] = r.rectangle;
            if r.set != 1
                || r.rotation > 3
                || r.rectangle.iter().any(|v| !v.is_finite())
                || x < 0.
                || y < 0.
                || x >= 1.
                || y >= 1.
                || w <= 0.
                || h <= 0.
                || x + w > 1. + f64::EPSILON.sqrt()
                || y + h > 1. + f64::EPSILON.sqrt()
            {
                return Err(SubDError("Invalid SubD face-pack rectangle".into()));
            }
            Ok(Some(SubDFacePack {
                id: r.id,
                origin: [x, y],
                size: [w, h],
                quarter_turns: r.rotation as u8,
            }))
        })
        .collect()
}

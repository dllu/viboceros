//! Retained face-pack identity and charts, with independent metadata assignment.
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
    fn vibo_subd_set_face_packs(
        bytes: *const u8,
        count: usize,
        records: *const Record,
        record_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
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

/// Apply a complete packing supplied by the geometry layer. The native bridge
/// checks every rectangle and retains all non-packing component properties.
/// The caller supplies connected rectangular quad groups with consistent chart
/// uses and separate IDs for non-quads; this expert setter does not infer them.
pub fn subd_set_face_packs(bytes: &[u8], packs: &[SubDFacePack]) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if packs.is_empty() || packs.len() > 100_000 {
        return Err(SubDError("Invalid SubD face-pack count".into()));
    }
    let records = packs
        .iter()
        .map(|p| Record {
            id: p.id,
            rotation: p.quarter_turns as u32,
            set: 1,
            rectangle: [p.origin[0], p.origin[1], p.size[0], p.size[1]],
        })
        .collect::<Vec<_>>();
    call(|out, error, capacity| unsafe {
        vibo_subd_set_face_packs(
            bytes.as_ptr(),
            bytes.len(),
            records.as_ptr(),
            records.len(),
            out,
            error,
            capacity,
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_pack_setter_rejects_incomplete_and_invalid_native_records() {
        let source = subd_from_polygons(
            &[[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            &[vec![0, 1, 2, 3]],
        )
        .unwrap();
        let valid = SubDFacePack {
            id: 1,
            origin: [0.; 2],
            size: [1.; 2],
            quarter_turns: 0,
        };
        let before = subd_face_packs(&source.archive).unwrap();
        assert!(subd_set_face_packs(&source.archive, &[]).is_err());
        assert!(subd_set_face_packs(&source.archive, &[valid; 2]).is_err());
        for invalid in [
            SubDFacePack { id: 0, ..valid },
            SubDFacePack {
                quarter_turns: 4,
                ..valid
            },
            SubDFacePack {
                origin: [f64::NAN, 0.],
                ..valid
            },
            SubDFacePack {
                size: [0., 1.],
                ..valid
            },
            SubDFacePack {
                size: [2., 1.],
                ..valid
            },
        ] {
            assert!(subd_set_face_packs(&source.archive, &[invalid]).is_err());
        }
        assert_eq!(before, subd_face_packs(&source.archive).unwrap());
        let packed = subd_set_face_packs(&source.archive, &[valid]).unwrap();
        assert_eq!(subd_face_packs(&packed.archive).unwrap(), vec![Some(valid)]);
        assert_eq!(packed.points, source.points);
        assert_eq!(packed.faces, source.faces);
    }
}

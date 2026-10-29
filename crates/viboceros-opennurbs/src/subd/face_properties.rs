//! Public per-face color and material properties, aligned with native face IDs.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubDFaceProperties {
    /// RGBA with OpenNURBS transparency: alpha 0 is opaque, 255 transparent.
    /// None inherits the object color. [255;4] is the reserved unset value.
    pub color: Option<[u8; 4]>,
    /// Index into the object's material channels; zero inherits its material.
    pub material_channel: u16,
}
#[repr(C)]
struct FaceProperties {
    face: u32,
    color: u32,
    material: u32,
}
unsafe extern "C" {
    fn vibo_subd_set_face_properties(
        bytes: *const u8,
        count: usize,
        input: *const FaceProperties,
        input_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_set_face_properties(
    bytes: &[u8],
    edits: &[(usize, SubDFaceProperties)],
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if edits.len() > 100_000 {
        return Err(SubDError("Excessive face-property edits".into()));
    }
    let edits = edits
        .iter()
        .map(|(index, value)| {
            if value.color == Some([255; 4]) {
                return Err(SubDError("Reserved unset face color; use None".into()));
            }
            Ok(FaceProperties {
                face: (*index)
                    .try_into()
                    .map_err(|_| SubDError("Face-property index overflow".into()))?,
                color: value.color.map_or(u32::MAX, u32::from_le_bytes),
                material: u32::from(value.material_channel),
            })
        })
        .collect::<Result<Vec<_>, SubDError>>()?;
    call(|out, error, capacity| unsafe {
        vibo_subd_set_face_properties(
            bytes.as_ptr(),
            bytes.len(),
            edits.as_ptr(),
            edits.len(),
            out,
            error,
            capacity,
        )
    })
}

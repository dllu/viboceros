//! Custom texture coordinates on control-face corners, using public OpenNURBS.
use super::*;

/// Source of evaluated SubD texture coordinates in the native archive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum SubDTextureCoordinateType {
    #[default]
    Unset = 0,
    Unpacked = 1,
    Packed = 2,
    Zero = 3,
    Nan = 4,
    FromFaceTexturePoints = 6,
    FromMapping = 7,
}
impl TryFrom<u32> for SubDTextureCoordinateType {
    type Error = SubDError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Unset,
            1 => Self::Unpacked,
            2 => Self::Packed,
            3 => Self::Zero,
            4 => Self::Nan,
            6 => Self::FromFaceTexturePoints,
            7 => Self::FromMapping,
            _ => return Err(SubDError("Unknown SubD texture coordinate source".into())),
        })
    }
}
#[repr(C)]
struct TextureEdit {
    face: u32,
    count: u32,
    points: *const f64,
}
unsafe extern "C" {
    fn vibo_subd_set_face_texture_points(
        bytes: *const u8,
        count: usize,
        input: *const TextureEdit,
        input_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_set_face_texture_points(
    bytes: &[u8],
    edits: &[(usize, Option<Vec<[f64; 3]>>)],
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    let count = edits
        .iter()
        .try_fold(0usize, |count, (_, points)| {
            count.checked_add(points.as_ref().map_or(0, Vec::len))
        })
        .ok_or_else(|| SubDError("Texture point count overflow".into()))?;
    if edits.len() > 100_000 || count > 400_000 {
        return Err(SubDError("Excessive face texture edits".into()));
    }
    let input = edits
        .iter()
        .map(|(face, points)| {
            if points
                .as_ref()
                .is_some_and(|p| p.is_empty() || p.as_flattened().iter().any(|x| !x.is_finite()))
            {
                return Err(SubDError(
                    "Texture points must be finite and nonempty; use None to clear".into(),
                ));
            }
            Ok(TextureEdit {
                face: (*face)
                    .try_into()
                    .map_err(|_| SubDError("Texture face index overflow".into()))?,
                count: points.as_ref().map_or(0, Vec::len) as u32,
                points: points
                    .as_ref()
                    .map_or(std::ptr::null(), |p| p.as_ptr().cast()),
            })
        })
        .collect::<Result<Vec<_>, SubDError>>()?;
    call(|out, error, capacity| unsafe {
        vibo_subd_set_face_texture_points(
            bytes.as_ptr(),
            bytes.len(),
            input.as_ptr(),
            input.len(),
            out,
            error,
            capacity,
        )
    })
}

pub(super) fn coordinate(value: [f64; 3]) -> Option<[f64; 3]> {
    value.iter().all(|x| x.is_finite()).then_some(value)
}
#[cfg(test)]
mod tests {
    use super::coordinate;
    #[test]
    fn nonfinite_native_texture_corners_are_explicitly_unset() {
        assert_eq!(coordinate([1., 2., 3.]), Some([1., 2., 3.]));
        assert_eq!(coordinate([f64::NAN, 0., 0.]), None);
        assert_eq!(coordinate([0., f64::INFINITY, 0.]), None);
    }
}

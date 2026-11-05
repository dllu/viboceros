//! Public OpenNURBS orientation witnesses and reversal of private archive copies.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubDSolidOrientation {
    NotSolid,
    Outward,
    Inward,
    Unknown,
}
impl SubDSolidOrientation {
    pub(super) fn from_native(value: c_int) -> Result<Self, SubDError> {
        match value {
            0 => Ok(Self::NotSolid),
            1 => Ok(Self::Outward),
            -1 => Ok(Self::Inward),
            2 => Ok(Self::Unknown),
            _ => Err(SubDError("Invalid SubD solid-orientation witness".into())),
        }
    }
}
unsafe extern "C" {
    fn vibo_subd_reverse_orientation(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_reverse_orientation(bytes: &[u8]) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    call(|out, error, capacity| unsafe {
        vibo_subd_reverse_orientation(bytes.as_ptr(), bytes.len(), out, error, capacity)
    })
}

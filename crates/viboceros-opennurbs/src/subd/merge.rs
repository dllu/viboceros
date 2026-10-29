//! Component-selected face regions on an owned decoded SubD archive.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubDMergeComponent {
    Vertex(usize),
    /// Original control-point ordinals identifying an existing edge.
    Edge([usize; 2]),
    Face(usize),
}
#[repr(C)]
struct Component {
    kind: u32,
    first: u32,
    second: u32,
}
unsafe extern "C" {
    fn vibo_subd_merge_faces(
        bytes: *const u8,
        count: usize,
        input: *const Component,
        input_count: usize,
        removed: *mut usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
/// Returns the changed control net and number of removed original faces.
/// `selected_faces` identifies newly merged faces in the resulting table.
/// Indices refer to the immutable source tables sorted by native component ID.
pub fn subd_merge_faces(
    bytes: &[u8],
    input: &[SubDMergeComponent],
) -> Result<(SubDData, usize), SubDError> {
    check_archive(bytes)?;
    if input.len() > 400_000 {
        return Err(SubDError("Excessive SubD merge selection".into()));
    }
    let input = input
        .iter()
        .map(|c| {
            let (kind, first, second) = match *c {
                SubDMergeComponent::Vertex(i) => (0, i, 0),
                SubDMergeComponent::Edge([a, b]) => (1, a, b),
                SubDMergeComponent::Face(i) => (2, i, 0),
            };
            Ok(Component {
                kind,
                first: first
                    .try_into()
                    .map_err(|_| SubDError("SubD merge index overflow".into()))?,
                second: second
                    .try_into()
                    .map_err(|_| SubDError("SubD merge index overflow".into()))?,
            })
        })
        .collect::<Result<Vec<_>, SubDError>>()?;
    let mut removed = 0;
    let result = call(|out, error, capacity| unsafe {
        vibo_subd_merge_faces(
            bytes.as_ptr(),
            bytes.len(),
            input.as_ptr(),
            input.len(),
            &mut removed,
            out,
            error,
            capacity,
        )
    })?;
    Ok((result, removed))
}

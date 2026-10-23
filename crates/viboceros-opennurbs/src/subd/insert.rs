//! Bounded public topology splits with actual output component ordinals.
use super::*;
unsafe extern "C" {
    fn vibo_subd_insert_path(
        bytes: *const u8,
        count: usize,
        edges: *const usize,
        parameters: *const f64,
        points: *const f64,
        split_count: usize,
        chords: *const u32,
        chord_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_insert_vertex(
        bytes: *const u8,
        count: usize,
        index: usize,
        point: *const f64,
        vertex: *mut usize,
        new_edge: *mut usize,
        changed: *mut c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_insert_face_edge(
        bytes: *const u8,
        count: usize,
        face: usize,
        a: usize,
        b: usize,
        new_edge: *mut usize,
        changed: *mut c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
/// Applies an entire path with one archive decode, native edit, and encode.
pub fn subd_insert_path(
    bytes: &[u8],
    splits: &[(usize, f64, [f64; 3])],
    chords: &[[u32; 2]],
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if splits.len() > 1_000_000 || chords.len() > 1_000_000 {
        return Err(SubDError("Excessive SubD insertion path".into()));
    }
    let edges = splits.iter().map(|s| s.0).collect::<Vec<_>>();
    let parameters = splits.iter().map(|s| s.1).collect::<Vec<_>>();
    let points = splits.iter().flat_map(|s| s.2).collect::<Vec<_>>();
    call(|out, error, capacity| unsafe {
        vibo_subd_insert_path(
            bytes.as_ptr(),
            bytes.len(),
            edges.as_ptr(),
            parameters.as_ptr(),
            points.as_ptr(),
            splits.len(),
            chords.as_flattened().as_ptr(),
            chords.len(),
            out,
            error,
            capacity,
        )
    })
}
pub fn subd_insert_vertex(
    bytes: &[u8],
    edge: usize,
    point: [f64; 3],
) -> Result<Option<(SubDData, usize, usize)>, SubDError> {
    check_archive(bytes)?;
    let (mut vertex, mut new_edge, mut changed) = (usize::MAX, usize::MAX, 0);
    let data = call(|out, error, capacity| unsafe {
        vibo_subd_insert_vertex(
            bytes.as_ptr(),
            bytes.len(),
            edge,
            point.as_ptr(),
            &mut vertex,
            &mut new_edge,
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    if changed == 0 {
        return Ok(None);
    }
    if vertex >= data.points.len() || new_edge >= data.edges.len() {
        return Err(SubDError(
            "Invalid inserted SubD vertex or edge ordinal".into(),
        ));
    }
    Ok(Some((data, vertex, new_edge)))
}
pub fn subd_insert_face_edge(
    bytes: &[u8],
    face: usize,
    a: usize,
    b: usize,
) -> Result<Option<(SubDData, usize)>, SubDError> {
    check_archive(bytes)?;
    let (mut edge, mut changed) = (usize::MAX, 0);
    let data = call(|out, error, capacity| unsafe {
        vibo_subd_insert_face_edge(
            bytes.as_ptr(),
            bytes.len(),
            face,
            a,
            b,
            &mut edge,
            &mut changed,
            out,
            error,
            capacity,
        )
    })?;
    if changed == 0 {
        return Ok(None);
    }
    if edge >= data.edges.len() {
        return Err(SubDError("Invalid inserted SubD edge ordinal".into()));
    }
    Ok(Some((data, edge)))
}

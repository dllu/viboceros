//! Bounded native component extrusion on an owned archive.
use super::*;
#[derive(Clone, Copy, Debug)]
pub enum SubDExtrudeDirection {
    World([f64; 3]),
    VertexNormals,
    EdgeOutward,
}
unsafe extern "C" {
    fn vibo_subd_extrude(
        bytes: *const u8,
        count: usize,
        faces: *const usize,
        face_count: usize,
        edges: *const usize,
        edge_count: usize,
        whole: c_int,
        mode: c_int,
        distance: f64,
        direction: *const f64,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_extrude(
    bytes: &[u8],
    faces: &[usize],
    edges: &[usize],
    whole: bool,
    distance: f64,
    direction: SubDExtrudeDirection,
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if !distance.is_finite() || distance == 0. || faces.len() > 100_000 || edges.len() > 400_000 {
        return Err(SubDError("Invalid SubD extrusion parameters".into()));
    }
    let (mode, vector) = match direction {
        SubDExtrudeDirection::World(v) => (0, v),
        SubDExtrudeDirection::VertexNormals => (1, [0.; 3]),
        SubDExtrudeDirection::EdgeOutward => (2, [0.; 3]),
    };
    call(|out, error, capacity| unsafe {
        vibo_subd_extrude(
            bytes.as_ptr(),
            bytes.len(),
            faces.as_ptr(),
            faces.len(),
            edges.as_ptr(),
            edges.len(),
            whole.into(),
            mode,
            distance,
            vector.as_ptr(),
            out,
            error,
            capacity,
        )
    })
}

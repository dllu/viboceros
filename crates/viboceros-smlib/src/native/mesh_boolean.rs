//! Owned working copies for SMLib's polygon Boolean operations.
use super::*;

unsafe extern "C" {
    fn vb_mesh_boolean_interaction(
        first: *const f64,
        first_count: usize,
        first_faces: *const u32,
        first_face_count: usize,
        second: *const f64,
        second_count: usize,
        second_faces: *const u32,
        second_face_count: usize,
        tolerance: f64,
        interacts: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_mesh_boolean(
        first: *const f64,
        first_count: usize,
        first_faces: *const u32,
        first_face_count: usize,
        second: *const f64,
        second_count: usize,
        second_faces: *const u32,
        second_face_count: usize,
        operation: c_int,
        tolerance: f64,
        out: *mut *mut NativeMesh,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

pub(super) fn prepare(mesh: &TriangleMesh) -> Result<(Vec<f64>, Vec<u32>), Error> {
    let triangulated = mesh
        .faces()
        .iter()
        .any(|face| face.is_quad())
        .then(|| mesh.triangulate_quads(Tolerance::MESH_VALIDATION))
        .transpose()?;
    let mesh = triangulated.as_ref().map_or(mesh, |(mesh, _)| mesh);
    Ok((
        mesh.vertices().iter().flat_map(|p| p.to_array()).collect(),
        mesh.triangles().iter().flatten().copied().collect(),
    ))
}

/// Kernel contact or containment relevant to polygon union. Purely disjoint
/// operands do not require document replacements. This uses SMLib's imprint
/// and classification mode on independent copies, including enclosed shells.
pub fn mesh_boolean_interaction(
    first: &TriangleMesh,
    second: &TriangleMesh,
    tolerance: Tolerance,
) -> Result<bool, Error> {
    let (a, a_faces) = prepare(first)?;
    let (b, b_faces) = prepare(second)?;
    let [amin, amax, bmin, bmax] = [
        first.bounds().min().to_array(),
        first.bounds().max().to_array(),
        second.bounds().min().to_array(),
        second.bounds().max().to_array(),
    ];
    if (0..3).any(|axis| amin[axis] > bmax[axis] || bmin[axis] > amax[axis]) {
        return Ok(false);
    }
    let mut interacts = 0;
    call(|error, capacity| unsafe {
        vb_mesh_boolean_interaction(
            a.as_ptr(),
            a.len() / 3,
            a_faces.as_ptr(),
            a_faces.len() / 3,
            b.as_ptr(),
            b.len() / 3,
            b_faces.as_ptr(),
            b_faces.len() / 3,
            tolerance.absolute(),
            &mut interacts,
            error,
            capacity,
        )
    })?;
    Ok(interacts != 0)
}

/// Polygon union, intersection or first-minus-second. Inputs and their stored
/// channels remain unchanged. An empty intersection/difference returns `None`.
/// Output faces are triangles; channels are rebuilt by the application.
pub fn mesh_boolean(
    first: &TriangleMesh,
    second: &TriangleMesh,
    operation: BooleanOperation,
    tolerance: Tolerance,
) -> Result<Option<TriangleMesh>, Error> {
    let (a, a_faces) = prepare(first)?;
    let (b, b_faces) = prepare(second)?;
    let mut pointer = std::ptr::null_mut();
    let operation = match operation {
        BooleanOperation::Union => 0,
        BooleanOperation::Intersection => 1,
        BooleanOperation::Difference => 2,
    };
    call(|error, capacity| unsafe {
        vb_mesh_boolean(
            a.as_ptr(),
            a.len() / 3,
            a_faces.as_ptr(),
            a_faces.len() / 3,
            b.as_ptr(),
            b.len() / 3,
            b_faces.as_ptr(),
            b_faces.len() / 3,
            operation,
            tolerance.absolute(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let Some(pointer) = NonNull::new(pointer) else {
        return Ok(None);
    };
    Ok(Some(OwnedMesh(pointer).decode(Tolerance::MESH_VALIDATION)?))
}

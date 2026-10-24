//! Transverse and coplanar boundary segments on independent kernel copies.
use super::*;
mod coplanar;

#[repr(C)]
struct NativeIntersections {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vb_mesh_intersections(
        first: *const f64,
        first_count: usize,
        first_faces: *const u32,
        first_face_count: usize,
        second: *const f64,
        second_count: usize,
        second_faces: *const u32,
        second_face_count: usize,
        tolerance: f64,
        out: *mut *mut NativeIntersections,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_mesh_intersections_sizes(
        p: *const NativeIntersections,
        first: *mut usize,
        second: *mut usize,
    ) -> c_int;
    fn vb_mesh_intersections_copy(
        p: *const NativeIntersections,
        first: *mut f64,
        first_count: usize,
        second: *mut u32,
        second_count: usize,
    ) -> c_int;
    fn vb_mesh_intersections_free(p: *mut NativeIntersections);
}
struct OwnedIntersections(NonNull<NativeIntersections>);
impl Drop for OwnedIntersections {
    fn drop(&mut self) {
        unsafe { vb_mesh_intersections_free(self.0.as_ptr()) };
    }
}

#[derive(Clone, Debug)]
pub struct MeshIntersections {
    /// Finite lines, including coplanar triangle pairs sharing no area.
    pub transverse: Vec<[Point3; 2]>,
    /// Boundaries of overlapping coplanar coverage, including holes.
    pub coplanar: Vec<[Point3; 2]>,
}

/// Imprint both meshes, retaining intersection edges without classifying solids.
/// Coplanar regions return their coverage boundaries. Triangle pairs that only
/// share a line contribute that line even when coplanar area is suppressed.
/// Input vertex channels and topology remain untouched, including on failure.
pub fn mesh_intersections(
    first: &TriangleMesh,
    second: &TriangleMesh,
    tolerance: Tolerance,
) -> Result<MeshIntersections, Error> {
    first.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
    second.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
    let [amin, amax, bmin, bmax] = [
        first.bounds().min().to_array(),
        first.bounds().max().to_array(),
        second.bounds().min().to_array(),
        second.bounds().max().to_array(),
    ];
    if (0..3).any(|axis| {
        amin[axis] > bmax[axis] + tolerance.absolute()
            || bmin[axis] > amax[axis] + tolerance.absolute()
    }) {
        return Ok(MeshIntersections {
            transverse: Vec::new(),
            coplanar: Vec::new(),
        });
    }
    let (a, af) = mesh_boolean::prepare(first)?;
    let (b, bf) = mesh_boolean::prepare(second)?;
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_mesh_intersections(
            a.as_ptr(),
            a.len() / 3,
            af.as_ptr(),
            af.len() / 3,
            b.as_ptr(),
            b.len() / 3,
            bf.as_ptr(),
            bf.len() / 3,
            tolerance.absolute(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let owned = OwnedIntersections(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
    let (mut na, mut nb) = (0, 0);
    if unsafe { vb_mesh_intersections_sizes(owned.0.as_ptr(), &mut na, &mut nb) } != 0
        || na > 1_000_000
        || nb > 1_000_000
    {
        return Err(Error::InvalidOutput);
    }
    let (mut lines, mut pairs) = (vec![0.; 6 * na], vec![0; 2 * nb]);
    if unsafe {
        vb_mesh_intersections_copy(
            owned.0.as_ptr(),
            lines.as_mut_ptr(),
            na,
            pairs.as_mut_ptr(),
            nb,
        )
    } != 0
    {
        return Err(Error::InvalidOutput);
    }
    let decode = |points: Vec<f64>| {
        points
            .chunks_exact(6)
            .map(|p| {
                Ok([
                    Point3::try_new(p[0], p[1], p[2])?,
                    Point3::try_new(p[3], p[4], p[5])?,
                ])
            })
            .collect::<Result<Vec<_>, GeometryError>>()
    };
    let mut result = MeshIntersections {
        transverse: decode(lines)?,
        coplanar: Vec::new(),
    };
    coplanar::append(
        first.vertices(),
        &af,
        second.vertices(),
        &bf,
        &pairs,
        tolerance,
        &mut result,
    )?;
    Ok(result)
}

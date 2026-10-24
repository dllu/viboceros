//! Open mesh partitions from owned SMLib imprints, without Boolean caps.
use super::*;

#[repr(C)]
struct NativeSplit {
    _private: [u8; 0],
}
#[repr(C)]
struct NativeCutter {
    points: *const f64,
    point_count: usize,
    faces: *const u32,
    face_count: usize,
}
unsafe extern "C" {
    fn vb_mesh_split(
        points: *const f64,
        point_count: usize,
        triangles: *const u32,
        face_count: usize,
        cutters: *const NativeCutter,
        cutter_count: usize,
        lines: *const f64,
        line_count: usize,
        tolerance: f64,
        out: *mut *mut NativeSplit,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_mesh_split_count(p: *const NativeSplit) -> usize;
    fn vb_mesh_split_take(p: *mut NativeSplit, index: usize, out: *mut *mut NativeMesh) -> c_int;
    fn vb_mesh_split_free(p: *mut NativeSplit);
}
struct OwnedSplit(NonNull<NativeSplit>);
impl Drop for OwnedSplit {
    fn drop(&mut self) {
        unsafe { vb_mesh_split_free(self.0.as_ptr()) };
    }
}

/// Partition a mesh along intersections with a collection of open or closed
/// meshes, without capping the cut. Parts are connected by uncut edges only.
/// No interaction returns an unchanged copy. A partial cut can return one imprinted part;
/// callers decide whether to retain that intermediate result. Imprinted parts
/// carry geometry without vertex channels or logical n-gons. Unchanged copies
/// retain the original geometry. Inputs remain unchanged on all exits.
pub fn mesh_split(
    target: &TriangleMesh,
    cutters: &[&TriangleMesh],
    coplanar: bool,
    tolerance: Tolerance,
) -> Result<Vec<TriangleMesh>, Error> {
    target.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
    if cutters.is_empty() {
        return Ok(vec![target.clone()]);
    }
    let (points, faces) = mesh_boolean::prepare(target)?;
    let mut prepared = Vec::with_capacity(cutters.len());
    let mut lines = Vec::new();
    for cutter in cutters {
        cutter.validate_face_geometry(Tolerance::MESH_VALIDATION)?;
        let intersections = mesh_intersections(target, cutter, tolerance)?;
        lines.extend(
            intersections
                .transverse
                .into_iter()
                .chain(intersections.tangential),
        );
        if coplanar {
            lines.extend(
                intersections
                    .coplanar
                    .iter()
                    .flat_map(|p| p.vertices().windows(2))
                    .map(|p| [p[0], p[1]]),
            );
        }
        if lines.len() > 1_000_000 {
            return Err(Error::Kernel("Mesh split constraint limit".into()));
        }
        prepared.push(mesh_boolean::prepare(cutter)?);
    }
    lines.retain(|line| line[0] != line[1]);
    if lines.is_empty() {
        return Ok(vec![target.clone()]);
    }
    for line in &mut lines {
        if line[0]
            .to_array()
            .partial_cmp(&line[1].to_array())
            .unwrap()
            .is_gt()
        {
            line.swap(0, 1);
        }
    }
    lines.sort_by(|a, b| {
        a[0].to_array()
            .partial_cmp(&b[0].to_array())
            .unwrap()
            .then_with(|| a[1].to_array().partial_cmp(&b[1].to_array()).unwrap())
    });
    lines.dedup();
    let cutting = prepared
        .iter()
        .map(|(p, f)| NativeCutter {
            points: p.as_ptr(),
            point_count: p.len() / 3,
            faces: f.as_ptr(),
            face_count: f.len() / 3,
        })
        .collect::<Vec<_>>();
    let lines = lines
        .into_iter()
        .flatten()
        .flat_map(|p| p.to_array())
        .collect::<Vec<_>>();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_mesh_split(
            points.as_ptr(),
            points.len() / 3,
            faces.as_ptr(),
            faces.len() / 3,
            cutting.as_ptr(),
            cutting.len(),
            lines.as_ptr(),
            lines.len() / 6,
            tolerance.absolute(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let owner = OwnedSplit(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
    let count = unsafe { vb_mesh_split_count(owner.0.as_ptr()) };
    if count > 1_000_000 {
        return Err(Error::InvalidOutput);
    }
    let mut parts = Vec::with_capacity(count);
    for i in 0..count {
        let mut mesh = std::ptr::null_mut();
        if unsafe { vb_mesh_split_take(owner.0.as_ptr(), i, &mut mesh) } != 0 {
            return Err(Error::InvalidOutput);
        }
        let mesh = OwnedMesh(NonNull::new(mesh).ok_or(Error::InvalidOutput)?);
        parts.push(
            mesh.decode(Tolerance::MESH_VALIDATION)?
                .combined_identical_vertices()
                .0,
        );
    }
    Ok(parts)
}

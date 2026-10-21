//! Owned SubD records and bounded calls to the public OpenNURBS subdivision API.
use std::ffi::{c_char, c_int};
use std::ptr::NonNull;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("OpenNURBS SubD: {0}")]
pub struct SubDError(pub String);

/// Immutable control net. The archive also retains crease sharpness and other
/// OpenNURBS fields that are not yet exposed as editable Rust properties.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDData {
    pub archive: Vec<u8>,
    pub points: Vec<[f64; 3]>,
    pub surface_points: Vec<[f64; 3]>,
    pub vertex_tags: Vec<u8>,
    pub edges: Vec<[u32; 2]>,
    pub edge_tags: Vec<u8>,
    pub faces: Vec<Vec<u32>>,
    /// Exact limit normals per face corner, when explicitly requested.
    pub corner_normals: Vec<[f64; 3]>,
}
#[repr(C)]
#[derive(Default)]
struct View {
    archive_count: usize,
    point_count: usize,
    edge_count: usize,
    face_count: usize,
    corner_count: usize,
    normal_count: usize,
    archive: *const u8,
    points: *const f64,
    surface_points: *const f64,
    vertex_tags: *const u8,
    edges: *const u32,
    edge_tags: *const u8,
    face_sizes: *const u32,
    face_vertices: *const u32,
    corner_normals: *const f64,
}
#[repr(C)]
struct NativeResult {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vibo_subd_box(
        corners: *const f64,
        counts: *const u32,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_create(
        points: *const f64,
        point_count: usize,
        sizes: *const u32,
        face_count: usize,
        corners: *const u32,
        corner_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_append(
        bytes: *const u8,
        count: usize,
        points: *const f64,
        point_count: usize,
        sizes: *const u32,
        face_count: usize,
        corners: *const u32,
        corner_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_set_tags(
        bytes: *const u8,
        count: usize,
        vertex: c_int,
        indices: *const usize,
        tags: *const u8,
        tag_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_read(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_edit_points(
        bytes: *const u8,
        count: usize,
        points: *const f64,
        point_count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_transform(
        bytes: *const u8,
        count: usize,
        matrix: *const f64,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_control_mesh(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_tessellate(
        bytes: *const u8,
        count: usize,
        density: u32,
        normals: c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_view(result: *const NativeResult, view: *mut View) -> c_int;
    fn vibo_subd_free(result: *mut NativeResult);
}
/// Uses the public native box constructor, including its face packing metadata.
pub fn subd_box(corners: &[[f64; 3]; 8], counts: [u32; 3]) -> Result<SubDData, SubDError> {
    call(|out, error, capacity| unsafe {
        vibo_subd_box(
            corners.as_ptr().cast(),
            counts.as_ptr(),
            out,
            error,
            capacity,
        )
    })
}
struct Owner(NonNull<NativeResult>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_subd_free(self.0.as_ptr()) }
    }
}
fn call(
    f: impl FnOnce(*mut *mut NativeResult, *mut c_char, usize) -> c_int,
) -> Result<SubDData, SubDError> {
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = f(&mut out, error.as_mut_ptr().cast(), error.len());
    let owner = NonNull::new(out).map(Owner);
    if status != 1 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing native result".into()))?;
    let mut v = View::default();
    if unsafe { vibo_subd_view(owner.0.as_ptr(), &mut v) } != 1 {
        return Err(SubDError("Missing native view".into()));
    }
    // The synchronous native result owns each buffer until Owner is dropped.
    // Zero-length std::vector::data() may be null, so never build a null slice.
    unsafe fn copy<T: Copy>(p: *const T, n: usize) -> Vec<T> {
        if n == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(p, n).to_vec() }
        }
    }
    let sizes = unsafe { copy(v.face_sizes, v.face_count) };
    let corners = unsafe { copy(v.face_vertices, v.corner_count) };
    let mut offset = 0;
    let faces = sizes
        .into_iter()
        .map(|n| {
            let end = offset + n as usize;
            let face = corners[offset..end].to_vec();
            offset = end;
            face
        })
        .collect();
    Ok(SubDData {
        archive: unsafe { copy(v.archive, v.archive_count) },
        points: unsafe { copy(v.points.cast(), v.point_count) },
        surface_points: unsafe { copy(v.surface_points.cast(), v.point_count) },
        vertex_tags: unsafe { copy(v.vertex_tags, v.point_count) },
        edges: unsafe { copy(v.edges.cast(), v.edge_count) },
        edge_tags: unsafe { copy(v.edge_tags, v.edge_count) },
        faces,
        corner_normals: unsafe { copy(v.corner_normals.cast(), v.normal_count) },
    })
}
fn check_archive(bytes: &[u8]) -> Result<(), SubDError> {
    if bytes.len() <= 16 || bytes.len() > 64 * 1024 * 1024 {
        Err(SubDError("Invalid archive size".into()))
    } else {
        Ok(())
    }
}
pub fn subd_from_polygons(points: &[[f64; 3]], faces: &[Vec<u32>]) -> Result<SubDData, SubDError> {
    if !(3..=100_000).contains(&points.len())
        || faces.is_empty()
        || faces.len() > 100_000
        || points.as_flattened().iter().any(|p| !p.is_finite())
        || faces.iter().any(|f| {
            !(3..=0xfff0).contains(&f.len()) || f.iter().any(|&v| v as usize >= points.len())
        })
        || faces.iter().map(Vec::len).sum::<usize>() > 400_000
    {
        return Err(SubDError("Invalid or excessive control net".into()));
    }
    let sizes = faces.iter().map(|f| f.len() as u32).collect::<Vec<_>>();
    let corners = faces.iter().flatten().copied().collect::<Vec<_>>();
    call(|out, error, n| unsafe {
        vibo_subd_create(
            points.as_flattened().as_ptr(),
            points.len(),
            sizes.as_ptr(),
            sizes.len(),
            corners.as_ptr(),
            corners.len(),
            out,
            error,
            n,
        )
    })
}
pub fn subd_from_archive(bytes: &[u8]) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    call(|out, error, n| unsafe { vibo_subd_read(bytes.as_ptr(), bytes.len(), out, error, n) })
}
pub fn subd_edit_points(bytes: &[u8], points: &[[f64; 3]]) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if points.len() > 100_000 || points.as_flattened().iter().any(|p| !p.is_finite()) {
        return Err(SubDError("Invalid edited points".into()));
    }
    call(|out, error, n| unsafe {
        vibo_subd_edit_points(
            bytes.as_ptr(),
            bytes.len(),
            points.as_flattened().as_ptr(),
            points.len(),
            out,
            error,
            n,
        )
    })
}
/// Samples the exact limit surface at the vertices of the refined control net.
/// The mesh between samples is an approximation, without a Hausdorff guarantee.
pub fn subd_tessellate(bytes: &[u8], density: u32) -> Result<SubDData, SubDError> {
    tessellate(bytes, density, false)
}
pub fn subd_tessellate_with_normals(bytes: &[u8], density: u32) -> Result<SubDData, SubDError> {
    tessellate(bytes, density, true)
}
fn tessellate(bytes: &[u8], density: u32, normals: bool) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    call(|out, error, n| unsafe {
        vibo_subd_tessellate(
            bytes.as_ptr(),
            bytes.len(),
            density,
            normals.into(),
            out,
            error,
            n,
        )
    })
}

/// Uses the native affine transform so SubD fields beyond vertex coordinates
/// receive the same transformation and cache invalidation as the control net.
pub fn subd_transform(bytes: &[u8], matrix: [[f64; 4]; 4]) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if matrix.as_flattened().iter().any(|x| !x.is_finite()) || matrix[3] != [0., 0., 0., 1.] {
        return Err(SubDError("Invalid affine transformation".into()));
    }
    call(|out, error, n| unsafe {
        vibo_subd_transform(
            bytes.as_ptr(),
            bytes.len(),
            matrix.as_flattened().as_ptr(),
            out,
            error,
            n,
        )
    })
}

/// Append polygons to the original record, preserving its existing components.
/// Points begin with the unchanged original control-point table. Adjacent new
/// faces are oriented against the original boundary and each other.
pub fn subd_append(
    bytes: &[u8],
    points: &[[f64; 3]],
    faces: &[Vec<u32>],
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if points.len() > 100_000
        || faces.is_empty()
        || faces.len() > 100_000
        || points.as_flattened().iter().any(|p| !p.is_finite())
        || faces.iter().any(|f| {
            !(3..=0xfff0).contains(&f.len()) || f.iter().any(|&i| i as usize >= points.len())
        })
        || faces.iter().map(Vec::len).sum::<usize>() > 400_000
    {
        return Err(SubDError("Invalid or excessive append control net".into()));
    }
    let sizes = faces.iter().map(|f| f.len() as u32).collect::<Vec<_>>();
    let corners = faces.iter().flatten().copied().collect::<Vec<_>>();
    call(|out, error, n| unsafe {
        vibo_subd_append(
            bytes.as_ptr(),
            bytes.len(),
            points.as_flattened().as_ptr(),
            points.len(),
            sizes.as_ptr(),
            sizes.len(),
            corners.as_ptr(),
            corners.len(),
            out,
            error,
            n,
        )
    })
}
/// Tags are applied through the public native setters, including their neighbor
/// updates. Vertex tags are 1..4; edge setters accept smooth (1) or crease (2).
pub fn subd_set_tags(
    bytes: &[u8],
    vertex: bool,
    values: &[(usize, u8)],
) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    if values.len() > 100_000
        || values
            .iter()
            .any(|&(i, t)| i >= 100_000 || t < 1 || t > if vertex { 4 } else { 2 })
    {
        return Err(SubDError("Invalid SubD component tags".into()));
    }
    let indices = values.iter().map(|&(i, _)| i).collect::<Vec<_>>();
    let tags = values.iter().map(|&(_, t)| t).collect::<Vec<_>>();
    call(|out, error, n| unsafe {
        vibo_subd_set_tags(
            bytes.as_ptr(),
            bytes.len(),
            vertex.into(),
            indices.as_ptr(),
            tags.as_ptr(),
            values.len(),
            out,
            error,
            n,
        )
    })
}

/// Native triangulated control cage, used only for control-boundary input.
/// This is not a limit-surface approximation.
pub fn subd_control_mesh(bytes: &[u8]) -> Result<SubDData, SubDError> {
    check_archive(bytes)?;
    call(|out, error, n| unsafe {
        vibo_subd_control_mesh(bytes.as_ptr(), bytes.len(), out, error, n)
    })
}

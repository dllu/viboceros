//! Oriented dyadic samples of each control face's limit surface.
use super::*;

/// A chart with source face winding: (0,0), (1,0), (1,1), (0,1).
/// A quad has one chart. Other polygons have one chart per corner, bounded by
/// that corner, its outgoing edge midpoint, face center, and incoming midpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDFaceGrid {
    pub face: usize,
    pub corner: Option<usize>,
    /// Number of intervals on either axis; there are `(side + 1)^2` points.
    pub side: usize,
    /// Row-major points, with the first chart coordinate varying fastest.
    pub points: Vec<[f64; 3]>,
}

#[repr(C)]
struct NativeGrids {
    _private: [u8; 0],
}
#[repr(C)]
struct GridInfo {
    face: usize,
    corner: usize,
    side: usize,
    offset: usize,
}
#[repr(C)]
#[derive(Default)]
struct GridsView {
    grid_count: usize,
    point_count: usize,
    grids: *const GridInfo,
    points: *const [f64; 3],
}
unsafe extern "C" {
    fn vibo_subd_face_grids(
        bytes: *const u8,
        count: usize,
        density: u32,
        out: *mut *mut NativeGrids,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_face_grids_view(result: *const NativeGrids, out: *mut GridsView) -> c_int;
    fn vibo_subd_face_grids_free(result: *mut NativeGrids);
}
struct GridsOwner(NonNull<NativeGrids>);
impl Drop for GridsOwner {
    fn drop(&mut self) {
        unsafe { vibo_subd_face_grids_free(self.0.as_ptr()) }
    }
}

/// Exact limit evaluations at dyadic chart coordinates. Density is 1..=6:
/// quads have `2^density` intervals, n-gon sectors have `2^(density-1)`.
/// The input archive is immutable. Linear interpolation between samples does
/// not provide a bound on the intervening surface approximation error.
pub fn subd_face_grids(bytes: &[u8], density: u32) -> Result<Vec<SubDFaceGrid>, SubDError> {
    check_archive(bytes)?;
    if !(1..=6).contains(&density) {
        return Err(SubDError("Invalid face sample density".into()));
    }
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_face_grids(
            bytes.as_ptr(),
            bytes.len(),
            density,
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(GridsOwner);
    if status != 1 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing face sample result".into()))?;
    let mut view = GridsView::default();
    if unsafe { vibo_subd_face_grids_view(owner.0.as_ptr(), &mut view) } != 1
        || view.grid_count > 1_000_000
        || view.point_count > 4_000_000
        || (view.grid_count > 0 && view.grids.is_null())
        || (view.point_count > 0 && view.points.is_null())
    {
        return Err(SubDError("Invalid face sample view".into()));
    }
    let grids = if view.grid_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(view.grids, view.grid_count) }
    };
    let points = if view.point_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(view.points, view.point_count) }
    };
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(SubDError("Nonfinite face sample".into()));
    }
    let mut offset = 0;
    let mut result = Vec::with_capacity(grids.len());
    for grid in grids {
        let corner = (grid.corner != usize::MAX).then_some(grid.corner);
        let side = 1usize << (density - u32::from(corner.is_some()));
        let count = (side + 1).pow(2);
        if grid.side != side
            || grid.offset != offset
            || offset > points.len()
            || count > points.len() - offset
        {
            return Err(SubDError("Invalid face sample layout".into()));
        }
        result.push(SubDFaceGrid {
            face: grid.face,
            corner,
            side,
            points: points[offset..offset + count].to_vec(),
        });
        offset += count;
    }
    if offset != points.len() {
        return Err(SubDError("Trailing face samples".into()));
    }
    Ok(result)
}

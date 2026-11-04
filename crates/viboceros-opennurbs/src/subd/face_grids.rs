//! Oriented dyadic face charts for limit samples and refined control nets.
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
    /// Source chart corners tagged Crease with two adjacent hard crease sides.
    pub crease_corners: [bool; 4],
}

/// Refined source-face controls with four clockwise rows just outside the
/// chart, and exact limit positions at its corners. Crease halos reflect the
/// interior row; smooth halos follow shared native topology.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDPatchStencil {
    pub face: usize,
    pub corner: Option<usize>,
    pub side: usize,
    pub controls: Vec<[f64; 3]>,
    pub halo: [Vec<[f64; 3]>; 4],
    pub limit_corners: [[f64; 3]; 4],
    pub crease_corners: [bool; 4],
}

/// A refinement level that may still contain sharp source-side intervals.
/// Unresolved corners cannot be used for a smooth bicubic approximation.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDPartialPatchStencil {
    pub stencil: SubDPatchStencil,
    pub unresolved_corners: [bool; 4],
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
    crease_corners: usize,
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
        kind: c_int,
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
    read_grids(bytes, density, 1).map(|g| g.into_iter().map(|(g, _)| g).collect())
}

/// Refined control points in the same charts as `subd_face_grids`, after
/// `density` global subdivision steps. These are control vertices, not surface
/// evaluations; density, immutability and resource limits match that query.
pub fn subd_refined_face_grids(bytes: &[u8], density: u32) -> Result<Vec<SubDFaceGrid>, SubDError> {
    read_grids(bytes, density, 0).map(|g| g.into_iter().map(|(g, _)| g).collect())
}

/// Bounded bicubic construction stencils, using the same source charts as the
/// limit query. Outer diagonal controls are intentionally left to the fitter.
pub fn subd_patch_stencils(bytes: &[u8], density: u32) -> Result<Vec<SubDPatchStencil>, SubDError> {
    patch_stencils(bytes, density, false).map(|g| g.into_iter().map(|g| g.stencil).collect())
}

/// Partial control stencils for local corrections across refinement levels.
/// A full surface must not be inferred where soft sharpness is unresolved;
/// callers must restrict use to resolved neighborhoods. Sources stay immutable.
pub fn subd_partial_patch_stencils(
    bytes: &[u8],
    density: u32,
) -> Result<Vec<SubDPartialPatchStencil>, SubDError> {
    patch_stencils(bytes, density, true)
}

fn patch_stencils(
    bytes: &[u8],
    density: u32,
    partial: bool,
) -> Result<Vec<SubDPartialPatchStencil>, SubDError> {
    read_grids(bytes, density, if partial { 3 } else { 2 })?
        .into_iter()
        .map(|(grid, unresolved_corners)| {
            let stride = grid.side + 1;
            let inner = stride * stride;
            Ok(SubDPartialPatchStencil {
                unresolved_corners,
                stencil: SubDPatchStencil {
                    face: grid.face,
                    corner: grid.corner,
                    side: grid.side,
                    crease_corners: grid.crease_corners,
                    controls: grid.points[..inner].to_vec(),
                    halo: std::array::from_fn(|side| {
                        grid.points[inner + side * stride..inner + (side + 1) * stride].to_vec()
                    }),
                    limit_corners: grid.points[inner + 4 * stride..]
                        .try_into()
                        .map_err(|_| SubDError("Invalid stencil corner count".into()))?,
                },
            })
        })
        .collect()
}

fn read_grids(
    bytes: &[u8],
    density: u32,
    kind: c_int,
) -> Result<Vec<(SubDFaceGrid, [bool; 4])>, SubDError> {
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
            kind,
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
        let count = (side + if kind >= 2 { 3 } else { 1 }).pow(2);
        if grid.side != side
            || grid.crease_corners > if kind == 3 { 255 } else { 15 }
            || grid.offset != offset
            || offset > points.len()
            || count > points.len() - offset
        {
            return Err(SubDError("Invalid face sample layout".into()));
        }
        result.push((
            SubDFaceGrid {
                face: grid.face,
                corner,
                side,
                points: points[offset..offset + count].to_vec(),
                crease_corners: std::array::from_fn(|i| grid.crease_corners & (1 << i) != 0),
            },
            std::array::from_fn(|i| grid.crease_corners & (1 << (i + 4)) != 0),
        ));
        offset += count;
    }
    if offset != points.len() {
        return Err(SubDError("Trailing face samples".into()));
    }
    Ok(result)
}

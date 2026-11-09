//! Surface-edge matching from independently evaluated boundary jets.
use crate::{
    CurveMatchPreserveEnd, GeometryError, NurbsInterpolationSystem, NurbsSurface, Real,
    SurfaceBlendContinuity, SurfaceExtensionEdge, Tolerance,
};
mod average;
mod fit;
pub use average::{
    SurfaceAverageOptions, try_average_match_nurbs_surfaces,
    try_average_match_nurbs_surfaces_with_operations,
};
mod jets;
mod orientation;
mod prepare;
mod refine;
use orientation::Chart;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceMatchIso {
    Automatic,
    MatchTarget,
    Perpendicular,
    Preserve,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceMatchOptions {
    pub continuity: SurfaceBlendContinuity,
    pub other_end: CurveMatchPreserveEnd,
    pub iso: SurfaceMatchIso,
    /// Toggle the target boundary traversal relative to its natural edge.
    pub reverse_match: bool,
    pub closest_points: bool,
    /// Use the measured four-step refinement at the supplied model tolerances.
    /// Reaching the iteration limit can return a match outside those tolerances.
    pub refine: bool,
}
impl Default for SurfaceMatchOptions {
    fn default() -> Self {
        Self {
            continuity: SurfaceBlendContinuity::Tangency,
            other_end: CurveMatchPreserveEnd::None,
            iso: SurfaceMatchIso::Automatic,
            reverse_match: false,
            closest_points: false,
            refine: false,
        }
    }
}

type Clamp<'a, E> = dyn FnMut(&NurbsSurface) -> Result<NurbsSurface, E> + 'a;
type Elevate<'a, E> = dyn FnMut(&NurbsSurface, [usize; 2]) -> Result<NurbsSurface, E> + 'a;
type Refine<'a, E> = dyn FnMut(&NurbsSurface, Real) -> Result<NurbsSurface, E> + 'a;
type Solve<'a, E> = dyn FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E> + 'a;
/// Backends share geometric policy; clamping, degree changes, knot insertion
/// and the collocation solve are delegated.
pub struct SurfaceMatchOperations<'a, E> {
    pub clamp: &'a mut Clamp<'a, E>,
    pub elevate: &'a mut Elevate<'a, E>,
    pub interpolate: &'a mut Solve<'a, E>,
    pub refine: &'a mut Refine<'a, E>,
}

/// One-sided natural-boundary matching. The result retains the source clocks
/// except for degree preparation and optional refinement. See `SurfaceMatchOptions`
/// for traversal and the bounded refinement policy.
pub fn try_match_nurbs_surfaces(
    source: &NurbsSurface,
    side: SurfaceExtensionEdge,
    target: &NurbsSurface,
    target_side: SurfaceExtensionEdge,
    options: SurfaceMatchOptions,
    tolerance: Tolerance,
) -> Result<NurbsSurface, GeometryError> {
    try_match_nurbs_surfaces_with_operations(
        source,
        side,
        target,
        target_side,
        options,
        tolerance,
        &mut SurfaceMatchOperations {
            clamp: &mut NurbsSurface::try_clamped_to_active_domain,
            elevate: &mut |s, d| s.try_change_degree(d[0], d[1], false),
            interpolate: &mut crate::surface_rebuild::portable_solve,
            refine: &mut |s, t| s.try_insert_knot_v(t, 1),
        },
    )
}

pub fn try_match_nurbs_surfaces_with_operations<E: From<GeometryError>>(
    source: &NurbsSurface,
    side: SurfaceExtensionEdge,
    target: &NurbsSurface,
    target_side: SurfaceExtensionEdge,
    options: SurfaceMatchOptions,
    tolerance: Tolerance,
    operations: &mut SurfaceMatchOperations<'_, E>,
) -> Result<NurbsSurface, E> {
    let prepared = prepare::operands(source, side, target, target_side, options, operations)?;
    let source = prepared.source;
    let target = prepared.target;
    let reverse = prepared.reverse;
    let chart = prepared.chart;
    let result = refine::run(&source, &target, reverse, options, tolerance, operations)?;
    Ok(chart.restore(&result)?)
}

fn error(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceMatch { context }
}

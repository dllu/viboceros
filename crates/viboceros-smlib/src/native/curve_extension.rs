//! Exact natural extrapolation through the native parameter extension producer.
use super::*;
use std::ops::RangeInclusive;

pub fn extend_curve(
    source: &NurbsCurve,
    interval: RangeInclusive<f64>,
) -> Result<NurbsCurve, Error> {
    let (start, end) = (*interval.start(), *interval.end());
    let domain = source.domain();
    if !start.is_finite()
        || !end.is_finite()
        || start >= end
        || (start >= *domain.start() && end <= *domain.end())
    {
        return Err(GeometryError::InvalidCurveExtensionInterval.into());
    }
    if source.is_closed()? {
        return Err(GeometryError::CurveExtensionMustBeOpen.into());
    }
    if source.degree() > 17 {
        return Ok(source.try_extended_to(interval)?);
    }
    extend_curve_controls(source, start.min(*domain.start())..=end.max(*domain.end()))
}

/// Preserve the shared length solve while using native extrapolation and trim.
pub fn extend_curve_by_length(
    source: &NurbsCurve,
    side: viboceros_geometry::CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    if source.degree() > 17 {
        return Ok(source.try_extended_by_length(side, length, tolerance)?);
    }
    source.try_extended_by_length_with_operations(
        side,
        length,
        tolerance,
        &mut extend_curve,
        &mut super::knot_refinement::trim_curve,
    )
}

pub fn extend_curve_naturally_by_length(
    source: &NurbsCurve,
    side: viboceros_geometry::CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    source.try_merged_naturally_by_length_with_extender(
        side,
        length,
        tolerance,
        &mut extend_curve_by_length,
    )
}

pub fn join_curve_smooth_extension(
    source: &NurbsCurve,
    side: viboceros_geometry::CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    if source.degree() > 17 {
        return Ok(source.try_joined_smoothly_by_length(side, length, tolerance)?);
    }
    source.try_joined_smoothly_by_length_with_operations(
        side,
        length,
        tolerance,
        &mut extend_curve,
        &mut trim_curve,
    )
}
pub fn join_curve_natural_extension(
    source: &NurbsCurve,
    side: viboceros_geometry::CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
) -> Result<NurbsCurve, Error> {
    source.try_joined_naturally_by_length_with_extender(
        side,
        length,
        tolerance,
        &mut join_curve_smooth_extension,
    )
}
pub fn separate_curve_smooth_extensions(
    source: &NurbsCurve,
    side: viboceros_geometry::CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, Error> {
    if source.degree() > 17 {
        return Ok(source.try_separate_smooth_extensions_by_length(side, length, tolerance)?);
    }
    source.try_separate_smooth_extensions_by_length_with_operations(
        side,
        length,
        tolerance,
        &mut extend_curve,
        &mut trim_curve,
    )
}
pub fn separate_curve_natural_extensions(
    source: &NurbsCurve,
    side: viboceros_geometry::CurveExtensionSide,
    length: f64,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, Error> {
    source.try_separate_natural_extensions_by_length_with_extender(
        side,
        length,
        tolerance,
        &mut separate_curve_smooth_extensions,
    )
}

pub(super) fn extend_curve_controls(
    source: &NurbsCurve,
    interval: RangeInclusive<f64>,
) -> Result<NurbsCurve, Error> {
    let domain = source.domain();
    let interval = (*interval.start()).min(*domain.start())..=(*interval.end()).max(*domain.end());
    let input = super::knot_refinement::NurbsInput::curve(source);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_extend_curve(
            &view,
            *interval.start(),
            *interval.end(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    brep::curve_from_native(pointer)
}

/// Extend one surface direction through its native complete control curves.
pub fn extend_surface(
    source: &NurbsSurface,
    direction: viboceros_geometry::SurfaceKnotDirection,
    interval: RangeInclusive<f64>,
) -> Result<NurbsSurface, Error> {
    use viboceros_geometry::SurfaceKnotDirection as D;
    match direction {
        D::U if source.degree_u() > 17 => Ok(source.try_extended_u(interval)?),
        D::V if source.degree_v() > 17 => Ok(source.try_extended_v(interval)?),
        D::U => source.try_extended_u_with_extender(interval, &mut extend_curve_controls),
        D::V => source.try_extended_v_with_extender(interval, &mut extend_curve_controls),
        D::Both => Err(GeometryError::InvalidSurfaceExtensionInterval.into()),
    }
}

/// Retain the shared Rhino RMS-speed convention for model-distance extension.
pub fn extend_surface_by_length(
    source: &NurbsSurface,
    edge: viboceros_geometry::SurfaceExtensionEdge,
    length: f64,
) -> Result<NurbsSurface, Error> {
    use viboceros_geometry::{SurfaceExtensionEdge as E, SurfaceKnotDirection as D};
    let interval = source.extension_interval_by_length(edge, length)?;
    let direction = match edge {
        E::West | E::East => D::U,
        E::South | E::North => D::V,
    };
    extend_surface(source, direction, interval)
}

unsafe extern "C" {
    fn vb_nurbs_extend_curve(
        source: *const brep::NurbsData,
        start: f64,
        end: f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

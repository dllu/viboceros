//! Command-level section orientation and crease topology, separate from the
//! ordered tensor loft API.

use super::*;
use std::borrow::Cow;
mod ordering;
mod orientation;
mod profiles;
use crate::{LoftEndPoints, LoftStyle, MAX_LOFT_SECTION_CONTROLS, try_loft_nurbs_curves};
#[cfg(test)]
use ordering::sort_profiles as sort_parallel_profiles;

/// Command policies layered over ordered tensor Loft construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoftOptions {
    /// Split G1 tangent breaks using the command's global preference.
    pub crease_splitting: bool,
    /// Also split G2 curvature breaks, independently of crease splitting.
    pub split_at_tangents: bool,
    /// Retain profile seams while still aligning their traversal directions.
    pub natural_seams: bool,
}
impl Default for LoftOptions {
    fn default() -> Self {
        Self {
            crease_splitting: true,
            split_at_tangents: false,
            natural_seams: false,
        }
    }
}

/// Source curves for a Loft. An interactive editor can supply sections whose
/// order, seams and traversal have already been accepted by the user.
#[derive(Clone, Copy, Debug)]
pub enum LoftProfiles<'a> {
    Automatic(&'a [NurbsCurve]),
    Aligned {
        curves: &'a [NurbsCurve],
        oriented: bool,
    },
}

impl<'a> LoftProfiles<'a> {
    pub fn curves(self) -> &'a [NurbsCurve] {
        match self {
            Self::Automatic(curves) | Self::Aligned { curves, .. } => curves,
        }
    }

    fn prepare(
        self,
        options: LoftOptions,
        tolerance: Tolerance,
    ) -> Result<(Cow<'a, [NurbsCurve]>, bool), GeometryError> {
        match self {
            Self::Automatic(curves) => {
                let (curves, oriented) = profiles::prepare(curves, options, tolerance)?;
                Ok((Cow::Owned(curves), oriented))
            }
            Self::Aligned { curves, oriented } => {
                profiles::validate_profile_controls(curves)?;
                Ok((Cow::Borrowed(curves), oriented))
            }
        }
    }
}

impl Brep {
    /// Loft with command-style ordering, traversal alignment and crease splits.
    /// Parallel planar profiles sort by plane offset; open profiles use the
    /// supporting-plane convention, with a midpoint-chain fallback for lines,
    /// coincident planes and spatial sections. Open traversal minimizes endpoint
    /// pairing distance. Closed profiles align seam tangents and nearest knots;
    /// nonparallel or nonplanar closed sections retain input order.
    pub fn try_loft(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_loft_with_surface(curves, style, closed, tolerance, &mut try_loft_nurbs_curves)
    }

    /// Keep section ordering, orientation and crease topology while delegating
    /// the ordered tensor surface to the caller's construction backend.
    pub fn try_loft_with_surface<E: From<GeometryError>>(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, bool) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        Self::try_loft_with_surface_options(
            curves,
            style,
            closed,
            LoftOptions::default(),
            tolerance,
            loft,
        )
    }

    /// Apply explicit topology and seam policies with a delegated tensor backend.
    pub fn try_loft_with_surface_options<E: From<GeometryError>>(
        curves: &[NurbsCurve],
        style: LoftStyle,
        closed: bool,
        options: LoftOptions,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, bool) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        Self::try_loft_from_profiles(
            LoftProfiles::Automatic(curves),
            style,
            closed,
            options,
            tolerance,
            loft,
        )
    }

    /// Reuse the command's topology and orientation policy for staged profiles.
    pub fn try_loft_from_profiles<E: From<GeometryError>>(
        profiles: LoftProfiles<'_>,
        style: LoftStyle,
        closed: bool,
        options: LoftOptions,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, bool) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        crate::loft::validate_count(profiles.curves().len(), closed)?;
        let (profiles, oriented) = profiles.prepare(options, tolerance)?;
        let surface = loft(&profiles, style, closed)?;
        Ok(Self::from_loft_surface(
            surface, style, closed, options, tolerance, oriented,
        )?)
    }

    /// Open Loft optionally tapered to either point. A single real profile is
    /// sufficient when a point end is supplied. Singular boundaries are retained
    /// as topological poles, sharing the ordinary seam and split policies.
    pub fn try_loft_with_end_points(
        curves: &[NurbsCurve],
        style: LoftStyle,
        ends: LoftEndPoints,
        options: LoftOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_loft_with_surface_end_points(
            curves,
            style,
            ends,
            options,
            tolerance,
            &mut crate::try_loft_nurbs_curves_with_end_points,
        )
    }

    /// Delegate the point-ended tensor to the selected geometry backend while
    /// sharing command profile preparation and exact pole/crease topology.
    pub fn try_loft_with_surface_end_points<E: From<GeometryError>>(
        curves: &[NurbsCurve],
        style: LoftStyle,
        ends: LoftEndPoints,
        options: LoftOptions,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, LoftEndPoints) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        Self::try_loft_from_profiles_and_points(
            LoftProfiles::Automatic(curves),
            style,
            ends,
            options,
            tolerance,
            loft,
        )
    }

    /// Reuse the same topology when the staged Loft includes point ends.
    pub fn try_loft_from_profiles_and_points<E: From<GeometryError>>(
        profiles: LoftProfiles<'_>,
        style: LoftStyle,
        ends: LoftEndPoints,
        options: LoftOptions,
        tolerance: Tolerance,
        loft: &mut impl FnMut(&[NurbsCurve], LoftStyle, LoftEndPoints) -> Result<NurbsSurface, E>,
    ) -> Result<Self, E> {
        crate::loft::validate_end_points(profiles.curves().len(), style, ends)?;
        let (profiles, oriented) = profiles.prepare(options, tolerance)?;
        let surface = loft(&profiles, style, ends)?;
        Ok(Self::from_loft_surface(
            surface, style, false, options, tolerance, oriented,
        )?)
    }

    /// Map the command's automatic section order back to the source objects.
    pub fn loft_profile_order(
        curves: &[NurbsCurve],
        tolerance: Tolerance,
    ) -> Result<(Vec<usize>, bool), GeometryError> {
        profiles::validate_profile_controls(curves)?;
        ordering::profile_order(curves, tolerance)
    }

    /// Align open profiles before exposing their traversal to an interactive editor.
    pub fn align_loft_open_profiles(
        curves: &mut [NurbsCurve],
        fixed: &[bool],
    ) -> Result<(), GeometryError> {
        profiles::align_open_profiles(curves, fixed)
    }

    /// Align closed profiles to neighboring seam knots in the supplied order.
    pub fn align_closed_profile_seams(curves: &mut [NurbsCurve]) -> Result<(), GeometryError> {
        profiles::align_closed_profiles(curves, true)
    }

    fn from_loft_surface(
        mut surface: NurbsSurface,
        style: LoftStyle,
        closed: bool,
        options: LoftOptions,
        tolerance: Tolerance,
        oriented: bool,
    ) -> Result<Self, GeometryError> {
        let linear_profiles = surface.degree_v() == 1;
        // Retain the established open-loft chart policy. Native Straight
        // direction exceptions remain unresolved; see docs/loft-validation.md.
        if oriented && !closed && !(style == LoftStyle::Straight && linear_profiles) {
            surface = surface.try_reversed_v()?;
        }
        let mut criteria = Vec::new();
        if options.crease_splitting {
            criteria.push((crate::CurveContinuity::G1, 0.1_f64.to_radians()));
        }
        if options.split_at_tangents {
            criteria.push((crate::CurveContinuity::G2, 1_f64.to_radians()));
        }
        let mut brep =
            Self::try_surface_partitioned_at_discontinuities(&surface, &criteria, tolerance)?;
        if brep.is_solid() && orientation::all_outside_probes_point_inward(&surface, tolerance) {
            brep.reverse_orientation();
        }
        Ok(brep)
    }
}

#[cfg(test)]
mod tests;

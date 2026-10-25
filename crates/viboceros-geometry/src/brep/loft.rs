//! Command-level section orientation and crease topology, separate from the
//! ordered tensor loft API.

use super::*;
mod orientation;
mod profiles;
use crate::{LoftEndPoints, LoftStyle, MAX_LOFT_SECTION_CONTROLS, try_loft_nurbs_curves};
#[cfg(test)]
use profiles::sort_parallel_profiles;

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

impl Brep {
    /// Loft with command-style crease splitting and automatic ordering for
    /// parallel planar closed profiles. Their plane offsets are sorted along
    /// the first profile's oriented area normal. Closed profiles align traversal
    /// with their preceding seam tangent and move seams to the nearest knot.
    /// Other profiles retain input order; general spatial matching is incomplete.
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
        crate::loft::validate_count(curves.len(), closed)?;
        let (profiles, oriented) = profiles::prepare(curves, options, tolerance)?;
        let surface = loft(&profiles, style, closed)?;
        Ok(Self::from_loft_surface(
            surface,
            style,
            closed,
            options,
            tolerance,
            oriented,
            profiles.iter().all(|c| c.degree() == 1),
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
        crate::loft::validate_end_points(curves.len(), style, ends)?;
        let (profiles, oriented) = profiles::prepare(curves, options, tolerance)?;
        let surface = loft(&profiles, style, ends)?;
        Ok(Self::from_loft_surface(
            surface,
            style,
            false,
            options,
            tolerance,
            oriented,
            profiles.iter().all(|c| c.degree() == 1),
        )?)
    }

    fn from_loft_surface(
        mut surface: NurbsSurface,
        style: LoftStyle,
        closed: bool,
        options: LoftOptions,
        tolerance: Tolerance,
        oriented: bool,
        linear_profiles: bool,
    ) -> Result<Self, GeometryError> {
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

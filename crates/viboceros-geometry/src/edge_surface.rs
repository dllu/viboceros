//! Homogeneous Coons patches with independently oriented boundary curves.

use crate::{
    Brep, GeometryError, NurbsCurve, NurbsSurface, Point3, Real, Tolerance, WeightedPoint3,
};

mod arrange;
mod basis;
mod coons;
#[cfg(test)]
mod tests;

const MAX_EDGE_CONTROLS: usize = 512;

impl Brep {
    /// Constructs one Coons face from two, three, or four open curves. Two
    /// curves form a ruled patch; three form a triangle with a collapsed side.
    /// Three/four-curve endpoint gaps are closed by homogeneous affine corner
    /// corrections, so disconnected input boundaries may be displaced.
    pub fn try_edge_surface(
        curves: &[NurbsCurve],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_surface_face(NurbsSurface::try_edge_curves(curves, tolerance)?, tolerance)
    }
}

impl NurbsSurface {
    /// Tensor surface underlying [`Brep::try_edge_surface`]. Boundary sorting,
    /// endpoint-weight normalization, and corner correction precede blending.
    pub fn try_edge_curves(
        curves: &[NurbsCurve],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !(2..=4).contains(&curves.len()) {
            return Err(GeometryError::InvalidEdgeSurfaceBoundaries);
        }
        for curve in curves {
            basis::check_count(curve, curve.degree())?;
            if curve.is_closed()?
                || curve
                    .interior_knot_groups()
                    .iter()
                    .any(|(_, m)| *m > curve.degree())
            {
                return Err(GeometryError::InvalidEdgeSurfaceBoundaries);
            }
        }
        let curves = arrange::curves(curves, tolerance)?;
        if curves.len() == 2 {
            let [a, b] = basis::compatible(&curves[0], &curves[1])?;
            let controls = a
                .control_points()
                .iter()
                .zip(b.control_points())
                .flat_map(|(a, b)| [*a, *b])
                .collect();
            return Self::try_new_rational(
                1,
                a.degree(),
                2,
                a.control_points().len(),
                controls,
                vec![0.0, 0.0, 1.0, 1.0],
                a.knots().to_vec(),
            );
        }
        let mut curves = curves
            .iter()
            .map(basis::normalized)
            .collect::<Result<Vec<_>, _>>()?;
        arrange::close_corners(&mut curves)?;
        if curves.len() == 3 {
            let [west, east] = basis::compatible(&curves[2], &curves[1].reversed()?)?;
            let north = &curves[0];
            let apex = west.control_points()[0].point();
            let south = NurbsCurve::try_new_rational(
                north.degree(),
                north
                    .control_points()
                    .iter()
                    .map(|c| WeightedPoint3::try_new(apex, c.weight()))
                    .collect::<Result<Vec<_>, _>>()?,
                north.knots().to_vec(),
            )?;
            coons::surface([&south, north], [&west, &east])
        } else {
            let [west, east] = basis::compatible(&curves[0], &curves[2].reversed()?)?;
            let [north, south] = basis::compatible(&curves[1], &curves[3].reversed()?)?;
            coons::surface([&south, &north], [&west, &east])
        }
    }
}

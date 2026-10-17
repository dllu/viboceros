//! First-harmonic interpolation in the existing uniform periodic B-spline basis.
use crate::{Circle3, GeometryError, NurbsCurve, Real};
use std::f64::consts::TAU;

/// Smallest unique control count in Circle's periodic approximation branch.
/// Smaller native layouts use a different nonperiodic approximation.
pub fn circle_deformable_minimum_points(degree: usize) -> Result<usize, GeometryError> {
    if !(2..=crate::MAX_CURVE_REBUILD_DEGREE).contains(&degree) {
        return Err(GeometryError::InvalidCircleDeformDegree { actual: degree });
    }
    // Measured public Circle getter layouts. At higher degrees Rhino switches
    // to a clamped approximation below these limits, despite accepting the
    // integer setting. Keep that branch out of the periodic construction.
    Ok([4, 4, 5, 6, 7, 9, 10, 12, 13, 15][degree - 2])
}

impl Circle3 {
    /// Non-rational periodic approximation through equal-angle stations.
    /// `point_count` counts unique controls, excluding the `degree` seam aliases.
    /// The domain is angular, independent of the analytic circle's own domain.
    pub fn try_deformable(
        self,
        degree: usize,
        point_count: usize,
    ) -> Result<NurbsCurve, GeometryError> {
        let minimum = circle_deformable_minimum_points(degree)?;
        if !(minimum..=crate::MAX_CURVE_REBUILD_POINT_COUNT).contains(&point_count) {
            return Err(GeometryError::InvalidCircleDeformPointCount {
                actual: point_count,
                minimum,
                maximum: crate::MAX_CURVE_REBUILD_POINT_COUNT,
            });
        }
        let knots = crate::curve_rebuild::periodic_uniform_knots(degree, point_count);
        // Circle interpolates seam stations for every degree; Rebuild uses
        // half-span Greville stations for even degrees.
        let offset = 0.0;
        let basis =
            crate::nurbs::bspline_basis_values(&knots, degree, point_count + degree, offset)?;
        let step = TAU / point_count as Real;
        let phase = (degree - 1) as Real * 0.5;
        // Uniform periodic collocation is circulant. A circle contains only
        // the first Fourier harmonic, so its solve is a single real gain.
        // Evaluate it in the shared basis; no dense solve or fitted geometry.
        let gain = basis
            .iter()
            .enumerate()
            .map(|(i, weight)| weight * (step * (i as Real - phase - offset)).cos())
            .sum::<Real>();
        if !gain.is_finite() || gain <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "deformable circle collocation",
            });
        }
        let center = self.center();
        let controls = (0..point_count)
            .map(|i| {
                let (sin, cos) = (step * (i as Real - phase)).sin_cos();
                crate::circular::frame_point(
                    center,
                    self.x_axis(),
                    self.y_axis(),
                    self.radius() / gain,
                    cos,
                    sin,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut periodic = controls.clone();
        periodic.extend_from_slice(&controls[..degree]);
        NurbsCurve::try_new(degree, periodic, knots)?.try_reparameterized(0.0..=TAU)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurveRef, Frame3, Point3, Tolerance, try_rebuild_curve};

    #[test]
    fn circle_deformable_odd_degrees_match_general_periodic_interpolation() {
        let tolerance = Tolerance::default();
        let plane = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            crate::Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::default(),
        )
        .unwrap();
        let circle = Circle3::try_from_frame(
            Point3::try_new(4., -7., 9.).unwrap(),
            3.,
            plane.x_axis(),
            plane.z_axis(),
            tolerance,
        )
        .unwrap();
        for degree in (3..=11).step_by(2) {
            for count in [circle_deformable_minimum_points(degree).unwrap(), 16, 35] {
                let actual = circle.try_deformable(degree, count).unwrap();
                let expected =
                    try_rebuild_curve(CurveRef::Circle(&circle), count, degree, false, tolerance)
                        .unwrap()
                        .try_reparameterized(0.0..=TAU)
                        .unwrap();
                assert_eq!(actual.knots(), expected.knots());
                for (a, b) in actual
                    .control_points()
                    .iter()
                    .zip(expected.control_points())
                {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-10,
                        "degree {degree}, count {count}: {a:?} != {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn circle_deformable_dense_stations_and_bounds() {
        let c = Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            3.,
            Frame3::try_from_normal(
                Point3::try_new(0., 0., 0.).unwrap(),
                crate::Vector3::try_new(0., 0., 1.).unwrap(),
                Tolerance::default(),
            )
            .unwrap()
            .z_axis(),
            Tolerance::default(),
        )
        .unwrap();
        for degree in [2, 5, 11] {
            let curve = c.try_deformable(degree, 1000).unwrap();
            for j in 0..1000 {
                let t = TAU * j as Real / 1000.;
                assert!(
                    curve
                        .evaluate(t)
                        .unwrap()
                        .distance_to(c.point_at_angle(t).unwrap())
                        .unwrap()
                        < 1e-12
                );
            }
        }
        assert!(c.try_deformable(1, 10).is_err());
        assert!(c.try_deformable(12, 16).is_err());
        assert!(c.try_deformable(5, 5).is_err());
        assert!(c.try_deformable(5, 1001).is_err());
    }
}

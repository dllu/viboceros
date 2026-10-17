//! Circle approximation using the shared periodic and clamped B-spline bases.
use crate::{Circle3, GeometryError, NurbsCurve, Point3, Real};
use std::f64::consts::TAU;

/// Smallest unique control count in Circle's periodic approximation branch.
/// Smaller layouts use clamped Greville interpolation instead.
pub fn circle_deformable_minimum_points(degree: usize) -> Result<usize, GeometryError> {
    if !(2..=crate::MAX_CURVE_REBUILD_DEGREE).contains(&degree) {
        return Err(GeometryError::InvalidCircleDeformDegree { actual: degree });
    }
    // Measured public Circle getter layouts. At higher degrees Rhino switches
    // to a clamped approximation below these limits, despite accepting the
    // integer setting.
    Ok([4, 4, 5, 6, 7, 9, 10, 12, 13, 15][degree - 2])
}

impl Circle3 {
    /// Non-rational approximation through equal-angle stations. Periodic
    /// layouts have `point_count + degree` controls; low counts have
    /// `max(point_count, degree) + 1` clamped controls. The angular domain is
    /// independent of the analytic circle's own domain.
    pub fn try_deformable(
        self,
        degree: usize,
        point_count: usize,
    ) -> Result<NurbsCurve, GeometryError> {
        let minimum = circle_deformable_minimum_points(degree)?;
        if !(4..=crate::MAX_CURVE_REBUILD_POINT_COUNT).contains(&point_count) {
            return Err(GeometryError::InvalidCircleDeformPointCount {
                actual: point_count,
                minimum: 4,
                maximum: crate::MAX_CURVE_REBUILD_POINT_COUNT,
            });
        }
        if point_count < minimum {
            return self.try_clamped_deformable(degree, point_count);
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

    fn try_clamped_deformable(
        self,
        degree: usize,
        point_count: usize,
    ) -> Result<NurbsCurve, GeometryError> {
        let interpolation_degree = degree.min(point_count);
        // Solve in the unit circle's local coordinates. Translation and large
        // world coordinates must not contaminate the interpolation targets.
        let targets = (0..=point_count)
            .map(|i| {
                let (sin, cos) = (TAU * i as Real / point_count as Real).sin_cos();
                Point3::try_new(cos, sin, 0.)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unit = crate::curve_rebuild::interpolate_clamped_greville_targets(
            interpolation_degree,
            &targets,
        )?;
        let unit = unit.try_change_degree(degree, false)?;
        let controls = unit
            .control_points()
            .iter()
            .map(|control| {
                let p = control.point();
                crate::circular::frame_point(
                    self.center(),
                    self.x_axis(),
                    self.y_axis(),
                    self.radius(),
                    p.x(),
                    p.y(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        NurbsCurve::try_new(degree, controls, unit.knots().to_vec())?.try_reparameterized(0.0..=TAU)
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
        assert!(c.try_deformable(5, 3).is_err());
        assert!(c.try_deformable(5, 1001).is_err());
    }

    #[test]
    fn circle_deformable_clamped_stations_preserve_spatial_frames_and_elevated_parameters() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(4., -7., 9.).unwrap(),
            crate::Vector3::try_new(2., -3., 4.).unwrap(),
            Tolerance::default(),
        )
        .unwrap();
        let circle = Circle3::try_from_frame(
            frame.origin(),
            3.,
            frame.x_axis(),
            frame.z_axis(),
            Tolerance::default(),
        )
        .unwrap();
        let mut layouts = 0;
        for degree in 2..=11 {
            for count in 4..circle_deformable_minimum_points(degree).unwrap() {
                let curve = circle.try_deformable(degree, count).unwrap();
                assert_eq!(curve.degree(), degree);
                assert!(!curve.is_periodic());
                assert_eq!(curve.control_points().len(), count.max(degree) + 1);
                assert!(curve.control_points().iter().all(|p| p.weight() == 1.));
                for i in 0..=count {
                    // Elevation retains the lower-degree Bezier parameters.
                    let parameter = if degree > count {
                        TAU * i as Real / count as Real
                    } else {
                        curve.knots()[i + 1..=i + degree].iter().sum::<Real>() / degree as Real
                    };
                    let expected = circle
                        .point_at_angle(TAU * i as Real / count as Real)
                        .unwrap();
                    assert!(
                        curve
                            .evaluate(parameter)
                            .unwrap()
                            .distance_to(expected)
                            .unwrap()
                            < 1e-10,
                        "degree {degree}, count {count}, station {i}"
                    );
                }
                layouts += 1;
            }
        }
        assert_eq!(layouts, 45);
    }
}

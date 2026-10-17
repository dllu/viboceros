//! Arc approximations reuse Rebuild's clamped solve and Circle's closed layouts.
use crate::{
    Circle3, CircularArc3, CurveRef, Frame3, GeometryError, MAX_CURVE_REBUILD_DEGREE,
    MAX_CURVE_REBUILD_POINT_COUNT, NurbsCurve, Point3, Tolerance, Vector3, try_rebuild_curve,
};
impl CircularArc3 {
    /// Non-rational uniform approximation with projected endpoint handles.
    /// Open arcs retain their native domain and requested control count; a
    /// full circle uses Circle's angular domain and periodic/clamped layouts.
    pub fn try_deformable(
        self,
        degree: usize,
        point_count: usize,
    ) -> Result<NurbsCurve, GeometryError> {
        self.try_deformable_with_rebuild(degree, point_count, &mut |unit, p, n| {
            try_rebuild_curve(
                CurveRef::Arc(&unit),
                n,
                p,
                true,
                Tolerance::NUMERICAL_VALIDATION,
            )
        })
    }
    /// Retain Arc's layout, normalization and domain policy while delegating
    /// only the unit arc's uniform rebuild to an existing backend.
    pub fn try_deformable_with_rebuild<E: From<GeometryError>>(
        self,
        degree: usize,
        point_count: usize,
        rebuild: &mut impl FnMut(CircularArc3, usize, usize) -> Result<NurbsCurve, E>,
    ) -> Result<NurbsCurve, E> {
        if !(2..=MAX_CURVE_REBUILD_DEGREE).contains(&degree) {
            return Err(GeometryError::InvalidArcDeformDegree { actual: degree }.into());
        }
        if !(4.max(degree + 1)..=MAX_CURVE_REBUILD_POINT_COUNT).contains(&point_count) {
            return Err(GeometryError::InvalidCurveRebuildPointCount {
                actual: point_count,
                minimum: 4.max(degree + 1),
                maximum: MAX_CURVE_REBUILD_POINT_COUNT,
            }
            .into());
        }
        // Public ON_Arc::IsCircle uses ON_ZERO_TOLERANCE (2^-32).
        if std::f64::consts::TAU - self.sweep_radians() <= 2.3283064365386963e-10 {
            return self
                .start_circle()?
                .try_deformable(degree, point_count)
                .map_err(E::from);
        }
        // Fit the unit arc, then map the solved controls into the world frame.
        // Translation and model scale must not contaminate collocation.
        let plane = Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.)?,
            Vector3::try_new(1., 0., 0.)?,
            Vector3::try_new(0., 1., 0.)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let circle = Circle3::try_from_frame(
            plane.origin(),
            1.,
            plane.x_axis(),
            plane.z_axis(),
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let unit = Self::try_from_circle_sweep(circle, self.sweep_radians())?;
        let curve = rebuild(unit, degree, point_count)?;
        if curve.degree() != degree
            || curve.control_points().len() != point_count
            || curve.control_points().iter().any(|p| p.weight() != 1.)
        {
            return Err(GeometryError::InvalidControlNet {
                context: "Arc approximation backend returned an inconsistent layout",
            }
            .into());
        }
        let controls = curve
            .control_points()
            .iter()
            .map(|p| {
                crate::circular::frame_point(
                    self.center(),
                    self.x_axis(),
                    self.y_axis(),
                    self.radius(),
                    p.point().x(),
                    p.point().y(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(
            NurbsCurve::try_new(degree, controls, curve.knots().to_vec())?
                .try_reparameterized(self.domain())?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Real;
    #[test]
    fn arc_deformable_normalizes_spatial_scale_rejects_bad_layout_and_uses_angular_closure() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(1e5, -2e5, 3e5).unwrap(),
            Vector3::try_new(1., 2., 3.).unwrap(),
            Vector3::try_new(-2., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let make = |radius, sweep| {
            CircularArc3::try_from_circle_sweep(
                Circle3::try_from_frame(
                    frame.origin(),
                    radius,
                    frame.x_axis(),
                    frame.z_axis(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
                sweep,
            )
            .unwrap()
        };
        let base = make(1., 2.1).try_deformable(5, 8).unwrap();
        for radius in [0.01, 1000.] {
            let arc = make(radius, 2.1);
            let curve = arc.try_deformable(5, 8).unwrap();
            for (a, b) in curve.control_points().iter().zip(base.control_points()) {
                let expected = frame
                    .origin()
                    .translated(
                        frame
                            .origin()
                            .vector_to(b.point())
                            .unwrap()
                            .scaled(radius)
                            .unwrap(),
                    )
                    .unwrap();
                assert!(a.point().distance_to(expected).unwrap() < 1e-7);
            }
            assert!((curve.domain().end() - radius * 2.1).abs() < 1e-10);
        }
        let arc = make(4., 2.1);
        for (p, n) in [(0, 10), (1, 10), (12, 20), (5, 5), (3, 3), (3, 1001)] {
            assert!(arc.try_deformable(p, n).is_err());
        }
        let mut calls = 0;
        assert!(
            arc.try_deformable_with_rebuild::<GeometryError>(3, 10, &mut |unit, _, _| {
                calls += 1;
                unit.to_nurbs()
            })
            .is_err()
        );
        assert_eq!(calls, 1);
        for (gap, count) in [(1e-9, 10), (1e-11, 13), (0., 13)] {
            let curve = make(4., std::f64::consts::TAU - gap)
                .try_deformable(3, 10)
                .unwrap();
            assert_eq!(curve.control_points().len(), count);
            assert!(
                (curve.domain().end()
                    - if count == 13 {
                        std::f64::consts::TAU
                    } else {
                        4. * (std::f64::consts::TAU - gap)
                    })
                .abs()
                    < 1e-10
            );
        }
    }
    #[test]
    fn arc_deformable_native_controls_knots_and_domains_use_shared_rebuild_and_circle_layouts() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/arc_deform_initial.json"
        ))
        .unwrap();
        let n: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/arc_deform_initial_rhino_20261017.json"
        ))
        .unwrap();
        for index in [0, 2, 4, 6, 8, 9, 10] {
            let op = &q["operations"][index];
            let row = &n["results"][index];
            assert_eq!(op["id"], row["id"]);
            let (mut degree, mut count) = (3, 10);
            for a in op["actions"].as_array().unwrap() {
                if let Some(w) = a.as_str()
                    && let Some((key, v)) = w.split_once('=')
                {
                    if key == "Degree" {
                        degree = v.parse().unwrap();
                    }
                    if key == "PointCount" {
                        count = v.parse().unwrap();
                    }
                }
            }
            let angle = op["actions"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .as_f64()
                .unwrap();
            let plane = Frame3::try_from_directions(
                Point3::try_new(0., 0., 0.).unwrap(),
                Vector3::try_new(1., 0., 0.).unwrap(),
                Vector3::try_new(0., angle.signum(), 0.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let arc = CircularArc3::try_from_circle_sweep(
                Circle3::try_from_frame(
                    plane.origin(),
                    4.,
                    plane.x_axis(),
                    plane.z_axis(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
                angle.abs().to_radians(),
            )
            .unwrap();
            let curve = arc.try_deformable(degree, count).unwrap();
            let expected = &row["value"]["after_script"]["outputs"][0]["curve"];
            assert_eq!(
                curve.degree(),
                expected["degree"].as_u64().unwrap() as usize,
                "{}",
                op["id"]
            );
            assert_eq!(
                curve.control_points().len(),
                expected["control_points"].as_array().unwrap().len(),
                "{}",
                op["id"]
            );
            for (a, b) in curve
                .control_points()
                .iter()
                .zip(expected["control_points"].as_array().unwrap())
            {
                let b: [Real; 3] = serde_json::from_value(b["point"].clone()).unwrap();
                assert!(
                    a.point().distance_to(Point3::try_from(b).unwrap()).unwrap() < 1e-6,
                    "{}: {a:?} != {b:?}",
                    op["id"]
                );
                assert_eq!(a.weight(), 1.);
            }
            assert_eq!(
                curve.knots().len(),
                expected["knots"].as_array().unwrap().len()
            );
            for (a, b) in curve
                .knots()
                .iter()
                .zip(expected["knots"].as_array().unwrap())
            {
                assert!((a - b.as_f64().unwrap()).abs() < 1e-6, "{}", op["id"]);
            }
            let d: [Real; 2] = serde_json::from_value(expected["domain"].clone()).unwrap();
            assert!((curve.domain().start() - d[0]).abs() < 1e-6);
            assert!((curve.domain().end() - d[1]).abs() < 1e-6);
        }
    }
}

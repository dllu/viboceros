//! Native side of the analytic surface/surface Python oracle fixtures.

use super::{Operation, ProbeError, measure};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_geometry::{
    Frame3, GeometryError, NurbsSurface, Point3, SurfaceSurfaceIntersectionEvent, Tolerance,
    Vector3, surface_surface_intersection_events,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SphereSpec {
    center: [f64; 3],
    radius: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CylinderSpec {
    center: [f64; 3],
    axis: [f64; 3],
    radius: f64,
    height: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConeSpec {
    apex: [f64; 3],
    axis: [f64; 3],
    radius: f64,
    height: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TorusSpec {
    center: [f64; 3],
    axis: [f64; 3],
    major_radius: f64,
    minor_radius: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlaneSpec {
    origin: [f64; 3],
    normal: [f64; 3],
    #[serde(default)]
    axes: Option<[[f64; 3]; 2]>,
    x_domain: [f64; 2],
    y_domain: [f64; 2],
}

impl SphereSpec {
    fn surface(&self, tolerance: Tolerance) -> Result<NurbsSurface, GeometryError> {
        let frame = Frame3::try_from_normal(
            Point3::try_from(self.center)?,
            Vector3::try_new(0.0, 0.0, 1.0)?,
            tolerance,
        )?;
        NurbsSurface::try_sphere(frame, self.radius)
    }
}

impl CylinderSpec {
    fn surface(&self, tolerance: Tolerance) -> Result<NurbsSurface, GeometryError> {
        let frame = Frame3::try_from_normal(
            Point3::try_from(self.center)?,
            Vector3::try_from(self.axis)?,
            tolerance,
        )?;
        NurbsSurface::try_cylinder(frame, self.radius, 0.0, self.height)
    }
}

impl ConeSpec {
    fn surface(&self, tolerance: Tolerance) -> Result<NurbsSurface, GeometryError> {
        let frame = Frame3::try_from_normal(
            Point3::try_from(self.apex)?,
            Vector3::try_from(self.axis)?,
            tolerance,
        )?;
        NurbsSurface::try_cone(frame, self.radius, self.height)
    }
}

impl TorusSpec {
    fn surface(&self, tolerance: Tolerance) -> Result<NurbsSurface, GeometryError> {
        let frame = Frame3::try_from_normal(
            Point3::try_from(self.center)?,
            Vector3::try_from(self.axis)?,
            tolerance,
        )?;
        NurbsSurface::try_torus(frame, self.major_radius, self.minor_radius)
    }
}

impl PlaneSpec {
    fn surface(&self, tolerance: Tolerance) -> Result<NurbsSurface, GeometryError> {
        let origin = Point3::try_from(self.origin)?;
        let frame = if let Some([x_axis, y_axis]) = self.axes {
            Frame3::try_from_directions(
                origin,
                Vector3::try_from(x_axis)?,
                Vector3::try_from(y_axis)?,
                tolerance,
            )?
        } else {
            Frame3::try_from_normal(origin, Vector3::try_from(self.normal)?, tolerance)?
        };
        let [x0, x1] = self.x_domain;
        let [y0, y1] = self.y_domain;
        NurbsSurface::try_bilinear([
            frame.point_at([x0, y0, 0.0])?,
            frame.point_at([x1, y0, 0.0])?,
            frame.point_at([x1, y1, 0.0])?,
            frame.point_at([x0, y1, 0.0])?,
        ])
    }
}

pub(super) fn run(
    operation: &Operation,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let (first, second) = match operation {
        Operation::SpherePlaneSurfaceIntersection { sphere, plane, .. } => {
            (sphere.surface(tolerance)?, plane.surface(tolerance)?)
        }
        Operation::SphereSphereSurfaceIntersection {
            sphere,
            other_sphere,
            ..
        } => (sphere.surface(tolerance)?, other_sphere.surface(tolerance)?),
        Operation::SphereCylinderSurfaceIntersection {
            sphere, cylinder, ..
        } => (sphere.surface(tolerance)?, cylinder.surface(tolerance)?),
        Operation::SphereConeSurfaceIntersection { sphere, cone, .. } => {
            (sphere.surface(tolerance)?, cone.surface(tolerance)?)
        }
        Operation::TorusSphereSurfaceIntersection { torus, sphere, .. } => {
            (torus.surface(tolerance)?, sphere.surface(tolerance)?)
        }
        Operation::TorusPlaneSurfaceIntersection { torus, plane, .. } => {
            (torus.surface(tolerance)?, plane.surface(tolerance)?)
        }
        Operation::TorusCylinderSurfaceIntersection {
            torus, cylinder, ..
        } => (torus.surface(tolerance)?, cylinder.surface(tolerance)?),
        Operation::TorusConeSurfaceIntersection { torus, cone, .. } => {
            (torus.surface(tolerance)?, cone.surface(tolerance)?)
        }
        Operation::TorusTorusSurfaceIntersection {
            torus, other_torus, ..
        } => (torus.surface(tolerance)?, other_torus.surface(tolerance)?),
        Operation::CylinderPlaneSurfaceIntersection {
            cylinder, plane, ..
        } => (cylinder.surface(tolerance)?, plane.surface(tolerance)?),
        Operation::CylinderCylinderSurfaceIntersection {
            cylinder,
            other_cylinder,
            ..
        } => (
            cylinder.surface(tolerance)?,
            other_cylinder.surface(tolerance)?,
        ),
        Operation::ConePlaneSurfaceIntersection { cone, plane, .. } => {
            (cone.surface(tolerance)?, plane.surface(tolerance)?)
        }
        Operation::ConeCylinderSurfaceIntersection { cone, cylinder, .. } => {
            (cone.surface(tolerance)?, cylinder.surface(tolerance)?)
        }
        _ => unreachable!("surface intersection runner only accepts surface pairs"),
    };
    let (events, elapsed_ns) = measure(iterations, || {
        surface_surface_intersection_events(&first, &second, tolerance)
    })?;
    let success = !events.is_empty();
    let mut curves = Vec::new();
    let mut points = Vec::new();
    for event in events {
        match event {
            SurfaceSurfaceIntersectionEvent::Point(point) => points.push(point.to_array()),
            SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                let domain = curve.domain();
                let start = *domain.start();
                let end = *domain.end();
                let middle = 0.5 * start + 0.5 * end;
                curves.push(json!({
                    "degree": curve.degree(),
                    "closed": curve.is_closed()?,
                    "samples": [
                        curve.evaluate(start)?.to_array(),
                        curve.evaluate(middle)?.to_array(),
                        curve.evaluate(end)?.to_array(),
                    ],
                    "length": curve.length(tolerance)?,
                }));
            }
        }
    }
    Ok((
        json!({"success": success, "curves": curves, "points": points}),
        elapsed_ns,
    ))
}

#[cfg(test)]
mod tests {
    use crate::{OperationOutcome, ProbeRequest, run_request_audit};

    #[test]
    fn all_analytic_surface_oracle_fixtures_are_available_to_the_native_python_api() {
        let fixtures = [
            include_str!(
                "../../../tools/rhino_oracle/fixtures/sphere_plane_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/sphere_sphere_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/sphere_cylinder_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/sphere_cone_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/torus_sphere_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/torus_plane_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/torus_cylinder_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/torus_cone_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/torus_torus_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/cylinder_plane_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/cylinder_cylinder_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/cone_plane_surface_intersection.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/fixtures/cone_cylinder_surface_intersection.json"
            ),
        ];
        for fixture in fixtures {
            let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
            let response = run_request_audit(&request).unwrap();
            assert_eq!(response.outcomes.len(), request.operations.len());
            for (operation, outcome) in request.operations.iter().zip(response.outcomes) {
                match outcome {
                    OperationOutcome::Success { result } => {
                        assert!(result.value["success"].is_boolean());
                        assert!(result.value["curves"].is_array());
                        assert!(result.value["points"].is_array());
                    }
                    OperationOutcome::Failure { id, error } => {
                        panic!("unexpected failure for {id}: {error:?} (operation={operation:?})");
                    }
                }
            }
        }
    }

    #[test]
    fn python_oracle_receives_exact_sphere_sphere_section_samples() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/sphere_sphere_surface_intersection.json"
        ))
        .unwrap();
        let response = run_request_audit(&request).unwrap();
        let OperationOutcome::Success { result } = &response.outcomes[0] else {
            panic!("secant sphere fixture must succeed")
        };
        assert_eq!(result.value["curves"].as_array().unwrap().len(), 2);
        assert_eq!(result.value["points"].as_array().unwrap().len(), 0);
        for curve in result.value["curves"].as_array().unwrap() {
            assert_eq!(curve["degree"], 2);
            assert_eq!(curve["closed"], false);
            for sample in curve["samples"].as_array().unwrap() {
                let location = sample.as_array().unwrap();
                assert!((location[0].as_f64().unwrap() - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn python_oracle_reports_offset_torus_sphere_topology() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/torus_sphere_surface_intersection.json"
        ))
        .unwrap();
        let response = run_request_audit(&request).unwrap();
        let expected = [
            ("two_axial_circles", 2, 0, Some(2)),
            ("two_offset_loops", 2, 0, Some(3)),
            ("two_turned_loops", 2, 0, Some(3)),
            ("offset_axial_loops", 2, 0, Some(3)),
            ("isolated_tangent", 0, 1, None),
            ("contained_meridian", 1, 0, Some(2)),
            ("disjoint", 0, 0, None),
        ];
        for (outcome, (id, curve_count, point_count, degree)) in
            response.outcomes.iter().zip(expected)
        {
            let OperationOutcome::Success { result } = outcome else {
                panic!("torus/sphere oracle fixture {id} must succeed")
            };
            assert_eq!(result.id, id);
            let curves = result.value["curves"].as_array().unwrap();
            assert_eq!(curves.len(), curve_count, "{id}");
            assert_eq!(
                result.value["points"].as_array().unwrap().len(),
                point_count,
                "{id}"
            );
            for curve in curves {
                assert_eq!(curve["degree"], degree.unwrap());
                assert_eq!(curve["closed"], true);
            }
        }
    }

    #[test]
    fn python_oracle_reports_parallel_offset_torus_cylinder_topology() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/torus_cylinder_surface_intersection.json"
        ))
        .unwrap();
        let response = run_request_audit(&request).unwrap();
        let expected = [
            ("coaxial_two_circles", 2, 0, Some(2), true),
            ("offset_two_loops", 2, 0, Some(3), true),
            ("offset_turned_loops", 2, 0, Some(3), true),
            ("finite_rim_arcs", 2, 0, Some(3), false),
            ("critical_crossing", 2, 0, Some(3), true),
            ("isolated_tangent", 0, 1, None, false),
            ("disjoint", 0, 0, None, false),
        ];
        assert_eq!(response.outcomes.len(), expected.len());
        for (outcome, (id, curve_count, point_count, degree, closed)) in
            response.outcomes.iter().zip(expected)
        {
            let OperationOutcome::Success { result } = outcome else {
                panic!("torus/cylinder oracle fixture {id} must succeed")
            };
            assert_eq!(result.id, id);
            let curves = result.value["curves"].as_array().unwrap();
            assert_eq!(curves.len(), curve_count, "{id}");
            assert_eq!(
                result.value["points"].as_array().unwrap().len(),
                point_count,
                "{id}"
            );
            for curve in curves {
                assert_eq!(curve["degree"], degree.unwrap());
                assert_eq!(curve["closed"], closed);
            }
        }
    }

    #[test]
    fn python_oracle_reports_parallel_offset_torus_sections() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/torus_torus_surface_intersection.json"
        ))
        .unwrap();
        let response = run_request_audit(&request).unwrap();
        let expected = [
            ("coaxial_two_circles", vec![2, 2], 0),
            ("coaxial_tangent_circle", vec![2], 0),
            ("equal_parallel_offset_four_loops", vec![3, 3, 3, 3], 0),
            ("equal_parallel_offset_meridian", vec![2, 3], 0),
            ("unequal_major_parallel_four_loops", vec![3, 3, 3, 3], 0),
            ("unequal_major_inner_pinch", vec![3, 3, 3, 3], 0),
            ("unequal_major_inner_contact", vec![], 1),
            ("unequal_major_near_coaxial_tangent", vec![3], 0),
            ("unequal_major_parallel_meridian", vec![2, 3], 0),
            ("disjoint", vec![], 0),
        ];
        assert_eq!(response.outcomes.len(), expected.len());
        for (outcome, (id, expected_degrees, point_count)) in response.outcomes.iter().zip(expected)
        {
            let OperationOutcome::Success { result } = outcome else {
                panic!("torus/torus oracle fixture {id} must succeed")
            };
            assert_eq!(result.id, id);
            let mut degrees = result.value["curves"]
                .as_array()
                .unwrap()
                .iter()
                .map(|curve| curve["degree"].as_u64().unwrap())
                .collect::<Vec<_>>();
            degrees.sort_unstable();
            assert_eq!(degrees, expected_degrees, "{id}");
            assert_eq!(
                result.value["points"].as_array().unwrap().len(),
                point_count
            );
        }
    }
}

//! Public directional projection comparison, independent fixture inputs.
use super::*;

pub(super) fn run(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    surface: &NurbsSurfaceDefinition,
    direction: [f64; 3],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_curve_from_definition(curve)?;
    let target = nurbs_surface_from_definition(surface)?;
    let direction = Vector3::try_from(direction)?
        .normalized_nonzero()?
        .as_vector()
        .to_array();
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let a = doc.add_geometry(Geometry::NurbsCurve(source.clone()))?;
        let b = doc.add_geometry(Geometry::NurbsSurface(target.clone()))?;
        doc.select_objects_direct([a, b], SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "Project Direction={},{},{}",
                direction[0], direction[1], direction[2]
            ),
        )?;
        let mut curves = Vec::new();
        for o in doc.objects().filter(|o| o.id() != a && o.id() != b) {
            let c =
                o.geometry()
                    .nurbs_curve_representation()?
                    .ok_or(ProbeError::FixtureInvariant(
                        "curve projection produced a point",
                    ))?;
            let mut points = CurveRef::NurbsCurve(&c)
                .divide_by_count(32, true, tolerance)?
                .into_iter()
                .map(Point3::to_array)
                .collect::<Vec<_>>();
            if compare_point(points.last().unwrap(), &points[0]).is_lt() {
                points.reverse();
            }
            curves.push(
                json!({"samples":points,"length":c.length(tolerance)?,"closed":c.is_closed()?}),
            );
        }
        curves.sort_by(|a, b| {
            let point = |v: &Value| std::array::from_fn(|i| v["samples"][0][i].as_f64().unwrap());
            compare_point(&point(a), &point(b))
        });
        Ok::<_, ProbeError>(json!({"curves":curves}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "native-smlib")]
    #[test]
    fn project_curve_surface_protocol_covers_finite_and_curved_targets() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/project_curve_surface.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 6);
        for row in &response.results {
            let curves = row.value["curves"].as_array().unwrap();
            assert_eq!(curves.len(), usize::from(row.id != "project_miss"));
            for curve in curves {
                assert_eq!(curve["samples"].as_array().unwrap().len(), 33);
                assert!(curve["length"].as_f64().unwrap() > 0.);
            }
            if row.id.ends_with("graph") {
                for curve in curves {
                    for p in curve["samples"].as_array().unwrap() {
                        let xyz = std::array::from_fn::<_, 3, _>(|i| p[i].as_f64().unwrap());
                        assert!(
                            (xyz[2] - 0.04 * xyz[0] * xyz[1]).abs() < request.tolerance.absolute
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn project_curve_surface_zero_direction_rejects_request() {
        let mut request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/project_curve_surface.json"
        ))
        .unwrap();
        request.operations.truncate(1);
        if let Operation::ProjectCurveSurface { direction, .. } = &mut request.operations[0] {
            *direction = [0.; 3];
        } else {
            panic!()
        }
        assert!(run_request(&request).is_err());
    }
}

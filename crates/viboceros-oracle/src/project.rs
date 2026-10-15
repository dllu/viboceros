//! Public directional projection comparison, independent fixture inputs.
use super::*;

pub(super) fn plane_curve(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    origin: [f64; 3],
    normal: [f64; 3],
    direction: [f64; 3],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let curve = nurbs_curve_from_definition(curve)?;
    let origin = Point3::try_from(origin)?;
    let normal = Vector3::try_from(normal)?.normalized_nonzero()?.as_vector();
    let direction = Vector3::try_from(direction)?
        .normalized_nonzero()?
        .as_vector();
    if normal.dot(direction)? == 0. {
        return Err(ProbeError::FixtureInvariant(
            "plane SDK comparison requires nonparallel direction",
        ));
    }
    let normal = normal.to_array();
    let direction = direction.to_array();
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let source = doc.add_geometry(Geometry::NurbsCurve(curve.clone()))?;
        doc.select_objects_direct([source], SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "Project InfinitePlane={},{},{},{},{},{} Direction={},{},{}",
                origin.x(),
                origin.y(),
                origin.z(),
                normal[0],
                normal[1],
                normal[2],
                direction[0],
                direction[1],
                direction[2]
            ),
        )?;
        let outputs = doc
            .selected_objects()
            .filter(|o| o.id() != source)
            .map(|o| {
                let c = o.geometry().nurbs_curve_representation()?.ok_or(
                    ProbeError::FixtureInvariant("plane projection produced a point"),
                )?;
                Ok(nurbs_curve_definition_value(&c))
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        Ok(json!({"curves":outputs}))
    })
}

pub(super) fn run(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    surface: &NurbsSurfaceDefinition,
    direction: [f64; 3],
    tolerance: Tolerance,
    loose: bool,
) -> Result<(Value, u64), ProbeError> {
    let source = nurbs_curve_from_definition(curve)?;
    let target = nurbs_surface_from_definition(surface)?;
    run_geometry(
        iterations,
        Geometry::NurbsCurve(source),
        Geometry::NurbsSurface(target),
        direction,
        tolerance,
        false,
        loose,
    )
}

fn run_geometry(
    iterations: u32,
    source: Geometry,
    target: Geometry,
    direction: [f64; 3],
    tolerance: Tolerance,
    point_output: bool,
    loose: bool,
) -> Result<(Value, u64), ProbeError> {
    let direction = Vector3::try_from(direction)?
        .normalized_nonzero()?
        .as_vector()
        .to_array();
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let a = doc.add_geometry(source.clone())?;
        let b = doc.add_geometry(target.clone())?;
        doc.select_objects_direct([a, b], SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "Project Direction={},{},{} Loose={}",
                direction[0],
                direction[1],
                direction[2],
                if loose { "Yes" } else { "No" }
            ),
        )?;
        let mut curves = Vec::new();
        let mut points = Vec::new();
        for o in doc.objects().filter(|o| o.id() != a && o.id() != b) {
            if let Geometry::Point(p) = o.geometry() {
                if !point_output {
                    return Err(ProbeError::FixtureInvariant(
                        "curve projection produced a point",
                    ));
                }
                points.push(p.to_array());
                continue;
            }
            if point_output {
                return Err(ProbeError::FixtureInvariant(
                    "point projection produced a curve",
                ));
            }
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
            let mut record =
                json!({"samples":points,"length":c.length(tolerance)?,"closed":c.is_closed()?});
            if loose {
                record["definition"] = nurbs_curve_definition_value(&c);
            }
            curves.push(record);
        }
        curves.sort_by(|a, b| {
            let point = |v: &Value| std::array::from_fn(|i| v["samples"][0][i].as_f64().unwrap());
            compare_point(&point(a), &point(b))
        });
        points.sort_by(compare_point);
        Ok::<_, ProbeError>(if point_output {
            json!({"points":points})
        } else {
            json!({"curves":curves})
        })
    })
}

pub(super) fn target_mesh(
    vertices: &[[f64; 3]],
    faces: &[Vec<u32>],
    tolerance: Tolerance,
) -> Result<Geometry, ProbeError> {
    Ok(Geometry::Mesh(TriangleMesh::try_new_faces(
        vertices
            .iter()
            .copied()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        polygon_mesh_faces(faces)?,
        tolerance,
    )?))
}
pub(super) fn mesh_curve(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    vertices: &[[f64; 3]],
    faces: &[Vec<u32>],
    direction: [f64; 3],
    tolerance: Tolerance,
    loose: bool,
) -> Result<(Value, u64), ProbeError> {
    run_geometry(
        iterations,
        Geometry::NurbsCurve(nurbs_curve_from_definition(curve)?),
        target_mesh(vertices, faces, tolerance)?,
        direction,
        tolerance,
        false,
        loose,
    )
}
pub(super) fn mesh_point(
    iterations: u32,
    point: [f64; 3],
    vertices: &[[f64; 3]],
    faces: &[Vec<u32>],
    direction: [f64; 3],
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_geometry(
        iterations,
        Geometry::Point(Point3::try_from(point)?),
        target_mesh(vertices, faces, tolerance)?,
        direction,
        tolerance,
        true,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn infinite_plane_protocol_retains_source_structure_and_rejects_parallel_sdk_inputs() {
        let mut q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/project_curve_plane.json"
        ))
        .unwrap();
        let response = run_request(&q).unwrap();
        assert_eq!(response.results.len(), 4);
        for (op, row) in q.operations.iter().zip(response.results) {
            let Operation::ProjectCurvePlane { curve, .. } = op else {
                panic!()
            };
            let outputs = row.value["curves"].as_array().unwrap();
            assert_eq!(outputs.len(), 1);
            let output = &outputs[0];
            assert_eq!(output["degree"], json!(curve.degree));
            assert_eq!(output["knots"], json!(curve.knots));
            assert_eq!(
                output["control_points"].as_array().unwrap().len(),
                curve.control_points.len()
            );
            for (a, b) in output["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(&curve.control_points)
            {
                assert_eq!(a["weight"], json!(b.weight));
            }
        }
        q.operations.truncate(1);
        if let Operation::ProjectCurvePlane { direction, .. } = &mut q.operations[0] {
            *direction = [1., 0., 0.];
        }
        assert!(run_request(&q).is_err());
    }
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

#[cfg(test)]
mod mesh_tests {
    use super::*;
    #[test]
    fn mesh_projection_protocol_checks_points_curves_warped_quads_and_misses() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/project_mesh.json"
        ))
        .unwrap();
        let response = run_request(&q).unwrap();
        assert_eq!(response.results.len(), 13);
        for row in response.results {
            if let Some(points) = row.value["points"].as_array() {
                let count = if row.id.ends_with("miss") {
                    0
                } else if row.id.ends_with("two_sheets") {
                    2
                } else {
                    1
                };
                assert_eq!(points.len(), count);
                if row.id.ends_with("warped") {
                    assert_eq!(points[0], json!([0.25, 0.75, 0.]));
                }
            } else {
                let curves = row.value["curves"].as_array().unwrap();
                assert_eq!(
                    curves.len(),
                    usize::from(!row.id.ends_with("miss") && !row.id.ends_with("vertex_touch")),
                    "{}",
                    row.id
                );
                for c in curves {
                    assert_eq!(c["samples"].as_array().unwrap().len(), 33);
                    for p in c["samples"].as_array().unwrap() {
                        let x = p[0].as_f64().unwrap();
                        let y = p[1].as_f64().unwrap();
                        let z = p[2].as_f64().unwrap();
                        let expected = if row.id.ends_with("warped") {
                            (x + y - 1.).max(0.)
                        } else {
                            0.
                        };
                        assert!((z - expected).abs() < q.tolerance.absolute);
                    }
                }
            }
        }
    }
}

//! Mesh Pull command compared with the public Curve.PullToMesh API.
use super::*;

pub(super) fn run(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    vertices: &[[f64; 3]],
    faces: &[Vec<u32>],
    tolerance: Tolerance,
    loose: bool,
) -> Result<(Value, u64), ProbeError> {
    let mut source = Document::new(tolerance);
    let first = source.add_geometry(Geometry::NurbsCurve(nurbs_curve_from_definition(curve)?))?;
    let target = source.add_geometry(project::target_mesh(vertices, faces, tolerance)?)?;
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "Pull Sources={first} Targets={target} Loose={}",
        if loose { "Yes" } else { "No" }
    );
    measure_probe(iterations, || {
        let mut doc = source.clone();
        registry.execute(&mut doc, &command)?;
        let mut curves = Vec::new();
        for object in doc.selected_objects() {
            let curve = object
                .geometry()
                .curve_ref()
                .ok_or(ProbeError::FixtureInvariant("mesh Pull returned a point"))?;
            let mut samples = curve
                .divide_by_count(32, true, tolerance)?
                .into_iter()
                .map(Point3::to_array)
                .collect::<Vec<_>>();
            if compare_point(samples.last().unwrap(), &samples[0]).is_lt() {
                samples.reverse();
            }
            let mut record = json!({"samples":samples,"length":curve.length(tolerance)?,"closed":curve.is_closed()?});
            if loose {
                record["definition"] = nurbs_curve_definition_value(
                    &object.geometry().nurbs_curve_representation()?.unwrap(),
                );
            }
            curves.push(record);
        }
        curves.sort_by(|a, b| {
            let first = |v: &Value| std::array::from_fn(|i| v["samples"][0][i].as_f64().unwrap());
            compare_point(&first(a), &first(b))
        });
        Ok::<_, ProbeError>(json!({"curves":curves}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_pull_recipes_admit_boundary_paths_and_folded_faces() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/pull_curve_mesh.json"
        ))
        .unwrap();
        let result = run_request(&request).unwrap();
        assert_eq!(result.results.len(), 4);
        for (row, expected) in result.results.iter().zip([8., 1. + 2_f64.sqrt(), 0., 7.]) {
            let curves = row.value["curves"].as_array().unwrap();
            assert_eq!(curves.len(), usize::from(expected > 0.));
            assert!(
                (curves
                    .iter()
                    .map(|c| c["length"].as_f64().unwrap())
                    .sum::<f64>()
                    - expected)
                    .abs()
                    <= request.tolerance.absolute
            );
        }
    }
}

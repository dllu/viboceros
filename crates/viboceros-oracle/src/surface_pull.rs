//! Surface-normal pull geometry compared through the public Rhino face API.
use super::*;
#[cfg(feature = "native-smlib")]
pub(super) fn run(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    surface: &NurbsSurfaceDefinition,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let curve = nurbs_curve_from_definition(curve)?;
    let surface = nurbs_surface_from_definition(surface)?;
    measure_probe(iterations, || {
        let output = viboceros_smlib::pull_curve_to_surface(&curve, &surface, tolerance)
            .map_err(CommandError::from)?;
        let mut curves = Vec::new();
        for output in output {
            let curve = CurveRef::NurbsCurve(&output.spatial);
            let mut samples = curve
                .divide_by_count(32, true, tolerance)?
                .into_iter()
                .map(Point3::to_array)
                .collect::<Vec<_>>();
            if compare_point(samples.last().unwrap(), &samples[0]).is_lt() {
                samples.reverse();
            }
            curves.push(json!({"samples":samples,"length":curve.length(tolerance)?,"closed":curve.is_closed()?}));
        }
        curves.sort_by(|a, b| {
            let point = |v: &Value| std::array::from_fn(|i| v["samples"][0][i].as_f64().unwrap());
            compare_point(&point(a), &point(b))
        });
        Ok::<_, ProbeError>(json!({"curves":curves}))
    })
}
#[cfg(not(feature = "native-smlib"))]
pub(super) fn run(
    _: u32,
    _: &NurbsCurveDefinition,
    _: &NurbsSurfaceDefinition,
    _: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    Err(ProbeError::FixtureInvariant(
        "pull_curve_surface requires native-smlib",
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surface_pull_protocol_keeps_its_native_requirement_explicit() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/pull_curve_surface.json"
        ))
        .unwrap();
        #[cfg(feature = "native-smlib")]
        {
            let result = run_request(&request).unwrap();
            assert_eq!(result.results.len(), 6);
            for (i, expected) in [(0, 8.), (1, 10.), (4, 4. * std::f64::consts::PI), (5, 16.)] {
                let curves = result.results[i].value["curves"].as_array().unwrap();
                if i != 4 {
                    assert_eq!(curves.len(), 1, "{}", result.results[i].id);
                }
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
            assert_eq!(result.results[2].value["curves"], json!([]));
            // The cylinder seam has two UV preimages. Keep both native
            // witnesses; command-level geometric admission is a separate step.
            let axial = result.results[3].value["curves"].as_array().unwrap();
            assert_eq!(axial.len(), 2);
            for curve in axial {
                assert!(
                    (curve["length"].as_f64().unwrap() - 4.).abs() <= request.tolerance.absolute
                );
            }
        }
        #[cfg(not(feature = "native-smlib"))]
        assert!(
            run_request(&request)
                .unwrap_err()
                .to_string()
                .contains("requires native-smlib")
        );
    }
}

/// Exercise document admission and spatial seam deduplication through Pull.
pub(super) fn command(
    iterations: u32,
    curve: &NurbsCurveDefinition,
    surface: &NurbsSurfaceDefinition,
    tolerance: Tolerance,
    loose: bool,
) -> Result<(Value, u64), ProbeError> {
    let mut source = Document::new(tolerance);
    let first = source.add_geometry(Geometry::NurbsCurve(nurbs_curve_from_definition(curve)?))?;
    let target = source.add_geometry(Geometry::NurbsSurface(nurbs_surface_from_definition(
        surface,
    )?))?;
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
                .ok_or(ProbeError::FixtureInvariant(
                    "Pull curve probe returned a point",
                ))?;
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
            let point = |v: &Value| std::array::from_fn(|i| v["samples"][0][i].as_f64().unwrap());
            compare_point(&point(a), &point(b))
        });
        Ok::<_, ProbeError>(json!({"curves":curves}))
    })
}
#[cfg(test)]
mod command_tests {
    use super::*;
    #[test]
    fn command_recipe_admission_removes_the_duplicate_cylinder_seam() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/pull_curve_surface_command.json"
        ))
        .unwrap();
        #[cfg(not(feature = "native-smlib"))]
        let request = ProbeRequest {
            operations: request
                .operations
                .into_iter()
                .filter(|op| op.id().starts_with("plane-"))
                .collect(),
            ..request
        };
        let result = run_request(&request).unwrap();
        assert!(!result.results.is_empty());
        for row in result.results {
            let curves = row.value["curves"].as_array().unwrap();
            let expected = match row.id.as_str() {
                "plane-inside" => 8.,
                "plane-cross" => 10.,
                "plane-miss" => 0.,
                "cylinder-axial" => 4.,
                "cylinder-loop" => 4. * std::f64::consts::PI,
                "plane-translated-domains" => 16.,
                _ => unreachable!(),
            };
            if row.id == "cylinder-axial" {
                assert_eq!(curves.len(), 1);
            }
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

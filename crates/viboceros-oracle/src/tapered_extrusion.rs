//! Command-side tapered extrusion diagnostics for public SDK comparison.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub displacement: [f64; 3],
    pub draft_angle: f64,
    #[serde(default)]
    pub round_corners: bool,
    #[serde(default)]
    pub solid: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let curve = nurbs_curve_from_definition(&f.curve)?;
    let displacement = Vector3::try_from(f.displacement)?;
    let height = displacement.length()?;
    let [x, y, z] = displacement.normalized_nonzero()?.as_vector().to_array();
    let command = format!(
        "ExtrudeCrvTapered {height} Direction={x},{y},{z} DraftAngle={} Solid={} Corners={}",
        f.draft_angle,
        if f.solid { "Yes" } else { "No" },
        if f.round_corners { "Round" } else { "Sharp" }
    );
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(Geometry::NurbsCurve(curve.clone()))?;
        doc.select_objects_direct([id], SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(&mut doc, &command)?;
        let mut records = Vec::new();
        for obj in doc.selected_objects() {
            let Geometry::Brep(brep) = obj.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "tapered result is not a B-rep",
                ));
            };
            let volume = brep
                .is_solid()
                .then(|| brep.signed_volume(tolerance).map(f64::abs))
                .transpose()?;
            let bounds = brep.bounds();
            records.push(
                json!({"is_solid":brep.is_solid(),"face_count":brep.faces().len(),"volume":volume,
                "bounds":[bounds.min().to_array(),bounds.max().to_array()]}),
            );
        }
        Ok(json!({"breps":records}))
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn pending_tapered_fixtures_run_through_protocol_and_match_analytic_volumes() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/tapered_extrusion.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 7);
        let k = 10_f64.to_radians().tan();
        for row in response.results {
            let values = row.value["breps"].as_array().unwrap();
            assert_eq!(values.len(), 1);
            let record = &values[0];
            if row.id == "tapered_open_line" {
                assert_eq!(record["is_solid"], false);
                assert!(record["volume"].is_null());
                continue;
            }
            assert_eq!(record["is_solid"], true);
            let expected = if row.id.contains("rectangle") {
                if row.id.contains("round") {
                    72. + 48. * k + std::f64::consts::PI * k * k * 8. / 3.
                } else {
                    let width = 6. - 4. * k;
                    2. / 3. * (36. + 6. * width + width * width)
                }
            } else {
                let r = if row.id.contains("outward") {
                    3. + 2. * k
                } else {
                    3. - 2. * k
                };
                std::f64::consts::PI * 2. / 3. * (9. + 3. * r + r * r)
            };
            assert!(
                (record["volume"].as_f64().unwrap() - expected).abs() < 1e-3,
                "{}: {}",
                row.id,
                row.value
            );
        }
    }
}

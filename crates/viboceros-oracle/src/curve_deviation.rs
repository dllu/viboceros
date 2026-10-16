//! Native command/SDK observations versus an independent interval-query backend.
use super::*;
use crate::curve_join_close::CurveInput;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: [CurveInput; 2],
    pub postselect: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    #[cfg(feature = "native-smlib")]
    {
        let a = f.curves[0].geometry()?;
        let b = f.curves[1].geometry()?;
        measure_probe(iterations, || -> Result<Value, ProbeError> {
            let ranges =
                viboceros_smlib::projected_curve_intervals(a.as_ref(), b.as_ref(), tolerance)
                    .map_err(CommandError::from)?;
            let report = ranges
                .map(|ranges| {
                    viboceros_smlib::curve_distance_extrema(
                        a.as_ref(),
                        b.as_ref(),
                        ranges,
                        tolerance,
                    )
                })
                .transpose()
                .map_err(CommandError::from)?
                .flatten();
            let witness = |w: &viboceros_smlib::CurveDistanceWitness| json!({"normalized_parameters":w.normalized_parameters,"parameters":w.parameters,"points":w.points.map(Point3::to_array),"distance":w.distance});
            Ok(
                json!({"backend_only":true,"command_registered":CommandRegistry::with_builtins().recognizes("CrvDeviation"),"intervals":ranges,
                "sdk":{"created":report.is_some(),"maximum":report.as_ref().map(|r|witness(&r.maximum)),"minimum":report.as_ref().map(|r|witness(&r.minimum))}}),
            )
        })
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (f, iterations, tolerance);
        Err(ProbeError::FixtureInvariant(
            "curve distance queries require native-smlib",
        ))
    }
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn curve_distance_protocol_retains_four_native_metric_disagreements() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/curve_deviation.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/curve_deviation_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        let mut scalar_matches = 0;
        let mut failures = 0;
        assert_eq!(actual.results.len(), 18);
        for (row, reference) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(row.id, reference["id"]);
            assert_eq!(row.value["backend_only"], true);
            let a = &row.value["sdk"];
            let b = &reference["value"]["sdk"];
            if a["created"] != b["created"] {
                assert!(matches!(
                    row.id.as_str(),
                    "crossing-circles" | "separated-circles"
                ));
                failures += 1;
            } else if a["created"] == true {
                assert!(
                    (a["maximum"]["distance"].as_f64().unwrap()
                        - b["maximum"]["distance"].as_f64().unwrap())
                    .abs()
                        <= 1e-6
                );
                if matches!(row.id.as_str(), "curved-offset" | "curved-spatial") {
                    assert!(
                        a["minimum"]["distance"].as_f64().unwrap()
                            < b["minimum"]["distance"].as_f64().unwrap() - 1e-4
                    );
                    failures += 1;
                } else {
                    assert!(
                        (a["minimum"]["distance"].as_f64().unwrap()
                            - b["minimum"]["distance"].as_f64().unwrap())
                        .abs()
                            <= 1e-6
                    );
                    scalar_matches += 1;
                }
            }
        }
        assert_eq!((scalar_matches, failures), (11, 4));
    }
}

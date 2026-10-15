//! Independent edge-point recipes execute the actual BlendEdge command.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub edge_points: Vec<[f64; 3]>,
    pub radius: f64,
    pub rail_type: String,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    edge_blend::run(
        edge_blend::Recipe {
            source: &f.source,
            edge_points: &f.edge_points,
            size: f.radius,
            rail_type: &f.rail_type,
            command: "BlendEdge",
            parameter: "Radius",
        },
        iterations,
        tolerance,
    )
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn blend_fixtures_match_independent_quintic_sections() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/blend_edge.json"
        ))
        .unwrap();
        let a = 3. / 5. - 23. * 2_f64.sqrt() / 70.;
        for op in &q.operations {
            let mut single = q.clone();
            single.operations = vec![op.clone()];
            let r = run_request(&single).unwrap_or_else(|e| panic!("{}: {e}", op.id()));
            let row = &r.results[0];
            let expected = match row.id.as_str() {
                "blend_rolling" => 480. - 6. * a,
                "blend_width" | "blend_shifted" => 480. - 3. * a,
                "blend_parallel" => 480. - 6. * a,
                "blend_cylinder_rolling" | "blend_cylinder_width" => {
                    let s = if row.id.ends_with("rolling") {
                        1.
                    } else {
                        1. / 2_f64.sqrt()
                    };
                    let m = -10121. / 30030. + 731. * 2_f64.sqrt() / 2860.;
                    std::f64::consts::PI * (200. - 10. * s * s * a + 2. * s * s * s * m)
                }
                _ => panic!(),
            };
            let b = &row.value["breps"][0];
            assert_eq!(b["is_solid"], true);
            assert!(
                (b["volume"].as_f64().unwrap() - expected).abs() < 1e-5,
                "{}: {} expected {expected}",
                row.id,
                row.value
            );
        }
    }
}

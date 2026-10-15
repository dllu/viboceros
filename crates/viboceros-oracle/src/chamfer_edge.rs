//! Independent edge-point recipes execute the actual ChamferEdge command.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub edge_points: Vec<[f64; 3]>,
    pub distance: f64,
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
            size: f.distance,
            rail_type: &f.rail_type,
            command: "ChamferEdge",
            parameter: "Distance",
        },
        iterations,
        tolerance,
    )
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn chamfer_fixtures_match_independent_box_and_conical_volumes() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/chamfer_edge.json"
        ))
        .unwrap();
        assert_eq!(q.operations.len(), 6);
        for op in &q.operations {
            let mut single = q.clone();
            single.operations = vec![op.clone()];
            let r = run_request(&single).unwrap_or_else(|e| panic!("{}: {e}", op.id()));
            let row = r.results.into_iter().next().unwrap();
            let expected = match row.id.as_str() {
                "chamfer_width" | "chamfer_shifted" => 478.5,
                "chamfer_rolling" => 477.,
                "chamfer_parallel" => 477.,
                "chamfer_cylinder_width" => {
                    let s = 1. / 2_f64.sqrt();
                    std::f64::consts::PI * (200. - 5. * s * s + s * s * s / 3.)
                }
                "chamfer_cylinder_rolling" => std::f64::consts::PI * (200. - 5. + 1. / 3.),
                _ => panic!(),
            };
            let b = &row.value["breps"][0];
            assert_eq!(b["is_solid"], true);
            assert!(
                (b["volume"].as_f64().unwrap() - expected).abs() < 1e-4,
                "{}: {} expected {expected}",
                row.id,
                row.value
            );
        }
    }
}

//! Native tangent rounding at relocated and reversed rational-curve seams.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};

#[test]
fn native_tangent_components_match_licensed_seam_and_reversal_queries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/loft_splitting_tangent_jets.json")).unwrap(),
    )
    .unwrap();
    let response: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/loft_splitting_tangent_jets.json"))
            .unwrap(),
    )
    .unwrap();
    let ops = request["operations"].as_array().unwrap();
    let rows = response["results"].as_array().unwrap();
    assert_eq!(ops.len(), 32);
    assert_eq!(rows.len(), 32);
    for (op, row) in ops.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let definition = &op["curve"];
        let controls = definition["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_new(
                        c["point"][0].as_f64().unwrap(),
                        c["point"][1].as_f64().unwrap(),
                        c["point"][2].as_f64().unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        let knots = definition["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_f64().unwrap())
            .collect();
        let mut curve = NurbsCurve::try_new_rational(
            definition["degree"].as_u64().unwrap() as usize,
            controls,
            knots,
        )
        .unwrap();
        if op["reversed"] == true {
            curve = curve.reversed().unwrap();
        }
        curve = curve
            .try_change_closed_seam(op["edit"]["parameter"].as_f64().unwrap())
            .unwrap();
        let controls = curve
            .control_points()
            .iter()
            .map(|c| {
                let p = c.point();
                [p.x(), p.y(), p.z(), c.weight()]
            })
            .collect::<Vec<_>>();
        let actual = viboceros_opennurbs::curve_tangents(
            curve.degree(),
            &controls,
            curve.knots(),
            &[*curve.domain().start()],
        )
        .unwrap()[0]
            .unwrap();
        let expected = &row["value"]["curves"][0]["samples"][0]["tangent"];
        for (axis, actual) in actual.into_iter().enumerate() {
            let expected = expected[axis].as_f64().unwrap();
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "{} axis {axis}: {actual:?} != {expected:?}",
                op["id"]
            );
        }
    }
}

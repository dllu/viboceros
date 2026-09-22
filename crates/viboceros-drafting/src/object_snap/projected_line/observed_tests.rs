//! Public per-line picking is checked separately from whole-mesh wire choice.
use super::super::ProjectedSnapMetric;
use super::*;
use serde_json::Value;

#[test]
fn per_wire_targets_and_misses_match_520_public_line_picks() {
    let mut tested = 0;
    let mut captured = 0;
    for (request, response) in [
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/mesh_snap_order.json"),
            include_str!("../../../../../tools/rhino_oracle/observations/mesh_snap_order.json"),
        ),
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/mesh_snap_endpoints.json"),
            include_str!("../../../../../tools/rhino_oracle/observations/mesh_snap_endpoints.json"),
        ),
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/mesh_snap_edge_on.json"),
            include_str!("../../../../../tools/rhino_oracle/observations/mesh_snap_edge_on.json"),
        ),
    ] {
        let request: Value = serde_json::from_str(request).unwrap();
        let response: Value = serde_json::from_str(response).unwrap();
        let operations = request["operations"].as_array().unwrap();
        let rows = response["results"].as_array().unwrap();
        assert_eq!(operations.len(), rows.len());
        for (op, row) in operations.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            let value = &row["value"];
            let Some(picks) = value.get("wire_picks") else {
                continue;
            };
            let frame = &value["frame"];
            let matrix: [[Real; 4]; 4] =
                serde_json::from_value(frame["world_to_screen"].clone()).unwrap();
            let metric = ProjectedSnapMetric {
                cursor: serde_json::from_value(frame["click_client"].clone()).unwrap(),
                capture_radius: op
                    .get("capture_radius")
                    .map_or(12., |v| v.as_f64().unwrap()),
                project: |point: Point3| {
                    let xyz = point.to_array();
                    let h: [Real; 4] = std::array::from_fn(|i| {
                        matrix[i][3] + (0..3).map(|j| matrix[i][j] * xyz[j]).sum::<Real>()
                    });
                    (h[3] > 0.).then_some([h[0] / h[3], h[1] / h[3]])
                },
            };
            for (wires, picks) in value["topology_wires"]
                .as_array()
                .unwrap()
                .iter()
                .zip(picks["sources"].as_array().unwrap())
            {
                let Some(wires) = wires.as_array() else {
                    continue;
                };
                let picks = picks.as_array().unwrap();
                assert_eq!(wires.len(), picks.len());
                for (wire, pick) in wires.iter().zip(picks) {
                    let ends: [[Real; 3]; 2] = serde_json::from_value(wire.clone()).unwrap();
                    let [a, b] = ends.map(|p| Point3::try_from(p).unwrap());
                    let actual = match capture_mesh(a, b, &metric) {
                        Capture::Point(point) => metric.captured_distance(point).map(|_| point),
                        Capture::Miss => None,
                        Capture::Unresolved => {
                            panic!("fully visible reference wire unresolved: {}", op["id"])
                        }
                    };
                    if pick.is_null() {
                        assert!(actual.is_none(), "{}", op["id"]);
                    } else {
                        let t = pick["t"].as_f64().unwrap();
                        assert!((0. ..=1.).contains(&t));
                        let expected = Point3::try_from(std::array::from_fn(|i| {
                            (1. - t) * ends[0][i] + t * ends[1][i]
                        }))
                        .unwrap();
                        assert!(
                            actual.unwrap().distance_to(expected).unwrap() < 1e-9,
                            "{}: {wire}",
                            op["id"]
                        );
                        captured += 1;
                    }
                    tested += 1;
                }
            }
        }
    }
    assert_eq!((tested, captured), (520, 300));
}

use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn json_sweep_point_constraints_reach_sdk_and_command_builders() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle/fixtures");
    let mut count = 0;
    for family in [
        "sweep1_point_ends",
        "sweep1_point_ends_commands",
        "sweep1_point_ends_retained",
        "sweep1_point_ends_closed",
    ] {
        let bytes = std::fs::read(root.join(format!("{family}.json"))).unwrap();
        let input: Value = serde_json::from_slice(&bytes).unwrap();
        let request: ProbeRequest = serde_json::from_slice(&bytes).unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(
            response.results.len(),
            input["operations"].as_array().unwrap().len()
        );
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(response.results)
        {
            assert_eq!(op["id"], row.id);
            assert_eq!(row.value.as_array().unwrap().len(), 1);
            let definition = &row.value[0]["definition"];
            let nu = definition["control_count"][0].as_u64().unwrap() as usize;
            let nv = definition["control_count"][1].as_u64().unwrap() as usize;
            for (key, index) in [("start_point", 0), ("end_point", nu - 1)] {
                if let Some(point) = op.get(key) {
                    for v in 0..nv {
                        assert_eq!(
                            &definition["control_points"][v * nu + index]["point"],
                            point,
                            "{} {key}",
                            op["id"]
                        );
                    }
                }
            }
            count += 1;
        }
    }
    assert_eq!(count, 20);
}

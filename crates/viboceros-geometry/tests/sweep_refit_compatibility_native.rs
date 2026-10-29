//! Relate public MakeCompatible output to complete native Sweep boundary curves.
use serde_json::{Value, json};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

fn read(path: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(&std::fs::read(root.join(path)).unwrap()).unwrap()
}

#[test]
fn public_compatibility_results_equal_complete_native_sweep_boundary_definitions() {
    let captured = read("observations/sweep1_refit_compatibility_stages.json");
    let sdk = read("observations/sweep1_refit_sdk.json");
    let holdouts = read("observations/sweep1_refit_sdk_holdouts.json");
    let reentry = read("observations/sweep1_refit_stage_reentry.json");
    let mut checked = 0;
    for row in captured["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap().split("-angle-").next().unwrap();
        let (base, c1) = id
            .strip_suffix("-c1")
            .map(|base| (base, true))
            .unwrap_or_else(|| (id.strip_suffix("-original").unwrap(), false));
        for record in row["value"]["refit_sources"].as_array().unwrap() {
            if record.get("curves").is_none() || record["simplified"] != false {
                continue;
            }
            let count = record["source_count"].as_u64().unwrap();
            // Existing Sweep captures include single originals and both C1
            // section-count contexts. Original pairs are new diagnostic inputs.
            if !c1 && count != 1 {
                continue;
            }
            let name = if c1 {
                format!("{base}-c1-{}", if count == 1 { "single" } else { "pair" })
            } else {
                base.to_owned()
            };
            let native = [&sdk, &holdouts, &reentry]
                .into_iter()
                .flat_map(|family| family["results"].as_array().unwrap())
                .find(|row| row["id"] == name)
                .unwrap();
            let faces = native["value"].as_array().unwrap();
            assert_eq!(faces.len(), 1, "{name}");
            let surface = &faces[0]["definition"];
            let nu = surface["control_count"][0].as_u64().unwrap() as usize;
            let nv = surface["control_count"][1].as_u64().unwrap() as usize;
            for (index, curve) in record["curves"].as_array().unwrap().iter().enumerate() {
                let u = if index == 0 { 0 } else { nu - 1 };
                let boundary = json!({
                    "degree":surface["degree"][1],"knots":surface["knots_v"],
                    "control_points":(0..nv).map(|v| &surface["control_points"][v * nu + u])
                        .collect::<Vec<_>>()
                });
                // Equality of all stored controls, weights, degrees and knots
                // proves the entire curve, independently of sampling density.
                assert_eq!(
                    definitions::curve(curve),
                    definitions::curve(&boundary),
                    "{name} {index}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 32);
    println!("COMPATIBILITY_SWEEP_BOUNDARIES exact_complete_curves={checked}");
}

#[test]
fn zero_and_one_degree_angles_retain_all_compatible_curve_definitions() {
    let captured = read("observations/sweep1_refit_compatibility_stages.json");
    let rows = captured["results"].as_array().unwrap();
    assert_eq!(rows.len(), 16);
    let mut checked = 0;
    for pair in rows.chunks_exact(2) {
        let records = |row: &Value| {
            row["value"]["refit_sources"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|record| record.get("curves"))
                .cloned()
                .collect::<Vec<_>>()
        };
        let (zero, default) = (records(&pair[0]), records(&pair[1]));
        assert_eq!(zero, default, "{}", pair[0]["id"]);
        checked += zero
            .iter()
            .map(|curves| curves.as_array().unwrap().len())
            .sum::<usize>();
    }
    assert_eq!(checked, 48);
}

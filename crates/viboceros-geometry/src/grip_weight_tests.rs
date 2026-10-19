//! Full definitions from public Weight commands, including periodic aliases.
use crate::grip_deletion_tests::{curve, curve_value, difference, surface, surface_value};
use serde_json::Value;
use std::collections::BTreeSet;

fn check(family: &str) {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("fixtures/grip_weight.json")).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("observations/grip_weight.json")).unwrap(),
    )
    .unwrap();
    let mut count = 0;
    let mut errors = Vec::new();
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        for step in row["value"]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["command"] == "Weight")
        {
            let inputs = step["getter"]["inputs"].as_array().unwrap();
            let value = inputs
                .last()
                .and_then(|v| {
                    if v == "Enter" {
                        Some(step["getter"]["default"].as_f64().unwrap_or(1.))
                    } else {
                        v.as_f64()
                    }
                })
                .filter(|v| *v > 0.);
            for owner in step["before"]["objects"].as_array().unwrap() {
                if owner.get(family).is_none() || owner["kind"] == "PolyCurve" {
                    continue;
                }
                let grips = owner["grips"].as_array().unwrap();
                let selected = grips
                    .iter()
                    .filter(|g| {
                        g["hidden"] == false
                            && g["weight"].as_f64().unwrap() > 0.
                            && (g["selected"] == true
                                || step["getter"]["selection"] == true && op["pick"] == true)
                    })
                    .map(|g| g["index"].as_u64().unwrap() as usize)
                    .collect::<BTreeSet<_>>();
                let actual = match family {
                    "curve" => {
                        let c = curve(&owner[family]);
                        curve_value(&if let Some(w) = value {
                            c.with_grip_weight(&selected, w).unwrap()
                        } else {
                            c
                        })
                    }
                    "surface" => {
                        let s = surface(&owner[family]);
                        surface_value(&if let Some(w) = value {
                            s.with_grip_weight(&selected, w).unwrap()
                        } else {
                            s
                        })
                    }
                    _ => unreachable!(),
                };
                let expected = step["after_script"]["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| o["role"] == owner["role"])
                    .unwrap();
                if let Some(error) = difference(
                    &actual,
                    &expected[family],
                    &format!("{}:{}", row["id"], owner["role"]),
                ) {
                    errors.push(error);
                }
                count += 1;
            }
        }
    }
    assert!(count > 30, "insufficient {family} witnesses");
    assert!(
        errors.is_empty(),
        "{family} weight differences ({}/{count}):\n{}",
        errors.len(),
        errors.join("\n")
    );
}

#[test]
fn native_weight_curve_bases_and_seam_aliases() {
    check("curve");
}
#[test]
fn native_weight_surface_tensors() {
    check("surface");
}

//! Correspond only axis exchanges of the same rectangular planar support.
use serde_json::Value;
pub(super) fn align(actual: &mut Value, native: &Value) {
    match (actual, native) {
        (Value::Array(a), Value::Array(b)) => {
            for (a, b) in a.iter_mut().zip(b) {
                align(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.get("solid") == Some(&Value::Bool(true)) && a.contains_key("brep") {
                let before = a["brep"]["faces"][0]["definition"].clone();
                let target = &b["brep"]["faces"][0]["definition"];
                assert_eq!(before["degree"], serde_json::json!([1, 1]));
                assert_eq!(before["control_count"], serde_json::json!([2, 2]));
                let points = |v: &Value| {
                    v["control_points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap())
                        .collect::<Vec<_>>()
                };
                let source = points(&before);
                let dest = points(target);
                let subtract =
                    |a: [f64; 3], b: [f64; 3]| std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
                let dot = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| a[i] * b[i]).sum::<f64>();
                let permutation = dest
                    .iter()
                    .map(|p| {
                        source
                            .iter()
                            .position(|q| dot(subtract(*p, *q), subtract(*p, *q)) < 1e-12)
                            .unwrap_or_else(|| {
                                panic!("same rectangular cap support: {source:?} versus {dest:?}")
                            })
                    })
                    .collect::<Vec<_>>();
                assert!(
                    [[0, 1, 2, 3], [1, 3, 0, 2], [3, 2, 1, 0], [2, 0, 3, 1]]
                        .contains(&permutation.as_slice().try_into().unwrap())
                );
                if permutation == [0, 1, 2, 3] {
                    return;
                }
                let domain =
                    |v: &Value, k: &str| serde_json::from_value::<[f64; 2]>(v[k].clone()).unwrap();
                let [su, sv] = [domain(&before, "domain_u"), domain(&before, "domain_v")];
                let [du, dv] = [domain(target, "domain_u"), domain(target, "domain_v")];
                let axes = [subtract(dest[1], dest[0]), subtract(dest[2], dest[0])];
                let mut definition = before.clone();
                definition["control_points"] = serde_json::json!(
                    permutation
                        .iter()
                        .map(|&i| before["control_points"][i].clone())
                        .collect::<Vec<_>>()
                );
                for k in ["domain_u", "domain_v", "knots_u", "knots_v"] {
                    definition[k] = target[k].clone();
                }
                for boundary in a["brep"]["faces"][0]["loops"].as_array_mut().unwrap() {
                    for trim in boundary.as_array_mut().unwrap() {
                        for control in trim["definition"]["control_points"].as_array_mut().unwrap()
                        {
                            let uv = serde_json::from_value::<[f64; 2]>(control["point"].clone())
                                .unwrap();
                            let fractions = [
                                (uv[0] - su[0]) / (su[1] - su[0]),
                                (uv[1] - sv[0]) / (sv[1] - sv[0]),
                            ];
                            let world = std::array::from_fn(|i| {
                                source[0][i]
                                    + fractions[0] * (source[1][i] - source[0][i])
                                    + fractions[1] * (source[2][i] - source[0][i])
                            });
                            let relative = subtract(world, dest[0]);
                            control["point"] = serde_json::json!([
                                du[0]
                                    + dot(relative, axes[0]) / dot(axes[0], axes[0])
                                        * (du[1] - du[0]),
                                dv[0]
                                    + dot(relative, axes[1]) / dot(axes[1], axes[1])
                                        * (dv[1] - dv[0])
                            ]);
                        }
                        // Iso classifications follow the physical cap side.
                        let iso = trim["iso"].as_i64().unwrap();
                        if iso != 0 {
                            panic!(
                                "axis correspondence of isoparametric cap trims requires explicit mapping"
                            );
                        }
                    }
                }
                a["brep"]["faces"][0]["definition"] = definition.clone();
                a["surfaces"][0] = definition;
            } else {
                for (key, value) in a {
                    align(value, &b[key]);
                }
            }
        }
        _ => {}
    }
}

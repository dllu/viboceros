//! Arc and Circle replay the same contacts; Arc adds a final interval choice.
use super::*;
pub(super) fn run(
    f: &crate::curve_getters::Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    crate::curve_getters::run(f, iterations, tolerance, |d, c, p, a, l, g| {
        crate::circle_tangent::enter(d, c, p, a, l, g, true)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arc_tangent_native_endpoint_diagnostics_exceed_the_parity_threshold() {
        let mut q: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/arc_tangent_numeric_diagnostics.json"
        ))
        .unwrap();
        let n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/arc_tangent_numeric_diagnostics_rhino_20261017.json"
        ))
        .unwrap();
        for (op, row) in q["operations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(n["results"].as_array().unwrap())
        {
            if let Some(clicks) = row["value"]["input_trace"]["clicks"].as_array() {
                for c in clicks {
                    let i = c["action"].as_u64().unwrap() as usize;
                    op["actions"][i]["pick"] = c["projected_point"].clone();
                }
            }
        }
        let actual = run_request(&serde_json::from_value(q).unwrap()).unwrap();
        assert_eq!(actual.results.len(), 7);
        for i in 0..7 {
            let a = &actual.results[i].value["after"]["outputs"][0]["curve"]["control_points"];
            let b =
                &n["results"][i]["value"]["after_script"]["outputs"][0]["curve"]["control_points"];
            let mut error = 0f64;
            for (a, b) in [
                (
                    a.as_array().unwrap().first().unwrap(),
                    b.as_array().unwrap().first().unwrap(),
                ),
                (
                    a.as_array().unwrap().last().unwrap(),
                    b.as_array().unwrap().last().unwrap(),
                ),
            ] {
                let a: [f64; 3] = serde_json::from_value(a["point"].clone()).unwrap();
                let b: [f64; 3] = serde_json::from_value(b["point"].clone()).unwrap();
                error = error.max((0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt());
            }
            assert!(error > 1e-6, "{}: {error}", actual.results[i].id);
            eprintln!(
                "ARC_NUMERIC {}",
                json!({"id":actual.results[i].id,"endpoint_error":error,"local_controls":a.as_array().unwrap().len(),"native_controls":b.as_array().unwrap().len()})
            );
        }
    }
    #[test]
    fn arc_tangent_native_intervals_full_nurbs_selection_and_history() {
        for (q, n) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_tangent.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_tangent_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_basic.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_basic_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_modes_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_spatial_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_memory.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_memory_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_curved.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_curved_rhino_20261017.json"
                ),
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_tangent_curved_variants.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_curved_variants_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_tolerance.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_tolerance_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_pair_types.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_pair_types_rhino_20261017.json"
                ),
            ),
        ] {
            let mut q: Value = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            for (op, row) in q["operations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(n["results"].as_array().unwrap())
            {
                // Native pixel rays resolve input contacts independently of
                // the expected output. Production artifacts stay unchanged.
                if let Some(clicks) = row["value"]["input_trace"]["clicks"].as_array() {
                    for c in clicks {
                        let i = c["action"].as_u64().unwrap() as usize;
                        assert!(c["projected_point"].is_array());
                        op["actions"][i]["pick"] = c["projected_point"].clone();
                    }
                }
            }
            let actual = run_request(&serde_json::from_value(q).unwrap()).unwrap();
            for (a, b) in actual.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    crate::curve_getters::assert_close(
                        &a.value[stage],
                        &b["value"][if stage == "after" {
                            "after_script"
                        } else {
                            stage
                        }],
                        &format!("{}/{stage}", a.id),
                    );
                }
                let label = if b["value"]["after_script"]["outputs"]
                    .as_array()
                    .unwrap()
                    .is_empty()
                {
                    Value::Null
                } else {
                    json!("Arc")
                };
                assert_eq!(a.value["undo_label"], label);
                assert_eq!(a.value["redo_label"], label);
            }
        }
    }
}

//! Exact-equivalent inputs, experimental budgets and supplied-basis outputs.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn data(family: &str) -> (Value, Value) {
    let q = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tools/rhino_oracle/fixtures/{family}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    (q, captured::read(family))
}
fn interpolate(source: &NurbsSurface, target: &NurbsSurface) -> NurbsSurface {
    let p = [target.degree_u(), target.degree_v()];
    let k = [target.knots_u(), target.knots_v()];
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::interpolate_surface_in_basis(source, p, k).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_interpolate_surface_in_basis(source, p, k).unwrap()
    }
}
fn bound(a: &NurbsSurface, b: &NurbsSurface) -> f64 {
    a.parameter_surface_deviation_bound(b, 1e-9)
        .unwrap()
        .unwrap()
}

#[test]
fn native_budget_observations_keep_geometry_equivalence_and_nulls_explicit() {
    let mut totals = [0; 3];
    for (family, expected_count) in [
        ("surface_fit_capacity", 38),
        ("surface_fit_capacity_holdouts", 19),
    ] {
        let (q, n) = data(family);
        let operations = q["operations"].as_array().unwrap();
        let rows = n["results"].as_array().unwrap();
        assert_eq!(operations.len(), expected_count);
        assert_eq!(rows.len(), expected_count);
        for (op, row) in operations.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            let v = &row["value"];
            assert_eq!(v["before"], v["after"]);
            let source = captured::surface(&v["before"]);
            let before = source.clone();
            let authored = captured::surface(&op["surface"]);
            let authored_bound = bound(&source, &authored);
            let label = op["id"].as_str().unwrap();
            let base = if label.contains("holdout-linear-") {
                "fit-capacity-holdout-linear-base".to_string()
            } else if label.starts_with("fit-capacity-holdout-") {
                let pieces = label.split('-').collect::<Vec<_>>();
                format!("fit-capacity-holdout-{}-{}-base", pieces[3], pieces[4])
            } else if label.contains("polynomial-") {
                "fit-capacity-polynomial-base-0.01".to_string()
            } else if label.contains("lower-degree") {
                label.to_string()
            } else {
                format!("fit-capacity-rational-bezier-base-{}", op["fit_tolerance"])
            };
            let base = rows
                .iter()
                .find(|r| r["id"] == base)
                .unwrap_or_else(|| panic!("missing base for {label}"));
            let equivalent = bound(&source, &captured::surface(&base["value"]["before"]));
            let degree = [
                op["degree_u"].as_u64().unwrap() as usize,
                op["degree_v"].as_u64().unwrap() as usize,
            ];
            let budget = SurfaceFitRefinementBudget::try_from_surface(&source, degree).unwrap();
            let mut native_bound = None;
            let mut archive_bound = None;
            let mut count = None;
            let mut admitted = None;
            if !v["surface"].is_null() {
                let target = captured::surface(&v["surface"]);
                let counts = [
                    target.control_point_count_u(),
                    target.control_point_count_v(),
                ];
                count = Some(counts);
                admitted = budget.map(|b| b.admits_control_count(counts));
                if budget.is_some() {
                    assert_eq!(admitted, Some(true), "{label}: budget counterexample");
                }
                let actual = interpolate(&source, &target);
                native_bound = Some(bound(&actual, &target));
                let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
                    actual.clone(),
                    actual.domain_u(),
                    actual.domain_v(),
                    false,
                    captured::model_tolerance(),
                )
                .unwrap();
                archive_bound = Some(bound(
                    &actual,
                    captured::archive(&brep).faces()[0].surface(),
                ));
                totals[1] += 1;
            } else {
                totals[2] += 1;
            }
            assert_eq!(source, before);
            totals[0] += 1;
            println!(
                "FIT_CAPACITY {}",
                json!({"id":op["id"],"source_count":[source.control_point_count_u(),source.control_point_count_v()],
                "source_bound":equivalent,"authored_bound":authored_bound,"maximum_controls":budget.map(|b|b.maximum_control_count()),
                "native_count":count,"candidate_admitted":admitted,"native_bound":native_bound,"archive_bound":archive_bound})
            );
        }
    }
    assert_eq!(totals, [57, 18, 39]);
    println!(
        "FIT_CAPACITY_TOTAL {}",
        json!({"observations":57,"captured_bases":18,"native_null":39,"adaptive_fit_parity":false})
    );
}

#[test]
fn all_prior_equal_degree_outputs_are_eligible_without_inventing_degree_change_rules() {
    let mut qualified = 0;
    let mut unknown = 0;
    for family in ["surface_fit", "surface_fit_adaptive", "surface_fit_strips"] {
        let (q, n) = data(family);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(n["results"].as_array().unwrap())
        {
            if row["value"]["surface"].is_null() {
                continue;
            }
            let source = captured::surface(&row["value"]["before"]);
            let target = captured::surface(&row["value"]["surface"]);
            let p = [
                op["degree_u"].as_u64().unwrap() as usize,
                op["degree_v"].as_u64().unwrap() as usize,
            ];
            let budget = SurfaceFitRefinementBudget::try_from_surface(&source, p).unwrap();
            let admitted = budget.map(|b| {
                b.admits_control_count([
                    target.control_point_count_u(),
                    target.control_point_count_v(),
                ])
            });
            if budget.is_some() {
                assert_eq!(admitted, Some(true), "{}", op["id"]);
                qualified += 1;
            } else {
                unknown += 1;
            }
            println!(
                "PRIOR_FIT_CAPACITY {}",
                json!({"id":op["id"],"maximum_controls":budget.map(|b|b.maximum_control_count()),"candidate_admitted":admitted})
            );
        }
    }
    assert_eq!(qualified + unknown, 90);
    println!(
        "PRIOR_FIT_CAPACITY_TOTAL {}",
        json!({"qualified":qualified,"degree_change_unknown":unknown})
    );
}

#[test]
fn budget_protocol_returns_unknown_for_degree_changes_and_never_claims_accuracy() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let (q, _) = data("surface_fit_capacity");
    let source = q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == "fit-capacity-rational-bezier-uv-mid-triple-0.01")
        .unwrap()["surface"]
        .clone();
    let mut op = json!({"op":"surface_fit_refinement_budget","id":"budget","surface":source,"degree_u":3,"degree_v":3,"candidate_controls":[5,5]});
    let call = |op: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[op]}),
            )
            .unwrap(),
        )
    };
    assert_eq!(
        call(&op).unwrap().results[0].value,
        json!({"supported":true,"initial_controls":[4,4],"maximum_controls":[5,5],"candidate_admitted":false})
    );
    op["candidate_controls"] = json!([5, 4]);
    assert_eq!(
        call(&op).unwrap().results[0].value["candidate_admitted"],
        true
    );
    op.as_object_mut().unwrap().remove("candidate_controls");
    assert_eq!(
        call(&op).unwrap().results[0].value["candidate_admitted"],
        Value::Null
    );
    op["degree_u"] = json!(2);
    assert_eq!(
        call(&op).unwrap().results[0].value,
        json!({"supported":false,"initial_controls":null,"maximum_controls":null,"candidate_admitted":null})
    );
    op["degree_u"] = json!(12);
    assert!(call(&op).is_err());
}

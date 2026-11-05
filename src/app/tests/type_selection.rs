use super::*;
use viboceros_oracle::type_selection::{Fixture, setup, snapshot};

#[test]
fn type_selection_matches_native_block_and_extrusion_workflows() {
    replay("type_selection_clean", 35);
    replay("type_selection_empty", 8);
}
fn replay(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = super::oracle_observation::read(&base, family);
    let ops = request["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), count);
    assert_eq!(ops.len(), rows.len());
    for (op, row) in ops.iter().zip(rows) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let (doc, scene) = setup(&f, app.document.tolerance()).unwrap();
        app.document = doc;
        {
            let mut check = |phase, input: Option<&str>| {
                if let Some(input) = input {
                    app.command_input = input.into();
                    app.run_command();
                }
                let actual = snapshot(&app.document, &scene).unwrap();
                super::edge_analysis_mixed_support::compare(
                    &actual,
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                );
                assert_eq!(actual["geometry_unchanged"], true);
            };
            let input = f.input().unwrap();
            check("before", None);
            check("after", Some(&input));
            check("repeat", Some(&input));
        }
        app.command_input = "SelNone".into();
        app.run_command();
        for (phase, input) in [("previous", "SelPrev"), ("redo", "Redo")] {
            app.command_input = input.into();
            app.run_command();
            super::edge_analysis_mixed_support::compare(
                &snapshot(&app.document, &scene).unwrap(),
                &row["value"][phase],
                &format!("{}/{phase}", op["id"]),
            );
        }
    }
}

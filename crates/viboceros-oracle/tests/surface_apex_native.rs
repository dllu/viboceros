#![cfg(feature = "native-smlib")]
#[path = "support/compare.rs"]
mod diagnostic_comparison;
#[path = "support/surface_apex.rs"]
mod support;
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, SelectionMode};
use viboceros_geometry::Tolerance;
use viboceros_oracle::{ProbeRequest, SurfaceApexExtrusionFixture, run_request};

#[test]
fn surface_apex_sdk_matches_licensed_geometry() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_apex_extrusion.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_apex_sdk_rhino_20261024.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 9);
    assert_eq!(native["results"].as_array().unwrap().len(), 9);
    for (row, expected) in actual
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(row.id, expected["id"]);
        let error = support::compare(&row.value, &expected["value"]);
        println!(
            "SURFACE_APEX_SDK {}",
            json!({"id":row.id,"value":row.value,"maximum_error":error})
        );
    }
}

#[test]
fn surface_apex_commands_match_licensed_geometry_and_history() {
    replay_commands(support::cases(), "SURFACE_APEX_COMMAND");
}

#[test]
fn surface_apex_tangents_match_licensed_geometry_and_history() {
    replay_commands(support::tangent_cases(), "SURFACE_APEX_TANGENTS_COMMAND");
}

fn replay_commands(cases: Vec<(Value, Value)>, prefix: &str) {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (op, native) in cases {
        let fixture: SurfaceApexExtrusionFixture = serde_json::from_value(op.clone()).unwrap();
        let original = fixture.source_face(tolerance).unwrap();
        let mut doc = Document::new(tolerance);
        let source = doc.add_geometry(original.clone()).unwrap();
        if op["preselect"] == true {
            doc.select_objects_direct([source], SelectionMode::Replace)
                .unwrap();
        }
        doc.clear_history().unwrap();
        let [x, y, z] = fixture.apex;
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfToPoint {x},{y},{z} FaceRefs={source}:0 Solid={} DeleteInput={} SplitAtTangents={}",
                    if fixture.solid { "Yes" } else { "No" },
                    if op["delete_input"] == true {
                        "Yes"
                    } else {
                        "No"
                    },
                    if op["split_at_tangents"] == true { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        let after = support::measure(&doc, source, &original, &native["after"]);
        registry.execute(&mut doc, "Undo").unwrap();
        let undo = support::measure(&doc, source, &original, &native["undo"]);
        registry.execute(&mut doc, "Redo").unwrap();
        let redo = support::measure(&doc, source, &original, &native["redo"]);
        println!(
            "{prefix} {}",
            json!({"id":op["id"],"after":after,"undo":undo,"redo":redo})
        );
    }
}

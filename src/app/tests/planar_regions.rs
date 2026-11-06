use super::planar_regions_comparison as comparison;
use super::*;
use viboceros_oracle::planar_regions::{Fixture, snapshot};
#[test]
fn planar_regions_application_commands_match_native_regions_and_history() {
    family("planar_regions_holdouts", 69);
    family("planar_regions_confirmations", 32);
    family("planar_regions_planes", 8);
    family("planar_crossings", 48);
    family("planar_crossings_details", 8);
    family("planar_crossings_holdouts", 33);
}
fn family(family: &str, expected_count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = super::oracle_observation::read(&base, family);
    let mut count = 0;
    let mut area_gaps = 0;
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        count += 1;
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
        let (doc, ids) = f.setup(tol).unwrap();
        let sources = doc
            .objects()
            .map(|o| {
                let Geometry::NurbsCurve(c) = o.geometry() else {
                    panic!("source")
                };
                c.clone()
            })
            .collect::<Vec<_>>();
        let mut app = test_app();
        app.document = doc;
        let before = snapshot(&app.document, &ids, &sources).unwrap();
        app.command_input = "PlanarSrf".into();
        app.run_command();
        assert_eq!(app.document.undo_label(), Some("PlanarSrf"), "{}", op["id"]);
        let after = snapshot(&app.document, &ids, &sources).unwrap();
        app.command_input = "Undo".into();
        app.run_command();
        let undo = snapshot(&app.document, &ids, &sources).unwrap();
        app.command_input = "Redo".into();
        app.run_command();
        let redo = snapshot(&app.document, &ids, &sources).unwrap();
        area_gaps += comparison::compare(&serde_json::json!({"success":true,"before":before,"after":after,"undo":undo,"redo":redo}),&row["value"],1e-9,op["id"].as_str().unwrap()).unwrap_or_else(|e|panic!("{}: {e}",op["id"]));
        assert!(app.active_command.is_none());
    }
    assert_eq!(count, expected_count);
    assert_eq!(
        area_gaps,
        if family == "planar_crossings_holdouts" {
            2
        } else {
            0
        }
    );
}

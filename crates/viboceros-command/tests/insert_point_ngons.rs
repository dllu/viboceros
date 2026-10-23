#[allow(dead_code)]
#[path = "support/insert_point.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::json;
use viboceros_command::{CommandRegistry, insert_point::InsertSession};

#[test]
fn original_ngon_adjacency_and_new_large_regions_match_independent_native_cases() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_point_ngons.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        eprintln!("N-gon {}", row["id"]);
        let v = &row["value"];
        let (mut document, ids) = native::setup_from_family(row, "insert_point_ngons");
        let original = document.object(ids[0]).unwrap().geometry().clone();
        let mut session = InsertSession::new(&document, ids[0]).unwrap();
        assert!(fixtures::stage(&mut session, &document, v));
        let mut preview = document.clone();
        session.apply_preview(&mut preview).unwrap();
        fixtures::check(&preview, &ids, &v["after"], true);
        assert_eq!(document.object(ids[0]).unwrap().geometry(), &original);
        session.finish(&mut document).unwrap();
        let comparison = fixtures::check(&document, &ids, &v["after"], true);
        fixtures::assert_ngon_cell_gap(row["id"].as_str().unwrap(), &comparison);
        let (mut scripted, script_ids) = native::setup_from_family(row, "insert_point_ngons");
        CommandRegistry::with_builtins()
            .execute(&mut scripted, &fixtures::script(v))
            .unwrap();
        assert_eq!(
            fixtures::check(&scripted, &script_ids, &v["after"], true),
            comparison
        );
        assert_eq!(document.undo_label(), Some("InsertPoint"));
        document.undo().unwrap();
        fixtures::check(&document, &ids, &v["undo"], false);
        document.redo().unwrap();
        fixtures::check(&document, &ids, &v["redo"], false);
        println!(
            "INSERT_POINT_NGON {}",
            json!({"id":row["id"],"comparison":comparison,"identity":true,"metadata":true,"history":true,"preview":true,"changed":true})
        );
    }
}

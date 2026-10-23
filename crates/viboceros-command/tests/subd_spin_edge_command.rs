#[allow(dead_code)]
#[path = "support/subd_spin_edge.rs"]
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
use viboceros_command::subd_spin_edge::{Direction, SpinSession};
#[test]
fn cross_object_local_undo_restores_each_source_instead_of_transplanting_native_geometry() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subd_spin_edge_live_v5.json"
    ))
    .unwrap();
    let row = capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "spin_live_v5_undo_both")
        .unwrap();
    let v = &row["value"];
    let after = v["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["source"] == 1)
        .unwrap();
    assert_eq!(
        after["subd"]["points"][0],
        v["before"][0]["subd"]["points"][0]
    );
    assert_ne!(
        after["subd"]["points"][0],
        v["before"][1]["subd"]["points"][0]
    );
    let (mut doc, ids) = native::setup_from_family(row, "subd_spin_edge_live_v5");
    let original = ids
        .iter()
        .map(|&id| doc.object(id).unwrap().geometry().clone())
        .collect::<Vec<_>>();
    let mut session = SpinSession::new(&doc, Direction::default());
    fixtures::replay(&mut session, &doc, &ids, v);
    assert_eq!(session.spin_count(), 0);
    session.finish(&mut doc).unwrap();
    for (&id, geometry) in ids.iter().zip(&original) {
        assert_eq!(doc.object(id).unwrap().geometry(), geometry);
    }
    assert_eq!(doc.undo_label(), Some("SubDSpinEdge"));
    doc.undo().unwrap();
    native::check(&doc, &v["before"], true);
    doc.redo().unwrap();
    for (&id, geometry) in ids.iter().zip(&original) {
        assert_eq!(doc.object(id).unwrap().geometry(), geometry);
    }
    println!(
        "SUBD_SPIN_UNDO_GAP {}",
        json!({"id":row["id"],"native_match":false,"source_locations_restored":true,"history":true})
    );
}
#[test]
fn staged_spins_match_native_surfaces_metadata_order_and_local_undo_history() {
    let rows = fixtures::rows();
    assert_eq!(rows.len(), 31);
    let mut failures = Vec::new();
    for (family, row) in rows {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let v = &row["value"];
            let (mut doc, ids) = native::setup_from_family(&row, family);
            let mut maximum = native::check(&doc, &v["before"], true);
            let attributes = ids
                .iter()
                .map(|&id| doc.object(id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            let mut session = SpinSession::new(&doc, Direction::default());
            fixtures::replay(&mut session, &doc, &ids, v);
            // Staging does not consume the existing Undo or Redo branch.
            maximum = maximum.max(native::check(&doc, &v["before"], true));
            let mut preview = doc.clone();
            session.apply_preview(&mut preview).unwrap();
            maximum = maximum.max(native::check(&preview, &v["after"], true));
            let out = session.finish(&mut doc).unwrap();
            maximum = maximum.max(native::check(&doc, &v["after"], true));
            for (&id, attrs) in ids.iter().zip(attributes) {
                assert_eq!(doc.object(id).unwrap().attributes(), &attrs);
            }
            assert_eq!(
                doc.undo_label(),
                Some(if out.processed_objects.is_empty() {
                    "Undo witness"
                } else {
                    "SubDSpinEdge"
                })
            );
            doc.clear_selection();
            maximum = maximum.max(native::check(&doc, &v["after_script"], true));
            doc.undo().unwrap();
            maximum = maximum.max(native::check(&doc, &v["undo"], true));
            doc.redo().unwrap();
            maximum = maximum.max(native::check(&doc, &v["redo"], true));
            println!(
                "SUBD_SPIN_COMMAND {}",
                json!({"id":row["id"],"maximum_error":maximum,"identity":true,"metadata":true,"history":true,"preview":true,"processed_objects":out.processed_objects.len()})
            );
        }));
        if result.is_err() {
            failures.push(row["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(
        failures.is_empty(),
        "SubD edge spin command differences: {failures:?}"
    );
}

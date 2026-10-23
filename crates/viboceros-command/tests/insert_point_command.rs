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
fn command_and_staged_admission_preserve_native_geometry_metadata_and_history() {
    for row in fixtures::rows() {
        eprintln!("Replaying {}", row["id"]);
        let v = &row["value"];
        let (mut doc, ids) = native::setup_from_family(&row, "insert_point_full");
        let original = doc.object(ids[0]).unwrap().geometry().clone();
        let mut session = InsertSession::new(&doc, ids[0]).unwrap();
        let accept = fixtures::stage(&mut session, &doc, v);
        let mut preview = doc.clone();
        if accept {
            session.apply_preview(&mut preview).unwrap();
        } else {
            preview.clear_selection();
        }
        assert_eq!(doc.object(ids[0]).unwrap().geometry(), &original);
        fixtures::check(&preview, &ids, &v["after"], true);
        if accept {
            session.finish(&mut doc).unwrap();
        } else {
            doc.clear_selection();
        }
        let staged = fixtures::check(&doc, &ids, &v["after"], true);
        let (mut scripted, script_ids) = native::setup_from_family(&row, "insert_point_full");
        CommandRegistry::with_builtins()
            .execute(&mut scripted, &fixtures::script(v))
            .unwrap();
        let check = fixtures::check(&scripted, &script_ids, &v["after"], true);
        assert_eq!(check, staged);
        fixtures::assert_known_cell_gap(row["id"].as_str().unwrap(), &check);
        assert_eq!(scripted.object(script_ids[0]).unwrap().id(), script_ids[0]);
        let changed = doc.undo_label() == Some("InsertPoint");
        assert_eq!(
            changed,
            v["undo"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["name"] == "Undo witness")
        );
        assert!(doc.undo().unwrap().is_some());
        fixtures::check(&doc, &ids, &v["undo"], false);
        assert!(doc.redo().unwrap().is_some());
        fixtures::check(&doc, &ids, &v["redo"], false);
        println!(
            "INSERT_POINT_COMMAND {}",
            json!({"id":row["id"],"comparison":check,"identity":true,"metadata":true,"history":true,"preview":true,"changed":changed})
        );
    }
}
#[test]
fn spatial_ngon_seam_and_original_edge_holdouts_retain_all_geometry_diagnostics() {
    use viboceros_document::Geometry;
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_point_holdouts_v2.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        eprintln!("Holdout {}", row["id"]);
        let v = &row["value"];
        let (mut doc, ids) = native::setup_from_family(row, "insert_point_holdouts_v2");
        let mut session = InsertSession::new(&doc, ids[0]).unwrap();
        assert!(fixtures::stage(&mut session, &doc, v));
        session.finish(&mut doc).unwrap();
        native::check_metadata(&doc, &v["after"], true);
        let target = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["source"] == 0)
            .unwrap();
        let check = match doc.object(ids[0]).unwrap().geometry() {
            Geometry::Mesh(m) => fixtures::compare_mesh(m, &target["mesh"]),
            Geometry::SubD(s) => subd_native::compare(s, &target["subd"]),
            _ => panic!(),
        };
        if let Geometry::Mesh(_) = doc.object(ids[0]).unwrap().geometry() {
            let id = row["id"].as_str().unwrap();
            assert_eq!(check["logical_regions"], true);
            assert_eq!(
                check["stored_pieces_correspond"],
                ![
                    "insert_point_holdout_mesh_ngon_island",
                    "insert_point_holdout_mesh_reverse",
                ]
                .contains(&id)
            );
            assert_eq!(
                check["stored_facets_correspond"],
                id != "insert_point_holdout_mesh_reverse"
            );
        }
        println!(
            "INSERT_POINT_HOLDOUT {}",
            json!({"id":row["id"],"comparison":check,"identity":true,"metadata":true})
        );
    }
}

//! Independent input replay of mesh point paths, with explicit cell diagnostics.
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
use serde_json::{Value, json};
use viboceros_command::insert_point::InsertSession;
use viboceros_document::Geometry;
#[test]
fn authored_and_retained_mesh_paths_report_cells_and_surface_error() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut diagnostics = Vec::new();
    for family in [
        "insert_point_full",
        "insert_point_regions",
        "insert_point_spatial_layouts",
        "insert_point_spatial_prefixes",
        "mesh_insert_paths",
    ] {
        let p = root.join(format!("observations/{family}.json"));
        let record: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        for row in record["results"].as_array().unwrap() {
            if family == "insert_point_regions" && row["id"] != "insert_point_regions_same_reverse"
            {
                continue;
            }
            let v = &row["value"];
            if family == "mesh_insert_paths"
                && (!v["input_trace"][0]["errors"].as_array().unwrap().is_empty()
                    || v["history"]
                        .as_str()
                        .unwrap()
                        .contains("Tessellation failure"))
            {
                continue;
            }
            let target = v["after"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["source"] == 0)
                .unwrap();
            if !target["mesh"].is_object() {
                continue;
            }
            let (doc, ids) = native::setup_from_family(row, family);
            let mut s = InsertSession::new(&doc, ids[0]).unwrap();
            s.prepare_picking().unwrap();
            let accept = fixtures::stage(&mut s, &doc, v);
            let Geometry::Mesh(m) = s.candidate() else {
                panic!()
            };
            let comparison = fixtures::compare_mesh_diagnostic(m, &target["mesh"]);
            diagnostics.push(json!({"family":family,"id":row["id"],"comparison":comparison}));
            println!(
                "MESH_INSERT_PATH {}",
                json!({"family":family,"id":row["id"],"accepted":accept,"comparison":comparison})
            );
        }
    }
    assert_eq!(diagnostics.len(), 48);
    let gaps: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/mesh_insert_retained_gaps.json")).unwrap(),
    )
    .unwrap();
    for field in [
        "stored_pieces_correspond",
        "stored_facets_correspond",
        "qualified",
    ] {
        let actual = diagnostics
            .iter()
            .filter(|r| r["comparison"][field] != true)
            .map(|r| json!([r["family"], r["id"]]))
            .collect::<Vec<_>>();
        assert_eq!(
            json!(actual),
            gaps[field],
            "unexplained tessellation change: {field}"
        );
    }
    for row in &diagnostics {
        assert_eq!(row["comparison"]["channels"], true);
        assert_eq!(row["comparison"]["logical_regions"], true);
        assert_eq!(row["comparison"]["ngon_overlays_correspond"], true);
    }
}

#[test]
fn flat_corner_sources_and_disconnected_getters_match_native_metadata_and_history() {
    use viboceros_command::insert_point::InsertAction;
    use viboceros_geometry::Vector3;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/observations");
    for family in ["mesh_insert_degenerate", "mesh_insert_disconnected"] {
        let capture: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("{family}.json"))).unwrap(),
        )
        .unwrap();
        for row in capture["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let (mut doc, ids) = native::setup_from_family(row, family);
            let original = doc.object(ids[0]).unwrap().geometry().clone();
            let mut session = InsertSession::new(&doc, ids[0]).unwrap();
            session.prepare_picking().unwrap();
            for (i, a) in fixtures::actions(v).into_iter().enumerate() {
                match a {
                    InsertAction::Point(_) => {
                        let pick = fixtures::captured_pick(&session, v, i);
                        let before = session.point_count();
                        let candidate = session.candidate().clone();
                        let result = session.insert(
                            &doc,
                            pick,
                            Some(
                                Vector3::try_new(0., 0., 1.)
                                    .unwrap()
                                    .normalized_nonzero()
                                    .unwrap(),
                            ),
                        );
                        if i == 1 && (name == "both-components" || name.starts_with("islands-")) {
                            assert!(result.is_err(), "{name}");
                            assert_eq!(session.point_count(), before);
                            assert_eq!(session.candidate(), &candidate);
                        } else {
                            result.unwrap();
                        }
                    }
                    InsertAction::Undo => {
                        session.undo_point(&doc).unwrap();
                    }
                    InsertAction::Finish => break,
                    _ => panic!(),
                }
                assert_eq!(doc.object(ids[0]).unwrap().geometry(), &original);
            }
            let mut preview = doc.clone();
            session.apply_preview(&mut preview).unwrap();
            fixtures::check_scene(&preview, &ids, &v["after"], true, family);
            session.finish(&mut doc).unwrap();
            let comparison = fixtures::check_scene(&doc, &ids, &v["after"], true, family);
            doc.undo().unwrap();
            fixtures::check_scene(&doc, &ids, &v["undo"], false, family);
            doc.redo().unwrap();
            fixtures::check_scene(&doc, &ids, &v["redo"], false, family);
            println!(
                "MESH_INSERT_SOURCE {}",
                json!({"family":family,"id":name,"comparison":comparison,"preview":true,"metadata":true,"history":true})
            );
        }
    }
}

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
        replay(row, "insert_point_ngons", "NGON");
    }
}
#[test]
fn skewed_rotated_tall_and_warped_quad_regions_match_native_geometry() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_point_mesh_shapes.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        replay(row, "insert_point_mesh_shapes", "SHAPE");
    }
}
#[test]
fn control_surface_hits_and_camera_target_fallback_match_native_depth_picks() {
    for (family, text, stage) in [
        (
            "insert_point_pick_depth",
            include_str!("../../../tools/rhino_oracle/observations/insert_point_pick_depth.json"),
            "DEPTH",
        ),
        (
            "insert_point_pick_depth_v2",
            include_str!(
                "../../../tools/rhino_oracle/observations/insert_point_pick_depth_v2.json"
            ),
            "DEPTH_V2",
        ),
    ] {
        let capture: serde_json::Value = serde_json::from_str(text).unwrap();
        for row in capture["results"].as_array().unwrap() {
            replay(row, family, stage);
        }
    }
}
#[test]
fn independent_spatial_layouts_keep_geometry_failures_explicit() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_point_spatial_layouts.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        replay(row, "insert_point_spatial_layouts", "SPATIAL");
    }
}
#[test]
fn spatial_prefixes_and_reversed_paths_retain_geometry_diagnostics() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_point_spatial_prefixes.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        replay(row, "insert_point_spatial_prefixes", "PREFIX");
    }
}
fn replay(row: &serde_json::Value, family: &str, stage: &str) {
    eprintln!("N-gon {}", row["id"]);
    let v = &row["value"];
    let (mut document, ids) = native::setup_from_family(row, family);
    let original = document.object(ids[0]).unwrap().geometry().clone();
    let mut session = InsertSession::new(&document, ids[0]).unwrap();
    session.prepare_picking().unwrap();
    let script = if stage != "NGON" {
        fixtures::edge_script(&session, v)
    } else {
        fixtures::script(v)
    };
    assert!(fixtures::stage(&mut session, &document, v));
    let mut preview = document.clone();
    session.apply_preview(&mut preview).unwrap();
    fixtures::check_scene(&preview, &ids, &v["after"], true, family);
    assert_eq!(document.object(ids[0]).unwrap().geometry(), &original);
    session.finish(&mut document).unwrap();
    let comparison = fixtures::check_scene(&document, &ids, &v["after"], true, family);
    if stage == "PREFIX" {
        fixtures::assert_prefix_result(row["id"].as_str().unwrap(), &comparison);
    } else if stage == "NGON" {
        fixtures::assert_ngon_cell_gap(row["id"].as_str().unwrap(), &comparison);
    } else if stage == "SPATIAL" {
        fixtures::assert_spatial_result(row["id"].as_str().unwrap(), &comparison);
    } else {
        fixtures::assert_additional_cell_gap(row["id"].as_str().unwrap(), &comparison);
    }
    let (mut scripted, script_ids) = native::setup_from_family(row, family);
    CommandRegistry::with_builtins()
        .execute(&mut scripted, &script)
        .unwrap();
    assert_eq!(
        fixtures::check_scene(&scripted, &script_ids, &v["after"], true, family),
        comparison
    );
    assert_eq!(document.undo_label(), Some("InsertPoint"));
    document.undo().unwrap();
    fixtures::check_scene(&document, &ids, &v["undo"], false, family);
    document.redo().unwrap();
    fixtures::check_scene(&document, &ids, &v["redo"], false, family);
    println!(
        "INSERT_POINT_{stage} {}",
        json!({"id":row["id"],"comparison":comparison,"identity":true,"metadata":true,"history":true,"preview":true,"changed":true})
    );
}

//! Frontend selection replay against captured native command results.
use super::native::fixture as native;
use super::*;
use serde_json::json;
use std::collections::BTreeSet;
#[path = "../../../../crates/viboceros-command/tests/support/edge_seams.rs"]
mod fixtures;

#[test]
fn edge_getter_ignores_vertex_grips_and_cancels_without_consuming_redo() {
    let (mut app, id) = setup();
    app.document.enable_control_points([id]).unwrap();
    app.document
        .select_control_points(
            [viboceros_document::ControlPointId {
                object: id,
                index: 0,
            }],
            viboceros_document::SelectionMode::Replace,
        )
        .unwrap();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    let before = app.document.object(id).unwrap().geometry().clone();
    enter(&mut app, "UnweldEdge ModifyNormals No");
    assert!(app.crease_prompt.is_some());
    select(&mut app, id, ComponentSelectionKind::SubDVertex, 0, false);
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    select(&mut app, id, ComponentSelectionKind::SubDEdge, 3, false);
    enter(&mut app, "SelNone");
    assert!(app.crease_prompt.is_some());
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.crease_prompt.is_none());
    assert_eq!(app.document.object(id).unwrap().geometry(), &before);
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
}

#[test]
fn edge_getter_rejects_geometry_or_tolerance_changes_before_admission() {
    for geometry in [false, true] {
        let (mut app, id) = setup();
        enter(&mut app, "UnweldEdge");
        select(&mut app, id, ComponentSelectionKind::SubDEdge, 3, false);
        if geometry {
            app.document
                .replace_object_geometries([(id, Geometry::Point(p(8., 9., 10.)))])
                .unwrap();
        } else {
            app.commands
                .execute(&mut app.document, "Tolerance Absolute=0.02")
                .unwrap();
        }
        let before = app.document.object(id).unwrap().geometry().clone();
        let label = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "");
        assert!(app.crease_prompt.is_some());
        assert_eq!(app.document.object(id).unwrap().geometry(), &before);
        assert_eq!(app.document.undo_label(), label.as_deref());
        assert!(app.command_log.back().unwrap().contains("changed"));
    }
}

#[test]
fn edge_seam_getters_replay_native_preselection_postselection_and_surviving_edges() {
    let rows = fixtures::captures()
        .into_iter()
        .filter(|(_, r)| r["value"]["success"] == true)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 66);
    let mut failures = Vec::new();
    for preselected in [false, true] {
        for (family, row) in &rows {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let v = &row["value"];
                let (doc, ids) = native::setup_from_family(row, family);
                let mut app = test_app();
                app.document = doc;
                let mut maximum = native::check(&app.document, &v["before"], true);
                let mode = fixtures::mode(v);
                let command = match mode {
                    EdgeSeamMode::Weld => mode.command().into(),
                    EdgeSeamMode::Unweld { modify_normals } => format!(
                        "{} ModifyNormals={}",
                        mode.command(),
                        if modify_normals { "Yes" } else { "No" }
                    ),
                };
                if !preselected {
                    app.document.clear_selection();
                    enter(&mut app, &command);
                    assert!(app.crease_prompt.is_some(), "{:?}", app.command_log);
                }
                for target in native::targets(&app.document, &ids, v) {
                    let CreaseTarget::Edge(id, index) = target else {
                        panic!()
                    };
                    let kind = if matches!(
                        app.document.object(id).unwrap().geometry(),
                        Geometry::SubD(_)
                    ) {
                        ComponentSelectionKind::SubDEdge
                    } else {
                        ComponentSelectionKind::MeshEdge
                    };
                    select(&mut app, id, kind, index, preselected);
                }
                enter(&mut app, if preselected { &command } else { "" });
                assert!(app.crease_prompt.is_none(), "{:?}", app.command_log);
                maximum = maximum.max(native::check(&app.document, &v["after"], true));
                let expected = v["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|s| s.get("subd").is_some())
                    .flat_map(|s| {
                        let object = ids[s["source"].as_u64().unwrap() as usize];
                        s["selected_edges"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(move |i| (object, i.as_u64().unwrap() as usize))
                    })
                    .collect::<BTreeSet<_>>();
                let actual = app
                    .component_selection
                    .checked_picks(&app.document)
                    .unwrap()
                    .into_iter()
                    .map(|p| (p.object, p.index))
                    .collect::<BTreeSet<_>>();
                assert_eq!(actual, expected, "surviving edge selection");
                app.component_selection.clear();
                app.document.clear_selection();
                maximum = maximum.max(native::check(&app.document, &v["after_script"], true));
                enter(&mut app, "Undo");
                maximum = maximum.max(native::check(&app.document, &v["undo"], true));
                enter(&mut app, "Redo");
                maximum = maximum.max(native::check(&app.document, &v["redo"], true));
                println!(
                    "EDGE_SEAM_GETTER {}",
                    json!({"id":row["id"],"phase":if preselected { "preselected" } else { "postselected" },"maximum_error":maximum,"identity":true,"attributes":true,"history":true,"selection":true})
                );
            }));
            if result.is_err() {
                failures.push(format!(
                    "{}:{}",
                    if preselected {
                        "preselected"
                    } else {
                        "postselected"
                    },
                    row["id"]
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "edge seam getter differences: {failures:?}"
    );
}

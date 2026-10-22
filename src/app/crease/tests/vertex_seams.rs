//! Frontend replay against native vertex command geometry, not native gestures.
use super::native::fixture as native;
use super::*;
use serde_json::json;
use std::collections::BTreeSet;
#[path = "../../../../crates/viboceros-command/tests/support/vertex_seams.rs"]
mod fixtures;

#[test]
fn subd_vertex_seam_getters_replay_native_preselection_postselection_and_grips() {
    let rows = fixtures::captures()
        .into_iter()
        .filter(|(_, r)| {
            r["value"]["input"]["sources"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["kind"] == "subd")
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 40);
    let mut failures = Vec::new();
    for phase in ["preselected", "postselected"] {
        for (family, row) in &rows {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let v = &row["value"];
                let (doc, ids) = native::setup_from_family(row, family);
                let mut app = test_app();
                app.document = doc;
                let mut maximum = native::check(&app.document, &v["before"], true);
                let mode = fixtures::mode(v);
                let command = match mode {
                    VertexSeamMode::Weld => mode.command().into(),
                    VertexSeamMode::Unweld { modify_normals } => format!(
                        "{} ModifyNormals={}",
                        mode.command(),
                        if modify_normals { "Yes" } else { "No" }
                    ),
                };
                if phase == "postselected" {
                    app.document.clear_selection();
                    app.document
                        .select_objects_direct(
                            v["before"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .filter(|s| s["selected"] == true)
                                .map(|s| ids[s["source"].as_u64().unwrap() as usize]),
                            viboceros_document::SelectionMode::Add,
                        )
                        .unwrap();
                    enter(&mut app, &command);
                    assert!(app.crease_prompt.is_some(), "{:?}", app.command_log);
                }
                for target in fixtures::targets(&ids, v) {
                    if phase == "preselected" && v["input"]["grips"] == true {
                        continue;
                    }
                    select(
                        &mut app,
                        target.object,
                        ComponentSelectionKind::SubDVertex,
                        target.index,
                        phase == "preselected",
                    );
                }
                enter(&mut app, if phase == "preselected" { &command } else { "" });
                assert!(app.crease_prompt.is_none(), "{:?}", app.command_log);
                maximum = maximum.max(native::check(&app.document, &v["after"], true));
                let expected = v["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|s| s.get("subd").is_some())
                    .flat_map(|s| {
                        let object = ids[s["source"].as_u64().unwrap() as usize];
                        s["selected_vertices"]
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
                assert_eq!(actual, expected, "component selection");
                app.component_selection.clear();
                app.document.disable_control_points();
                app.document.clear_selection();
                maximum = maximum.max(native::check(&app.document, &v["after_script"], true));
                enter(&mut app, "Undo");
                maximum = maximum.max(native::check(&app.document, &v["undo"], true));
                enter(&mut app, "Redo");
                maximum = maximum.max(native::check(&app.document, &v["redo"], true));
                println!(
                    "VERTEX_SEAM_GETTER {}",
                    json!({"id":row["id"],"phase":phase,"maximum_error":maximum,"identity":true,"attributes":true,"history":true,"selection":true})
                );
            }));
            if result.is_err() {
                failures.push(format!("{phase}:{}", row["id"]));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "vertex seam getter differences: {failures:?}"
    );
}

#[test]
fn vertex_seam_getters_reject_edges_and_cancel_without_erasing_redo() {
    let (mut app, id) = setup();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    let before = app.document.object(id).unwrap().geometry().clone();
    for command in ["WeldVertices", "UnweldVertex ModifyNormals=No"] {
        enter(&mut app, command);
        select(&mut app, id, ComponentSelectionKind::SubDEdge, 0, false);
        assert!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .is_empty()
        );
        select(&mut app, id, ComponentSelectionKind::SubDVertex, 0, false);
        enter(&mut app, "SelNone");
        assert!(app.crease_prompt.is_some());
        app.cancel_current_prompt_or_selection();
        assert_eq!(app.document.object(id).unwrap().geometry(), &before);
        assert!(app.document.can_redo());
        assert!(!app.document.can_undo());
    }
}

#[test]
fn subd_surface_points_support_command_first_display_move_and_undo() {
    use viboceros_document::{ControlPointId, GripMode, SelectionMode};
    for command_first in [false, true] {
        let (mut app, id) = setup();
        let original = app.document.object(id).unwrap().geometry().clone();
        if command_first {
            enter(&mut app, "EditPtOn");
            assert!(app.object_prompt.is_some());
            app.apply_selection_click(crate::viewport::SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        } else {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "EditPtOn");
        }
        assert_eq!(app.document.grip_mode(id), Some(GripMode::Edit));
        let Geometry::SubD(source) = &original else {
            panic!()
        };
        assert_eq!(
            app.document.control_point_locations(id).unwrap(),
            source.surface_points()
        );
        app.document
            .select_control_points(
                [ControlPointId {
                    object: id,
                    index: 0,
                }],
                SelectionMode::Replace,
            )
            .unwrap();
        enter(&mut app, "Move 0,0,0 0.1,0.2,0.3");
        assert_eq!(app.document.undo_label(), Some("Move"));
        let changed = app.document.object(id).unwrap().geometry().clone();
        let Geometry::SubD(output) = &changed else {
            panic!()
        };
        let p = source.surface_points()[0].to_array();
        assert!(
            output.surface_points()[0]
                .distance_to(super::p(p[0] + 0.1, p[1] + 0.2, p[2] + 0.3))
                .unwrap()
                <= 1e-6
        );
        assert_eq!(&output.control_points()[1..], &source.control_points()[1..]);
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        assert_eq!(app.document.grip_mode(id), Some(GripMode::Edit));
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id).unwrap().geometry(), &changed);
    }
}

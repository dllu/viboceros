//! Object getter replay uses the same immutable native command corpus.
use super::*;
use crate::app::tests::test_app;
use serde_json::{Value, json};
#[path = "../../../crates/viboceros-command/tests/support/control_polygon.rs"]
mod native;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn control_polygon_getters_replay_native_geometry_metadata_selection_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for family in [
        "control_polygon",
        "control_polygon_options",
        "control_polygon_priority",
    ] {
        let rows: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in rows["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["history"].as_str().unwrap().contains("Unknown") {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            let context = format!("{family}/{id}");
            let (document, sources) = native::setup(family, id, v, &context);
            let original = document
                .objects()
                .map(|o| {
                    (
                        o.id(),
                        o.geometry_snapshot().clone(),
                        o.attributes().clone(),
                        o.group_ids().to_vec(),
                    )
                })
                .collect::<Vec<_>>();
            let mut app = test_app();
            app.document = document;
            let command = format!(
                "ExtractControlPolygon OutputLayer={} SubDPriority={}",
                v["input"]["layer"].as_str().unwrap(),
                v["input"]["priority"].as_str().unwrap_or("Geometry")
            );
            enter(&mut app, &command);
            if app.object_prompt.is_some() {
                assert_eq!(
                    app.object_prompt.as_ref().unwrap().phase,
                    ObjectPromptPhase::Selecting
                );
                app.select_prompt_objects(sources.clone(), SelectionMode::Replace);
                enter(&mut app, "");
            }
            assert!(
                app.object_prompt.is_none(),
                "{context}: {:?}",
                app.command_log
            );
            let error = native::compare(&app.document, &sources, &v["after"], &context);
            for (id, geometry, attrs, groups) in original {
                let o = app.document.object(id).unwrap();
                assert_eq!(o.geometry_snapshot(), &geometry);
                assert_eq!(o.attributes(), &attrs);
                assert_eq!(o.group_ids(), groups);
            }
            if v["success"] == true {
                assert_eq!(app.document.undo_label(), Some("ExtractControlPolygon"));
                enter(&mut app, "Undo");
                native::compare(&app.document, &sources, &v["undo"], &context);
                enter(&mut app, "Redo");
                native::compare(&app.document, &sources, &v["redo"], &context);
            } else {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "CONTROL_POLYGON_UI {}",
                json!({"family":family,"id":id,"success":v["success"],"maximum_error":error,"metadata_exact":true,"selection_exact":true,"history_exact":true,"qualified":true})
            );
            count += 1;
        }
    }
    assert_eq!(count, 42);
}

#[test]
fn control_polygon_choices_cancel_and_reset_without_model_edits() {
    let mut app = test_app();
    let captures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/control_polygon.json"
    ))
    .unwrap();
    let value = &captures["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "subd_cube")
        .unwrap()["value"];
    let (document, sources) = native::setup("control_polygon", "subd_cube", value, "option getter");
    app.document = document;
    let source = sources[0];
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtractControlPolygon");
    enter(&mut app, "SubDPriority");
    enter(&mut app, "_TextureCoordinates");
    enter(&mut app, "OutputLayer=Input");
    app.select_prompt_objects(vec![source], SelectionMode::Replace);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(mesh.vertices().len(), 24);
    assert_eq!(mesh.texture_coordinates().unwrap().len(), 24);
    enter(&mut app, "Undo");
    app.document.clear_selection();
    enter(&mut app, "ExtractControlPolygon");
    enter(&mut app, "SubDPriority=TextureCoordinates");
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.document.redo_label().is_some());
    enter(&mut app, "ExtractControlPolygon");
    let prompt = &app.object_prompt.as_ref().unwrap().description;
    assert_eq!(prompt.choices[0].value, "Current");
    assert_eq!(prompt.choices[1].value, "Geometry");
    app.select_prompt_objects(vec![source], SelectionMode::Replace);
    enter(&mut app, "");
    let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(mesh.vertices().len(), 8);
}

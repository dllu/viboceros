use super::super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_oracle::convert_extrusion::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Add,
    });
}

#[test]
fn convert_extrusion_matches_native_picking_options_geometry_and_history() {
    for (family, count) in [("convert_extrusion_pilot", 5), ("convert_extrusion", 60)] {
        replay(family, count);
    }
}
fn replay(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = super::oracle_observation::read(&base, family);
    let ops = request["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), count);
    assert_eq!(ops.len(), rows.len());
    for (op, row) in ops.iter().zip(rows) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let (doc, scene) = setup(&f, app.document.tolerance()).unwrap();
        app.document = doc;
        let original = scene
            .sources(false)
            .iter()
            .map(|id| {
                app.document
                    .object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .clone()
            })
            .collect::<Vec<_>>();
        let compare = |app: &VibocerosApp, phase: &str| {
            super::edge_analysis_mixed_support::compare(
                &snapshot(&app.document, &scene).unwrap(),
                &row["value"][phase],
                &format!("{}/{phase}", op["id"]),
            )
        };
        compare(&app, "before");
        enter(&mut app, &f.input());
        if !f.preselect {
            assert_eq!(
                app.object_prompt.as_ref().unwrap().phase,
                ObjectPromptPhase::Selecting
            );
            pick(&mut app, scene.peer);
            assert_eq!(app.document.selected_object_count(), 0);
            for id in scene.sources(f.reverse) {
                pick(&mut app, id);
            }
            enter(&mut app, "");
        }
        assert_eq!(
            app.object_prompt.as_ref().unwrap().phase,
            ObjectPromptPhase::Options
        );
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        compare(&app, "after_script");
        assert_eq!(app.document.undo_label(), Some("ConvertExtrusion"));
        enter(&mut app, "Undo");
        compare(&app, "undo");
        for (id, geometry) in scene.sources(false).iter().zip(&original) {
            assert!(
                geometry.shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
            );
        }
        enter(&mut app, "Redo");
        compare(&app, "redo");
    }
}

#[test]
fn convert_extrusion_cancel_and_invalid_options_preserve_model_and_redo() {
    for preselect in [false, true] {
        let mut app = test_app();
        let f = Fixture {
            asset: "tosubd_extrusion_rectangle_00".into(),
            delete: None,
            preselect,
            grouped: false,
            pair: false,
            reverse: false,
        };
        let (doc, scene) = setup(&f, app.document.tolerance()).unwrap();
        app.document = doc;
        enter(&mut app, "Point 70,80,90");
        enter(&mut app, "Undo");
        let original = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "ConvertExtrusion");
        if !preselect {
            pick(&mut app, scene.source);
            enter(&mut app, "");
        }
        let pending = app.object_prompt.clone();
        enter(&mut app, "DeleteInput=Maybe");
        assert_eq!(app.object_prompt, pending);
        enter(&mut app, "DeleteInput=No");
        app.cancel_interactive_command(false);
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            original
        );
        assert_eq!(app.document.redo_label(), Some("Point"));
        assert_eq!(app.document.selected_object_count(), usize::from(preselect));
        let prompt = app
            .commands
            .object_selection_prompt("ConvertExtrusion")
            .unwrap()
            .unwrap();
        assert!(prompt.options[0].value);
    }
}

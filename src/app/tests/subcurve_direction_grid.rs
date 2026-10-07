use super::subcurve_direction::{compare, enter, geometry, position};
use super::*;
use serde_json::Value;

#[test]
fn closed_direction_matrix_replays_native_inputs_geometry_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_direction_grid.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        let mut app = test_app();
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| app.document.add_geometry(geometry(row)).unwrap())
            .collect::<Vec<_>>();
        let source = ids[0];
        let original = app.document.object(source).unwrap().geometry().clone();
        app.document.clear_history().unwrap();
        let inline = case.starts_with("inline_");
        if inline {
            enter(&mut app, "CreateUVCrv");
            enter(&mut app, &ids[1].to_string());
            enter(&mut app, "SubCrv");
            enter(&mut app, &source.to_string());
        } else {
            app.document.select_command_results([source]).unwrap();
            enter(&mut app, "SubCrv Copy=Yes FromMidpoint=No Mode=Shorten");
        }
        assert!(app.accept_drafting_point(position(&v["start"])));
        app.handle_viewport_action(ViewportOutput {
            drafting_hover: Some(position(&v["aim"])),
            ..Default::default()
        });
        for token in v["typed_inputs"].as_array().unwrap() {
            enter(&mut app, token.as_str().unwrap());
            if token == &v["length_token"] && !v["finish_states"].as_array().unwrap().is_empty() {
                let (object, start, length, locked) = if inline {
                    let p = app
                        .intersection_prompt
                        .as_ref()
                        .unwrap()
                        .uv_subcurves
                        .pending
                        .as_ref()
                        .unwrap();
                    (p.object, p.start, p.length, p.locked_forward)
                } else {
                    let p = app.subcurve_prompt.as_ref().unwrap();
                    (p.source, p.start, p.length, p.locked_forward)
                };
                assert_eq!(object, Some(source), "{case}");
                assert!(start.is_some(), "{case}");
                assert_eq!(
                    length,
                    Some(v["length_token"].as_str().unwrap().parse().unwrap()),
                    "{case}"
                );
                assert!(locked.is_none(), "{case}");
                assert!(app.active_command.is_some(), "{case}");
                assert!(!app.document.can_undo(), "{case}");
                assert_eq!(app.document.objects().len(), ids.len(), "{case}");
                compare(&app, &v["finish_states"][0]["objects"], case);
            }
        }
        assert!(app.subcurve_prompt.is_none(), "{case}");
        assert!(app.active_command.is_none(), "{case}");
        compare(&app, &v["after"], case);
        assert_eq!(app.document.object(source).unwrap().geometry(), &original);
        if v["success"] == true {
            enter(&mut app, "Undo");
            compare(&app, &v["undo"], case);
            enter(&mut app, "Redo");
            compare(&app, &v["redo"], case);
        } else {
            assert!(!app.document.can_undo(), "{case}");
        }
    }
}

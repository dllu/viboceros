use super::curve_fillet::{enter, inputs, mouse_picks, replay_filtered};
use super::plane_surface_native_support as native;
use super::*;
const FAMILIES: [&str; 1] = ["fillet_zero_final"];

fn invalid(value: &serde_json::Value) -> bool {
    value["after"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["native_valid"] == false)
}

#[test]
fn valid_zero_radius_workflows_match_native_geometry_and_history() {
    assert_eq!(
        replay_filtered(&FAMILIES, "FILLET_ZERO_UI", |v| !invalid(v)),
        38
    );
}

#[test]
fn invalid_native_empty_joins_are_rejected_without_mutation_or_lost_redo() {
    let mut count = 0;
    for family in FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            if !invalid(value) {
                continue;
            }
            let mut app = test_app();
            app.document = inputs::setup(value);
            if !app.document.can_redo() {
                app.commands
                    .execute(&mut app.document, "Point 90,91,92")
                    .unwrap();
                app.document.undo().unwrap();
            }
            let picks = inputs::picks(&app.document, value);
            let originals = app.document.objects().cloned().collect::<Vec<_>>();
            let options = inputs::final_options(value, Default::default());
            enter(&mut app, &format!("Fillet {}", options.arguments()));
            for (id, point) in picks {
                app.pick_curve_fillet_at(id, point);
            }
            assert!(app.curve_fillet_session.is_none());
            assert_eq!(
                app.command_log.back().unwrap(),
                "Unable to fillet these curves"
            );
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                originals
            );
            assert_eq!(app.document.selected_object_count(), 0);
            assert!(!app.document.can_undo());
            assert!(app.document.can_redo());
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("guard.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = viboceros_document::Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            let error = inputs::check(&restored, &value["before"], true);
            println!(
                "FILLET_ZERO_INVALID_UI {}",
                serde_json::json!({"id":format!("{family}/{}",row["id"].as_str().unwrap()),"redo_preserved":true,"archives":1,"maximum_geometry_error":error,"native_invalid":true})
            );
            count += 1;
        }
    }
    assert_eq!(count, 2);
}

#[test]
fn zero_radius_mouse_preview_commits_its_retained_piece_once() {
    let data = native::read("fillet_zero_final");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "verified_zero_dynamic_accept")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = inputs::setup(value);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, value);
    let parent = app
        .document
        .object(picks[0].0)
        .unwrap()
        .geometry_snapshot()
        .clone();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Fillet Radius=0 Trim=Yes Join=Yes Dynamic=Yes");
    mouse_picks(
        &mut app,
        picks.map(|(id, _)| id),
        [
            native::point(&value["prescribed_case"]["actions"][6]["pick"]),
            native::point(&value["prescribed_case"]["actions"][7]["pick"]),
        ],
    );
    let scene = app.curve_fillet_session.as_ref().unwrap().scene().unwrap();
    let retained = scene.objects().last().unwrap().geometry_snapshot().clone();
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(app.document.objects().any(|o| o.geometry() == &*retained));
    assert!(
        parent.shares_storage_with(app.document.object(picks[0].0).unwrap().geometry_snapshot())
    );
    state.verify(&mut app.document, value);
    println!("FILLET_ZERO_MOUSE_SOURCE_PICKS 2");
}

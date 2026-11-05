use super::oracle_observation as observation;
use super::*;
use serde_json::Value;
#[path = "../../../crates/viboceros-oracle/tests/support/subd_area.rs"]
mod support;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn subd_area_getters_accept_flat_smooth_mixed_and_postselected_sources_and_preserve_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in ["subd_area_discovery", "subd_area_followups"] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let observed = observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            if !matches!(
                op["id"].as_str().unwrap(),
                "grid_0" | "grid_1" | "bent_grid_postselected" | "cube_mixed"
            ) {
                continue;
            }
            for native in row["value"]["commands"].as_array().unwrap() {
                eprintln!("{} {}", op["id"], native["command"]);
                count += 1;
                let mut app = test_app();
                app.document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
                let geometry = viboceros_oracle::subd_area::source(
                    op["scene"].as_str().unwrap(),
                    app.document.tolerance(),
                )
                .unwrap();
                let owner = app.document.add_geometry(geometry).unwrap();
                if op["flat"] == true {
                    enter(&mut app, "SubDDisplayToggle");
                }
                if op["curve"] == true {
                    enter(&mut app, "Circle 20,0,0 2");
                }
                let originals = app.document.objects().cloned().collect::<Vec<_>>();
                app.document.clear_selection();
                app.document.clear_history().unwrap();
                enter(&mut app, "Point 40,50,60");
                enter(&mut app, "Undo");
                let preselect = op["preselect"].as_bool().unwrap_or(true);
                if preselect {
                    app.document
                        .select_objects_direct(originals.iter().map(|o| o.id()), SelectionMode::Add)
                        .unwrap();
                }
                support::state(
                    &viboceros_oracle::subd_area::snapshot(&app.document, owner),
                    &native["before"],
                    None,
                );
                let command = native["command"].as_str().unwrap();
                let mixed = (op["curve"] == true && command == "AreaCentroid")
                    .then_some(&row["value"]["diagnostics"]);
                enter(&mut app, command);
                if !preselect {
                    assert!(app.object_prompt.is_some());
                    app.select_prompt_objects([owner], SelectionMode::Add);
                    assert!(app.document.is_selected(owner));
                    enter(&mut app, "");
                }
                assert!(app.object_prompt.is_none());
                support::state(
                    &viboceros_oracle::subd_area::snapshot(&app.document, owner),
                    &native["after"],
                    mixed,
                );
                for source in &originals {
                    assert_eq!(app.document.object(source.id()), Some(source));
                }
                if command == "Area" {
                    assert!(app.document.can_redo());
                    assert!(!app.document.can_undo());
                    let value = app
                        .command_log
                        .back()
                        .unwrap()
                        .split_once("total area ")
                        .unwrap()
                        .1
                        .parse::<f64>()
                        .unwrap();
                    assert!((value - support::area(native)).abs() < 1e-6);
                    enter(&mut app, "Redo");
                    support::state(
                        &viboceros_oracle::subd_area::snapshot(&app.document, owner),
                        &native["redo"],
                        mixed,
                    );
                    enter(&mut app, "Undo");
                    support::state(
                        &viboceros_oracle::subd_area::snapshot(&app.document, owner),
                        &native["undo"],
                        None,
                    );
                } else {
                    assert_eq!(app.document.undo_label(), Some("AreaCentroid"));
                    assert!(!app.document.can_redo());
                    enter(&mut app, "Undo");
                    support::state(
                        &viboceros_oracle::subd_area::snapshot(&app.document, owner),
                        &native["undo"],
                        None,
                    );
                    enter(&mut app, "Redo");
                    support::state(
                        &viboceros_oracle::subd_area::snapshot(&app.document, owner),
                        &native["redo"],
                        mixed,
                    );
                }
            }
        }
    }
    assert_eq!(count, 8);
}

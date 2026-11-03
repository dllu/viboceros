use super::*;
use crate::app::tests::subd_native as common;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind;
mod loose;
#[path = "../../../crates/viboceros-oracle/tests/support/inset_surface.rs"]
mod native_surface;
#[path = "../../../crates/viboceros-oracle/tests/support/inset.rs"]
mod support;
mod surface;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn inset_native_face_getters_options_cap_selection_and_history_match_application() {
    for family in ["inset_faces", "inset_extra_faces", "inset_mesh_ready"] {
        for (op, native) in common::cases_for(family) {
            let Some(indices) = support::indices(&op) else {
                continue;
            };
            if native["after"][0]["subd"]["valid"] == false {
                continue;
            }
            let label = op["id"].as_str().unwrap();
            for preselect in [false, true] {
                let mut app = test_app();
                let (doc, id) = support::fixture(&native);
                app.document = doc;
                let mesh = matches!(
                    app.document.object(id).unwrap().geometry(),
                    Geometry::Mesh(_)
                );
                let kind = if mesh {
                    ComponentSelectionKind::MeshFace
                } else {
                    ComponentSelectionKind::SubDFace
                };
                let picks = indices
                    .iter()
                    .map(|&index| ComponentPick {
                        object: id,
                        index,
                        kind,
                    })
                    .collect::<Vec<_>>();
                if preselect {
                    for p in &picks {
                        app.accept_component_click(ComponentClick {
                            picks: vec![*p],
                            preselection: true,
                            modifiers: Default::default(),
                        });
                    }
                }
                enter(&mut app, "Inset");
                assert!(
                    app.inset_session.is_some(),
                    "{label}: {:?}",
                    app.command_log
                );
                if !preselect {
                    assert!(app.picking_inset());
                    app.accept_component_window(ComponentWindow {
                        picks: picks.clone(),
                        preselection: false,
                        modifiers: Default::default(),
                        crossing: false,
                        inverted: false,
                    });
                    app.accept_component_click(ComponentClick {
                        picks: vec![picks[0]],
                        preselection: false,
                        modifiers: egui::Modifiers {
                            ctrl: true,
                            ..Default::default()
                        },
                    });
                    app.accept_component_click(ComponentClick {
                        picks: vec![picks[0]],
                        preselection: false,
                        modifiers: Default::default(),
                    });
                    enter(&mut app, "");
                }
                assert!(!app.picking_inset(), "{label}: {:?}", app.command_log);
                let distance = support::effective_distance(&op, &native, family);
                let options = support::options(&op, distance);
                enter(&mut app, options.command().strip_prefix("Inset ").unwrap());
                if op["distance"].as_f64().unwrap() <= 0. {
                    enter(&mut app, &op["distance"].to_string());
                    assert!(app.command_log.iter().any(|line| line.contains("Error:")));
                }
                assert!(!app.inset_session.as_ref().unwrap().edges().is_empty());
                assert!(!app.document.can_undo());
                enter(&mut app, "");
                assert!(
                    app.inset_session.is_none(),
                    "{label}: {:?}",
                    app.command_log
                );
                support::compare(
                    &support::snapshot(&app.document, id),
                    &support::expected_rows(&native["after"], true),
                    label,
                );
                let selected = app
                    .component_selection
                    .checked_picks(&app.document)
                    .unwrap();
                assert_eq!(selected.len(), indices.len());
                assert!(
                    indices.iter().all(|i| selected.contains(&ComponentPick {
                        object: id,
                        index: *i,
                        kind
                    })),
                    "{label}"
                );
                // The native macro's trailing Cancel releases retained cap picks.
                enter(&mut app, "SelNone");
                assert!(
                    app.component_selection
                        .checked_picks(&app.document)
                        .unwrap()
                        .is_empty(),
                    "{label}: cleanup"
                );
                support::compare(
                    &support::snapshot(&app.document, id),
                    &support::expected_rows(&native["after_script"], true),
                    label,
                );
                enter(&mut app, "Undo");
                support::compare(
                    &support::snapshot(&app.document, id),
                    &support::expected_rows(&native["undo"], false),
                    label,
                );
                enter(&mut app, "Redo");
                support::compare(
                    &support::snapshot(&app.document, id),
                    &support::expected_rows(&native["redo"], true),
                    label,
                );
            }
        }
    }
}
#[test]
fn inset_cancel_invalid_inputs_stale_previews_and_complete_typed_selectors_are_atomic() {
    let native = common::cases_for("inset_faces").remove(0).1;
    let mut app = test_app();
    let (doc, id) = support::fixture(&native);
    app.document = doc;
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Inset .3 Faces=0");
    assert!(app.inset_session.is_none());
    assert_eq!(app.document.undo_label(), Some("Inset"));
    enter(&mut app, "SelNone");
    enter(&mut app, "Undo");
    let before = support::snapshot(&app.document, id);
    let pick = ComponentPick {
        object: id,
        index: 0,
        kind: ComponentSelectionKind::SubDFace,
    };
    app.accept_component_click(ComponentClick {
        picks: vec![pick],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "Inset");
    for invalid in [
        "0",
        "-1",
        "NaN",
        "Mode=Other",
        "Mode=Group Mode=Single",
        "2",
    ] {
        enter(&mut app, invalid);
        assert!(app.inset_session.is_some());
    }
    assert_eq!(support::snapshot(&app.document, id), before);
    assert_eq!(app.document.redo_label(), Some("Inset"));
    enter(&mut app, "Cancel");
    assert!(app.inset_session.is_none());
    app.accept_component_click(ComponentClick {
        picks: vec![pick],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "Inset");
    app.document
        .add_group(Some("Changed".into()), [id])
        .unwrap();
    let snapshot = support::snapshot(&app.document, id);
    enter(&mut app, "");
    assert!(app.inset_session.is_some());
    assert_eq!(support::snapshot(&app.document, id), snapshot);
    assert!(app.command_log.iter().any(|s| s.contains("changed")));
    enter(&mut app, "Cancel");
}
mod viewport;

#[test]
fn inset_native_subd_preferences_reset_mode_each_command_and_keep_completed_distances() {
    let native = common::cases_for("inset_preferences").remove(0).1;
    let mut app = test_app();
    for (row_index, row) in native.as_array().unwrap().iter().enumerate() {
        let value = &row["value"];
        let (doc, id) = support::fixture(value);
        app.document = doc;
        app.component_selection.clear();
        for index in [0, 1] {
            app.accept_component_click(ComponentClick {
                picks: vec![ComponentPick {
                    object: id,
                    index,
                    kind: ComponentSelectionKind::SubDFace,
                }],
                preselection: true,
                modifiers: Default::default(),
            });
        }
        enter(&mut app, "Inset");
        assert!(app.inset_session.is_some());
        if let Some(mode) = row["input"]["mode"].as_str() {
            enter(&mut app, &format!("Mode={mode}"));
        }
        if row["input"]["cancel"] == true {
            enter(&mut app, "Cancel");
        } else {
            let distance = row["input"]["distance"].as_f64().unwrap();
            enter(&mut app, &distance.to_string());
            enter(&mut app, "");
            enter(&mut app, "SelNone");
        }
        support::compare(
            &support::snapshot(&app.document, id),
            &support::expected_rows(&value["after_script"], true),
            &format!("preferences {row_index}"),
        );
        assert_eq!(
            app.commands.inset_options(false).mode,
            viboceros_command::inset::Mode::Group
        );
    }
}
#[test]
fn inset_mesh_cancel_keeps_the_last_completed_distance() {
    let native = common::cases_for("inset_mesh_ready").remove(0).1;
    let mut app = test_app();
    let (doc, id) = support::fixture(&native);
    app.document = doc;
    let pick = ComponentPick {
        object: id,
        index: 0,
        kind: ComponentSelectionKind::MeshFace,
    };
    app.accept_component_click(ComponentClick {
        picks: vec![pick],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "Inset");
    enter(&mut app, ".3");
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.inset_options(true).distance, 1.);
    app.accept_component_click(ComponentClick {
        picks: vec![pick],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "Inset .3");
    assert_eq!(app.commands.inset_options(true).distance, 0.3);
    enter(&mut app, "SelNone");
    enter(&mut app, "Undo");
    assert_eq!(app.commands.inset_options(true).distance, 0.3);
}

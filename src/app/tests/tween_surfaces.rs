use super::boolean_intersection::enter;
use super::*;
fn pair() -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0");
    enter(&mut app, "SrfPt 0,0,4 4,0,4 4,6,4 0,6,4");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    (app, [ids[0], ids[1]])
}
fn click(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}
#[test]
fn ordered_picks_preserve_source_order_and_preview_without_document_edits() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "TweenSurfaces NumberOfSurfaces=2 SampleNumber=4");
    click(&mut app, ids[1]);
    assert_eq!(
        app.tween_surfaces_prompt.as_ref().unwrap().sources,
        vec![ids[1]]
    );
    click(&mut app, ids[0]);
    let p = app.tween_surfaces_prompt.as_ref().unwrap();
    assert_eq!(p.sources, vec![ids[1], ids[0]]);
    let scene = p.scene().unwrap();
    assert_eq!(scene.objects().len(), 4);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    let Geometry::Brep(b) = scene.objects().nth(2).unwrap().geometry() else {
        panic!()
    };
    let s = b.faces()[0].surface();
    assert!(
        (s.evaluate(*s.domain_u().start(), *s.domain_v().start())
            .unwrap()
            .z()
            - 8. / 3.)
            .abs()
            < 1e-12
    );
    let preview_ids = scene.objects().map(|o| o.id()).collect::<Vec<_>>();
    enter(&mut app, "NumberOfSurfaces=2");
    assert_eq!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        preview_ids
    );
    enter(&mut app, "");
    assert!(app.tween_surfaces_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[test]
fn options_refresh_cached_preview_and_invalid_edits_preserve_it() {
    let (mut app, ids) = pair();
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    enter(&mut app, "NumberOfSurfaces");
    enter(&mut app, "0");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "3");
    assert_eq!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        5
    );
    enter(&mut app, "MatchMethod Refit");
    let p = app.tween_surfaces_prompt.as_ref().unwrap();
    let before = p
        .scene()
        .unwrap()
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>();
    enter(&mut app, "SampleNumber=4");
    assert_eq!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        before
    );
    enter(&mut app, "FlipEndU=Yes");
    assert_ne!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        before
    );
    enter(&mut app, "NumberOfSurfaces");
    enter(&mut app, "");
    assert!(app.tween_surfaces_prompt.is_some());
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Cancel");
    assert!(app.tween_surfaces_prompt.is_none());
    assert!(!app.document.can_undo());
}
#[test]
fn preselection_admission_and_cancellation_preserve_native_source_selection() {
    for count in 0..=2 {
        let (mut app, ids) = pair();
        app.document
            .select_objects_direct(ids[..count].iter().copied(), SelectionMode::Replace)
            .unwrap();
        let selected = app.document.selected_object_ids().collect::<Vec<_>>();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "TweenSurfaces SampleNumber=4");
        assert_eq!(
            app.tween_surfaces_prompt.as_ref().unwrap().sources.len(),
            count
        );
        app.apply_selection_region(
            SelectionWindow {
                object_ids: ids.to_vec(),
                mode: SelectionMode::Replace,
                crossing: true,
                inverted: false,
            },
            true,
        );
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        for &id in &ids[count..] {
            click(&mut app, id);
        }
        enter(&mut app, "");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        assert_eq!(app.document.objects().len(), 3);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        enter(&mut app, "Redo");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        enter(&mut app, "TweenSurfaces");
        enter(&mut app, "Cancel");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
    }
}
#[test]
fn stale_sources_and_other_commands_discard_preview_before_acceptance() {
    let (mut app, ids) = pair();
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    app.document.delete_object(ids[1]).unwrap();
    app.validate_tween_surfaces();
    assert!(app.tween_surfaces_prompt.is_none());
    let (mut app, ids) = pair();
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    enter(&mut app, "Point 9,9,9");
    assert!(app.tween_surfaces_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(
        app.document
            .objects()
            .filter(|o| matches!(o.geometry(), Geometry::Brep(_) | Geometry::NurbsSurface(_)))
            .count(),
        2
    );
}
#[test]
fn repeated_source_picks_create_copies_without_mutating_the_source() {
    let (mut app, ids) = pair();
    let before = app.document.object(ids[0]).unwrap().clone();
    enter(&mut app, "TweenSurfaces MatchMethod=None");
    click(&mut app, ids[0]);
    click(&mut app, ids[0]);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.object(ids[0]).unwrap(), &before);
}
#[test]
fn corner_controls_transform_current_end_axes_and_leave_live_sources_unchanged() {
    use crate::viewport::SurfaceCornerAction as Action;
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "TweenSurfaces MatchMethod=None");
    for id in ids {
        click(&mut app, id);
    }
    let initial = app.tween_corner_controls().to_vec();
    assert_eq!(initial.len(), 3);
    assert!(app.handle_viewport_action(ViewportOutput {
        surface_corner_click: Some(Action::ReverseU),
        ..Default::default()
    }));
    let changed = app.tween_corner_controls().to_vec();
    assert_eq!(changed[0].point, initial[1].point);
    assert_eq!(changed[1].point, initial[0].point);
    app.edit_tween_corner(Action::SwapUv);
    let swapped = app.tween_corner_controls();
    assert_eq!(swapped[0].point, initial[2].point);
    assert_eq!(swapped[1].point, initial[0].point);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    app.edit_tween_corner(Action::SwapUv);
    app.edit_tween_corner(Action::ReverseU);
    let restored = app.tween_corner_controls();
    assert_eq!(restored[0].point, initial[0].point);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[test]
fn source_selection_and_value_questions_hide_corner_controls() {
    let (mut app, ids) = pair();
    enter(&mut app, "TweenSurfaces");
    assert!(app.tween_corner_controls().is_empty());
    click(&mut app, ids[0]);
    assert!(app.tween_corner_controls().is_empty());
    click(&mut app, ids[1]);
    assert_eq!(app.tween_corner_controls().len(), 3);
    enter(&mut app, "NumberOfSurfaces");
    assert!(app.tween_corner_controls().is_empty());
    enter(&mut app, "");
    assert_eq!(app.tween_corner_controls().len(), 3);
    enter(&mut app, "Cancel");
    assert!(app.tween_corner_controls().is_empty());
}
#[test]
fn failed_preview_keeps_source_controls_available_for_direction_recovery() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0");
    enter(
        &mut app,
        "SrfControlPtGrid Degree=2 3 Degree=2 3 10,0,4 10,3,4 10,6,4 12,0,4 12,3,5 12,6,6 14,0,4 14,3,6 14,6,8",
    );
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    enter(
        &mut app,
        "TweenSurfaces MatchMethod=SamplePoints SampleNumber=255 NumberOfSurfaces=16",
    );
    for id in ids {
        click(&mut app, id);
    }
    assert!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_none()
    );
    assert_eq!(app.tween_corner_controls().len(), 3);
    app.edit_tween_corner(crate::viewport::SurfaceCornerAction::ReverseU);
    assert_eq!(app.tween_corner_controls().len(), 3);
    enter(&mut app, "MatchMethod=Refit");
    assert!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_some()
    );
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn queued_corner_edits_reject_stale_source_geometry_attributes_and_settings() {
    use crate::viewport::SurfaceCornerAction as Action;
    for changed in 0..3 {
        let (mut app, ids) = pair();
        enter(&mut app, "TweenSurfaces");
        for id in ids {
            click(&mut app, id);
        }
        match changed {
            0 => {
                app.document.delete_object(ids[1]).unwrap();
            }
            1 => {
                app.document
                    .set_object_names([(ids[1], Some("changed".into()))])
                    .unwrap();
            }
            _ => {
                let layer = app
                    .document
                    .add_layer("Other", viboceros_document::ColorRgb::BLACK)
                    .unwrap();
                app.document.set_current_layer(layer).unwrap();
            }
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        assert!(!app.edit_tween_corner(Action::SwapUv));
        assert!(app.tween_surfaces_prompt.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}
#[test]
fn failed_option_preparation_drops_previous_preview_as_one_unit() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0");
    enter(
        &mut app,
        "SrfControlPtGrid Degree=2 3 Degree=2 3 10,0,4 10,3,4 10,6,4 12,0,4 12,3,5 12,6,6 14,0,4 14,3,6 14,6,8",
    );
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let ids = before.iter().map(|o| o.id()).collect::<Vec<_>>();
    enter(&mut app, "TweenSurfaces MatchMethod=Refit");
    for id in ids {
        click(&mut app, id);
    }
    assert!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_some()
    );
    enter(
        &mut app,
        "MatchMethod=SamplePoints SampleNumber=255 NumberOfSurfaces=16",
    );
    assert!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_none()
    );
    enter(&mut app, "");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.tween_surfaces_prompt.is_some());
    enter(&mut app, "MatchMethod=Refit");
    assert!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_some()
    );
}
#[test]
fn corner_actions_replay_nine_native_calibrated_click_outcomes_and_history() {
    use crate::viewport::SurfaceCornerAction as Action;
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tween_surfaces_corners.json"
    ))
    .unwrap();
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let mut app = test_app();
        enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,1 0,6,0");
        enter(&mut app, "SrfPt 10,0,4 14,0,4 14,6,7 10,6,4");
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let ids = before.iter().map(|o| o.id()).collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        enter(&mut app, "TweenSurfaces MatchMethod=None");
        for id in ids {
            click(&mut app, id);
        }
        for click in v["spec"]["clicks"].as_array().unwrap() {
            if click["source"] == 1 {
                let action = match click["corner"].as_u64().unwrap() {
                    0 => Some(Action::SwapUv),
                    1 => Some(Action::ReverseU),
                    2 => Some(Action::ReverseV),
                    _ => None,
                };
                if let Some(action) = action {
                    app.handle_viewport_action(ViewportOutput {
                        surface_corner_click: Some(action),
                        ..Default::default()
                    });
                }
            }
        }
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "");
        let output = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(app.document.objects().len(), output.len());
        for (object, n) in app.document.objects().zip(output) {
            let s = match object.geometry() {
                Geometry::NurbsSurface(s) => s,
                Geometry::Brep(b) => b.faces()[0].surface(),
                _ => panic!(),
            };
            for (i, p) in n["samples"].as_array().unwrap().iter().enumerate() {
                let u = *s.domain_u().start()
                    + (*s.domain_u().end() - *s.domain_u().start()) * (i % 9) as f64 / 8.;
                let w = *s.domain_v().start()
                    + (*s.domain_v().end() - *s.domain_v().start()) * (i / 9) as f64 / 8.;
                let expected =
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                        .unwrap();
                assert!(
                    s.evaluate(u, w).unwrap().distance_to(expected).unwrap() < 1e-7,
                    "{}",
                    v["case"]
                );
            }
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), output.len());
    }
}
#[test]
fn repeated_corner_sequences_replay_current_controls_native_geometry_and_history() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tween_surfaces_corner_sequences.json"
    ))
    .unwrap();
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let mut app = test_app();
        enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,1 0,6,0");
        enter(&mut app, "SrfPt 10,0,4 14,0,4 14,6,7 10,6,4");
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let ids = before.iter().map(|o| o.id()).collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        enter(&mut app, "TweenSurfaces MatchMethod=None");
        for id in ids {
            click(&mut app, id);
        }
        for click in v["spec"]["clicks"].as_array().unwrap() {
            let index = click["corner"].as_u64().unwrap() as usize;
            let point = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(
                    v["spec"]["sources"][1]["control_points"][index]["point"].clone(),
                )
                .unwrap(),
            )
            .unwrap();
            let action = app
                .tween_corner_controls()
                .iter()
                .find(|c| c.point.distance_to(point).unwrap() < 1e-10)
                .map(|c| c.action);
            if let Some(action) = action {
                assert!(app.edit_tween_corner(action));
            }
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        }
        enter(&mut app, "");
        let native = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(app.document.objects().len(), native.len(), "{}", v["case"]);
        for (object, n) in app.document.objects().zip(native) {
            let s = match object.geometry() {
                Geometry::NurbsSurface(s) => s,
                Geometry::Brep(b) => b.faces()[0].surface(),
                _ => panic!(),
            };
            for (i, p) in n["samples"].as_array().unwrap().iter().enumerate() {
                let u = *s.domain_u().start()
                    + (*s.domain_u().end() - *s.domain_u().start()) * (i % 9) as f64 / 8.;
                let w = *s.domain_v().start()
                    + (*s.domain_v().end() - *s.domain_v().start()) * (i / 9) as f64 / 8.;
                let expected =
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                        .unwrap();
                assert!(
                    s.evaluate(u, w).unwrap().distance_to(expected).unwrap() < 1e-7,
                    "{} {i}",
                    v["case"]
                );
            }
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), native.len());
    }
}
#[test]
fn cancel_saves_only_layer_while_acceptance_saves_count_method_and_inactive_samples() {
    use viboceros_command::tween_surfaces::{Method, OutputLayer, Preferences};
    let (mut app, ids) = pair();
    let initial = app.commands.tween_surface_preferences();
    enter(
        &mut app,
        "TweenSurfaces NumberOfSurfaces=3 OutputLayer=StartSrf",
    );
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.tween_surface_preferences(), initial);
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    enter(&mut app, "NumberOfSurfaces=3");
    enter(&mut app, "SampleNumber=6");
    enter(&mut app, "OutputLayer=StartSrf");
    enter(&mut app, "MatchMethod=Refit");
    enter(&mut app, "FlipEndU=Yes");
    enter(&mut app, "Cancel");
    assert_eq!(
        app.commands.tween_surface_preferences(),
        Preferences {
            layer: OutputLayer::Start,
            ..initial
        }
    );
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    enter(&mut app, "NumberOfSurfaces=2");
    enter(&mut app, "SampleNumber=6");
    enter(&mut app, "MatchMethod=Refit");
    enter(&mut app, "");
    let saved = app.commands.tween_surface_preferences();
    assert_eq!(
        (saved.number, saved.method, saved.sample_number),
        (2, Method::Refit, 6)
    );
    enter(&mut app, "Undo");
    assert_eq!(app.commands.tween_surface_preferences(), saved);
    enter(&mut app, "Redo");
    assert_eq!(app.commands.tween_surface_preferences(), saved);
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    enter(&mut app, "MatchMethod=SamplePoints");
    assert!(
        app.tween_surfaces_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_some()
    );
    enter(&mut app, "NumberOfSurfaces=0");
    assert_eq!(app.commands.tween_surface_preferences(), saved);
    enter(&mut app, "Cancel");
    assert_eq!(
        app.commands.tween_surface_options(&[]).unwrap().reverse,
        [[false; 3]; 2]
    );
}
#[test]
fn native_option_sequence_replays_each_next_invocation_default() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tween_surfaces_options.json"
    ))
    .unwrap();
    let states: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/tween-options-provenance.json")).unwrap();
    replay_option_defaults(&q, &states["states"]);
}
#[test]
fn native_inactive_sample_count_is_saved_on_acceptance() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tween_surfaces_sample_memory.json"
    ))
    .unwrap();
    let states: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/tween-options-provenance.json")).unwrap();
    replay_option_defaults(&q, &states["inactive_sample_followup"]["states"]);
}
fn replay_option_defaults(q: &serde_json::Value, states: &serde_json::Value) {
    use viboceros_command::tween_surfaces::{Method, OutputLayer};
    let (mut app, ids) = pair();
    for r in q["results"][0]["value"]["records"].as_array().unwrap() {
        let step = r["step"].as_str().unwrap();
        let expected = &states[step]["before"];
        let saved = app.commands.tween_surface_preferences();
        assert_eq!(
            saved.number,
            expected["NumberOfSurfaces"]
                .as_str()
                .unwrap()
                .parse::<usize>()
                .unwrap(),
            "{step}"
        );
        assert_eq!(
            saved.layer,
            match expected["OutputLayer"].as_str().unwrap() {
                "StartSrf" => OutputLayer::Start,
                "EndSrf" => OutputLayer::End,
                _ => OutputLayer::Current,
            },
            "{step}"
        );
        assert_eq!(
            saved.method,
            match expected["MatchMethod"].as_str().unwrap() {
                "None" => Method::Control,
                "Refit" => Method::Refit,
                _ => Method::Sampled,
            },
            "{step}"
        );
        if let Some(sample) = expected["SampleNumber"].as_str() {
            assert_eq!(
                saved.sample_number,
                sample.parse::<usize>().unwrap(),
                "{step}"
            );
        }
        enter(&mut app, "TweenSurfaces");
        for id in ids {
            click(&mut app, id);
        }
        for edit in r["edits"].as_array().unwrap() {
            let (name, value) = edit.as_str().unwrap().split_once('=').unwrap();
            enter(
                &mut app,
                &format!(
                    "{}={}",
                    name.trim_start_matches('_'),
                    value.trim_start_matches('_')
                ),
            );
        }
        enter(&mut app, if r["accepted"] == true { "" } else { "Cancel" });
        if r["accepted"] == true {
            let memory = app.commands.tween_surface_preferences();
            enter(&mut app, "Undo");
            assert_eq!(app.commands.tween_surface_preferences(), memory);
            enter(&mut app, "Redo");
            assert_eq!(app.commands.tween_surface_preferences(), memory);
            let outputs = app
                .document
                .objects()
                .filter(|o| !ids.contains(&o.id()))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            app.document.delete_objects(outputs).unwrap();
        }
        app.document.clear_history().unwrap();
        assert_eq!(app.document.objects().len(), 2);
    }
}
#[test]
fn failed_acceptance_does_not_rollback_an_unrelated_open_transaction() {
    let (mut app, ids) = pair();
    enter(&mut app, "TweenSurfaces");
    for id in ids {
        click(&mut app, id);
    }
    app.document.begin_transaction("External edit").unwrap();
    let id = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(9., 9., 9.).unwrap()))
        .unwrap();
    enter(&mut app, "");
    assert!(app.document.object(id).is_some());
    app.document.commit_transaction().unwrap();
    assert_eq!(app.document.undo_label(), Some("External edit"));
    assert_eq!(app.document.objects().len(), 3);
}
#[test]
fn native_preselection_acceptance_and_cancellation_replay_geometry_and_history() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tween_surfaces_interaction.json"
    ))
    .unwrap();
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let (mut app, ids) = pair();
        let count = v["spec"]["pre"].as_u64().unwrap() as usize;
        app.document
            .select_objects_direct(ids[..count].iter().copied(), SelectionMode::Replace)
            .unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "TweenSurfaces");
        click(&mut app, ids[0]);
        if v["spec"]["finish"] != "cancel_first" {
            click(&mut app, ids[1]);
            enter(&mut app, "NumberOfSurfaces=2");
            enter(&mut app, "MatchMethod=SamplePoints");
            enter(&mut app, "SampleNumber=4");
            enter(&mut app, "OutputLayer=CurrentLayer");
        }
        enter(
            &mut app,
            if v["command"]["success"] == true {
                ""
            } else {
                "Cancel"
            },
        );
        let native = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(app.document.objects().len(), native.len(), "{}", v["case"]);
        let selection = |rows: &serde_json::Value| {
            rows.as_array()
                .unwrap()
                .iter()
                .map(|r| r["selected"].as_bool().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            app.document
                .objects()
                .map(|o| app.document.is_selected(o.id()))
                .collect::<Vec<_>>(),
            selection(&v["command"]["after_script"]),
            "{}",
            v["case"]
        );
        for (object, n) in app.document.objects().zip(native) {
            let s = match object.geometry() {
                Geometry::NurbsSurface(s) => s,
                Geometry::Brep(b) => b.faces()[0].surface(),
                _ => panic!(),
            };
            for (i, p) in n["samples"].as_array().unwrap().iter().enumerate() {
                let u = *s.domain_u().start()
                    + (*s.domain_u().end() - *s.domain_u().start()) * (i % 9) as f64 / 8.;
                let w = *s.domain_v().start()
                    + (*s.domain_v().end() - *s.domain_v().start()) * (i / 9) as f64 / 8.;
                let expected =
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                        .unwrap();
                assert!(
                    s.evaluate(u, w).unwrap().distance_to(expected).unwrap() < 1e-7,
                    "{}",
                    v["case"]
                );
            }
        }
        if v["command"]["success"] == true {
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(
                app.document
                    .objects()
                    .map(|o| app.document.is_selected(o.id()))
                    .collect::<Vec<_>>(),
                selection(&v["undo"]["after_script"])
            );
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().len(), native.len());
        } else {
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!app.document.can_undo());
        }
    }
}

#[test]
fn unequal_control_nets_preview_and_accept_three_native_tweens_with_atomic_history() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tween_surfaces_control_followup.json"
    ))
    .unwrap();
    let v = &q["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["value"]["case"] == "curved_rows_number3")
        .unwrap()["value"];
    let mut app = test_app();
    for source in v["before"].as_array().unwrap() {
        let d = &source["definition"];
        let controls = d["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                viboceros_geometry::WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        let s = viboceros_geometry::NurbsSurface::try_new_rational(
            d["degree"][0].as_u64().unwrap() as usize,
            d["degree"][1].as_u64().unwrap() as usize,
            d["control_count"][0].as_u64().unwrap() as usize,
            d["control_count"][1].as_u64().unwrap() as usize,
            controls,
            serde_json::from_value(d["knots_u"].clone()).unwrap(),
            serde_json::from_value(d["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        app.document
            .add_geometry(Geometry::NurbsSurface(s))
            .unwrap();
    }
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "TweenSurfaces MatchMethod=None NumberOfSurfaces=3",
    );
    for object in &before {
        click(&mut app, object.id());
    }
    let scene = app.tween_surfaces_prompt.as_ref().unwrap().scene().unwrap();
    assert_eq!(scene.objects().len(), 5);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    for (object, native) in scene.objects().skip(2).zip(
        v["command"]["after_script"]
            .as_array()
            .unwrap()
            .iter()
            .skip(2),
    ) {
        let Geometry::Brep(b) = object.geometry() else {
            panic!()
        };
        let s = b.faces()[0].surface();
        for (i, point) in native["samples"].as_array().unwrap().iter().enumerate() {
            let u = *s.domain_u().start()
                + (*s.domain_u().end() - *s.domain_u().start()) * (i % 9) as f64 / 8.;
            let w = *s.domain_v().start()
                + (*s.domain_v().end() - *s.domain_v().start()) * (i / 9) as f64 / 8.;
            let expected =
                Point3::try_from(serde_json::from_value::<[f64; 3]>(point.clone()).unwrap())
                    .unwrap();
            assert!(s.evaluate(u, w).unwrap().distance_to(expected).unwrap() < 1e-6);
        }
    }
    enter(&mut app, "");
    let accepted = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(accepted.len(), 5);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        accepted
    );
}

#[test]
fn surface_rebuild_command_first_picks_replaces_and_undoes_in_one_step() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(
        &mut app,
        "Rebuild UPointCount=5 VPointCount=4 UDegree=3 VDegree=2 ReTrim=No",
    );
    click(&mut app, ids[0]);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 2);
    let Geometry::Brep(b) = app.document.object(ids[0]).unwrap().geometry() else {
        panic!()
    };
    let surface = b.faces()[0].surface();
    assert_eq!((surface.degree_u(), surface.degree_v()), (3, 2));
    assert_eq!(
        (
            surface.control_point_count_u(),
            surface.control_point_count_v()
        ),
        (5, 4)
    );
    assert_eq!(app.document.undo_label(), Some("Rebuild"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

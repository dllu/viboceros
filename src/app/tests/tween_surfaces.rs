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

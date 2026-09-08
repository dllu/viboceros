use super::super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::{LineSegment, NurbsSurface};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn surface(offset: f64) -> NurbsSurface {
    NurbsSurface::try_new(
        2,
        1,
        4,
        3,
        (0..3)
            .flat_map(|j| (0..4).map(move |i| point(offset + [0., 1., 3., 4.][i], j as f64, 0.)))
            .collect(),
        vec![-2., -2., -2., 1., 4., 4., 4.],
        vec![10., 10., 13., 18., 18.],
    )
    .unwrap()
}
fn fixture() -> (VibocerosApp, [ObjectId; 4]) {
    let mut app = test_app();
    let a = app
        .document
        .add_geometry(Geometry::NurbsSurface(surface(0.)))
        .unwrap();
    let b = app
        .document
        .add_geometry(Geometry::NurbsSurface(surface(10.)))
        .unwrap();
    let c = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(20., 0., 0.), point(24., 0., 0.), Tolerance::DEFAULT)
                .unwrap(),
        ))
        .unwrap();
    let p = app
        .document
        .add_geometry(Geometry::Point(point(30., 0., 0.)))
        .unwrap();
    let ids = [a, b, c, p];
    app.document.add_group(Some("all".into()), ids).unwrap();
    (app, ids)
}
fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}
fn preferences(app: &VibocerosApp) -> (bool, &'static str) {
    let p = app
        .commands
        .object_selection_prompt("ConvertToSingleSpans")
        .unwrap()
        .unwrap();
    (p.options[0].value, p.choices[0].value)
}

#[test]
fn single_span_direction_chooser_overrides_global_alias_and_uses_atomic_toggles() {
    let (mut app, ids) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "ConvertSurfaceToSingleSpans");
    enter(&mut app, "Direction U");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.command_input, "Direction U");
    pick(&mut app, ids[2]);
    pick(&mut app, ids[3]);
    assert_eq!(app.document.selected_object_count(), 0);
    pick(&mut app, ids[1]);
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    let pending = app.object_prompt.clone();
    enter(&mut app, "Toggle");
    assert_eq!(app.object_prompt, pending);
    enter(&mut app, "Direction");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().phase,
        ObjectPromptPhase::Choice(0)
    );
    enter(&mut app, "Invalid");
    assert_eq!(app.command_input, "Invalid");
    enter(&mut app, "_U");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().phase,
        ObjectPromptPhase::Options
    );
    assert_eq!(preferences(&app), (false, "U"));
    enter(&mut app, "Toggle Toggle Toggle");
    assert_eq!(preferences(&app), (false, "V"));
    let pending = app.object_prompt.clone();
    for invalid in [
        "Direction=Both Toggle",
        "DeleteInput=Yes Direction=Invalid",
        "Direction U Direction V",
    ] {
        enter(&mut app, invalid);
        assert_eq!(app.object_prompt, pending);
        assert_eq!(preferences(&app), (false, "V"));
    }
    enter(&mut app, "Direction U Toggle");
    enter(&mut app, "DeleteInput=Yes");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    pick(&mut app, ids[0]);
    enter(&mut app, "SelNone");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1], ids[0]]
    );
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    let outputs = app
        .document
        .objects()
        .filter(|o| !ids.contains(&o.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    for (index, object) in outputs.iter().enumerate() {
        let Geometry::NurbsSurface(s) = object.geometry() else {
            panic!()
        };
        assert_eq!(s.domain_u(), -2.0..=4.0);
        assert_eq!(s.domain_v(), 0.0..=1.0);
        assert_eq!(
            s.evaluate(-2., 0.).unwrap().x(),
            if index < 2 { 10. } else { 0. }
        );
        assert!(object.group_ids().is_empty());
    }
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
    assert_eq!(preferences(&app), (true, "V"));
}

#[test]
fn single_span_cancellation_remembers_only_options_that_reached_confirmation() {
    for post in [false, true] {
        for target in [
            ObjectPromptPhase::Selecting,
            ObjectPromptPhase::Options,
            ObjectPromptPhase::Choice(0),
        ] {
            if !post && target == ObjectPromptPhase::Selecting {
                continue;
            }
            let (mut app, ids) = fixture();
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
            app.commands
                .execute(
                    &mut app.document,
                    "ConvertToSingleSpans Direction=U DeleteInput=Yes",
                )
                .unwrap();
            enter(&mut app, "Undo");
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            app.document
                .select_objects_direct([ids[if post { 3 } else { 0 }]], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "ConvertToSingleSpans Direction=V DeleteInput=No");
            if post {
                assert_eq!(preferences(&app), (true, "U"));
                assert_eq!(app.document.selected_object_count(), 0);
                pick(&mut app, ids[0]);
                if target != ObjectPromptPhase::Selecting {
                    enter(&mut app, "");
                }
            }
            if target == ObjectPromptPhase::Choice(0) {
                enter(&mut app, "Direction");
            }
            assert_eq!(app.object_prompt.as_ref().unwrap().phase, target);
            app.cancel_object_prompt(true);
            assert_eq!(
                preferences(&app),
                if target == ObjectPromptPhase::Selecting {
                    (true, "U")
                } else {
                    (false, "V")
                }
            );
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(app.document.selected_object_count(), usize::from(!post));
            assert!(app.document.can_redo());
        }
    }
}

#[test]
fn single_span_postselected_noop_still_asks_options_accepts_memory_and_preserves_redo() {
    let (mut app, ids) = fixture();
    let one = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            surface(20.).try_bezier_patches().unwrap().remove(0),
        ))
        .unwrap();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    app.commands
        .execute(
            &mut app.document,
            "ConvertToSingleSpans Direction=U DeleteInput=Yes",
        )
        .unwrap();
    enter(&mut app, "Undo");
    enter(&mut app, "SelNone");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ConvertToSingleSpans");
    pick(&mut app, one);
    enter(&mut app, "");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().phase,
        ObjectPromptPhase::Options
    );
    enter(&mut app, "Direction=V DeleteInput=No");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(app.document.can_redo());
    assert_eq!(preferences(&app), (false, "V"));
}

#[test]
fn single_span_direction_subprompt_survives_transparent_controls_and_nested_cplane() {
    let (mut app, ids) = fixture();
    app.document.set_objects_locked([ids[1]], true).unwrap();
    enter(&mut app, "ConvertToSingleSpans");
    enter(&mut app, "SelAll");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    enter(&mut app, "");
    enter(&mut app, "Direction");
    let pending = app.object_prompt.clone();
    for input in [
        "SetDisplayMode Viewport=All Mode=Ghosted",
        "CPlane World Front",
        "Osnap",
        "Snap",
    ] {
        enter(&mut app, input);
        assert_eq!(app.object_prompt, pending);
    }
    enter(&mut app, "CPlane");
    assert!(app.plane_prompt.is_some());
    app.cancel_plane_prompt();
    assert_eq!(app.object_prompt, pending);
    enter(&mut app, "");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().phase,
        ObjectPromptPhase::Options
    );
    enter(&mut app, "Point 40,0,0");
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(ids[0]).is_some());
}

use super::*;
use crate::app::edge_commands::EdgePrompt;
use crate::viewport::EdgePick;

fn fixture(app: &mut VibocerosApp) -> EdgePick {
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 10.], [0., 12.], [0., 14.]],
        app.document.tolerance(),
    )
    .unwrap();
    EdgePick {
        object: app.document.add_geometry(Geometry::Brep(brep)).unwrap(),
        edge: 0,
    }
}
fn submit(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn split_one_shot_survives_distance_options_then_clears_after_accepted_point_or_finish() {
    use viboceros_drafting::{ObjectSnapKind, ObjectSnapModes};
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "SplitEdge");
    assert!(!app.try_one_shot_snap("Cen")); // Edge selection is not point input.
    app.accept_edge_click(vec![pick]);
    submit(&mut app, "Cen");
    assert_eq!(
        app.effective_snap_modes(),
        ObjectSnapModes::only(ObjectSnapKind::Center)
    );
    submit(&mut app, "2");
    assert_eq!(app.one_shot_snap_label(), Some("Cen"));
    app.accept_split_parameter(f64::NAN);
    assert_eq!(app.one_shot_snap_label(), Some("Cen"));
    app.accept_split_parameter(2.);
    assert_eq!(app.one_shot_snap_label(), None);
    submit(&mut app, "NoSnap");
    submit(&mut app, "");
    assert!(app.edge_prompt.is_none());
    assert_eq!(app.one_shot_snap_label(), None);
    assert_eq!(app.effective_snap_modes(), ObjectSnapModes::ALL);
}

#[test]
fn split_collects_typed_and_picked_points_then_finishes_as_one_undo_even_on_escape() {
    for escape in [false, true] {
        let mut app = test_app();
        let pick = fixture(&mut app);
        let before = app.document.object(pick.object).unwrap().clone();
        let history = app.document.undo_label().map(str::to_owned);
        submit(&mut app, "SplitEdge");
        app.accept_edge_click(vec![pick]);
        assert!(matches!(app.edge_prompt, Some(EdgePrompt::SplitPoints(_))));
        submit(&mut app, "w2,0,0");
        app.handle_viewport_action(ViewportOutput {
            edge_parameter: Some(6.),
            ..Default::default()
        });
        assert_eq!(app.document.object(pick.object).unwrap(), &before);
        assert_eq!(app.document.undo_label(), history.as_deref());
        if escape {
            app.cancel_interactive_command(true);
        } else {
            submit(&mut app, "");
        }
        assert!(app.edge_prompt.is_none());
        let Geometry::Brep(brep) = app.document.object(pick.object).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(brep.edges().len(), 14);
        assert_eq!(app.document.undo_label(), Some("SplitEdge"));
        submit(&mut app, "Undo");
        assert_eq!(app.document.object(pick.object).unwrap(), &before);
        assert_eq!(app.document.undo_label(), history.as_deref());
        submit(&mut app, "Redo");
        assert_eq!(app.document.undo_label(), Some("SplitEdge"));
    }
}

#[test]
fn split_shares_ambiguity_choices_and_preserves_collected_points_during_cplane_input() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "SplitEdge");
    app.accept_edge_click(vec![pick, EdgePick { edge: 1, ..pick }]);
    submit(&mut app, "1");
    submit(&mut app, "w2,0,0");
    submit(&mut app, "CPlane");
    assert!(app.plane_prompt.is_some());
    submit(&mut app, "w0,0,7");
    assert!(app.plane_prompt.is_none());
    let Some(EdgePrompt::SplitPoints(selection)) = &app.edge_prompt else {
        panic!()
    };
    assert_eq!(selection.parameters(), &[2.]);
    submit(&mut app, "w6,0,0");
    submit(&mut app, "Cancel");
    assert_eq!(app.document.undo_label(), Some("SplitEdge"));
}

#[test]
fn duplicate_batch_and_stale_source_never_partially_commit() {
    for stale in [false, true] {
        let mut app = test_app();
        let pick = fixture(&mut app);
        submit(&mut app, "Point 1,2,3");
        submit(&mut app, "Undo");
        submit(&mut app, "SplitEdge");
        app.accept_edge_click(vec![pick]);
        submit(&mut app, "w2,0,0");
        if stale {
            app.document
                .set_objects_locked([pick.object], true)
                .unwrap();
        } else {
            submit(&mut app, "w2,0,0");
        }
        let before = format!("{:?}", app.document);
        submit(&mut app, "");
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.edge_prompt.is_none());
    }
}

#[test]
fn typed_distance_persists_through_points_and_cplane_edits_and_resets_explicitly() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "SplitEdge");
    app.accept_edge_click(vec![pick]);
    submit(&mut app, "2"); // No edge anchor yet: do not reinterpret as a point.
    submit(&mut app, "w0,0,0");
    submit(&mut app, "-2");
    submit(&mut app, "w8,0,0");
    submit(&mut app, "CPlane");
    submit(&mut app, "w0,0,7");
    let Some(EdgePrompt::SplitPoints(selection)) = &app.edge_prompt else {
        panic!()
    };
    assert_eq!(selection.parameters(), &[0., 2.]);
    assert_eq!(selection.distance(), Some(2.));
    let parameter = selection.distance_parameters().unwrap()[1];
    app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(parameter),
        ..Default::default()
    });
    assert!((app.last_point.unwrap().to_array()[0] - 4.).abs() < 1e-12);
    submit(&mut app, "0");
    submit(&mut app, "w9,0,0");
    let Some(EdgePrompt::SplitPoints(selection)) = &app.edge_prompt else {
        panic!()
    };
    assert_eq!(selection.parameters(), &[0., 2., 4., 9.]);
    assert_eq!(selection.distance(), None);
    submit(&mut app, "Cancel");
    assert_eq!(app.document.undo_label(), Some("SplitEdge"));
    submit(&mut app, "SplitEdge");
    app.accept_edge_click(vec![pick]);
    let Some(EdgePrompt::SplitPoints(selection)) = &app.edge_prompt else {
        panic!()
    };
    assert_eq!(selection.distance(), None);
}

#[test]
fn unreachable_distance_and_invalid_numbers_preserve_the_batch() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "SplitEdge");
    app.accept_edge_click(vec![pick]);
    submit(&mut app, "w4,0,0");
    submit(&mut app, "20");
    submit(&mut app, "w9,0,0");
    submit(&mut app, "NaN");
    let Some(EdgePrompt::SplitPoints(selection)) = &app.edge_prompt else {
        panic!()
    };
    assert_eq!(selection.parameters(), &[4.]);
    assert_eq!(selection.distance(), Some(20.));
    assert_eq!(selection.distance_parameters(), Some(&[][..]));
}

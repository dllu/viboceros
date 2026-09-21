use super::*;
use crate::app::tests::{point, test_app};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn modes_are_persistent_but_one_shot_lasts_for_one_accepted_point() {
    let mut app = test_app();
    app.snaps.persistent = ObjectSnapModes::only(ObjectSnapKind::End);
    enter(&mut app, "Line");
    enter(&mut app, "_Cen");
    assert_eq!(
        app.effective_snap_modes(),
        ObjectSnapModes::only(ObjectSnapKind::Center)
    );
    enter(&mut app, "ZE");
    assert_eq!(app.one_shot_snap_label(), Some("Cen"));
    assert!(app.accept_drafting_point(point(1., 2., 3.)));
    assert_eq!(
        app.effective_snap_modes(),
        ObjectSnapModes::only(ObjectSnapKind::End)
    );
    assert_eq!(app.one_shot_snap_label(), None);
    enter(&mut app, "NoSnap");
    assert_eq!(app.effective_snap_modes(), ObjectSnapModes::NONE);
    enter(&mut app, "4,5,6");
    assert!(app.active_command.is_none());
    assert_eq!(app.one_shot_snap_label(), None);
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn one_shot_overrides_disabled_persistent_snaps_without_reenabling_them() {
    let mut app = test_app();
    enter(&mut app, "DisableOsnap Disable");
    enter(&mut app, "Points");
    enter(&mut app, "Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::Points));
    assert_eq!(
        app.effective_snap_modes(),
        ObjectSnapModes::only(ObjectSnapKind::Point)
    );
    app.accept_drafting_point(point(1., 2., 3.));
    assert!(!app.osnap);
    assert_eq!(app.effective_snap_modes(), ObjectSnapModes::NONE);
    enter(&mut app, "End");
    app.cancel_interactive_command(true);
    enter(&mut app, "Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::Point));
    assert_eq!(app.one_shot_snap_label(), None);
    assert_eq!(app.snaps.persistent, ObjectSnapModes::ALL);
}

#[test]
fn invalid_points_keep_override_but_finishing_or_replacing_commands_clears_it() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    enter(&mut app, "End");
    enter(&mut app, "1,NaN,0");
    assert_eq!(app.one_shot_snap_label(), Some("End"));
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert_eq!(app.one_shot_snap_label(), Some("End"));
    enter(&mut app, "Circle");
    assert_eq!(app.one_shot_snap_label(), None);
    app.cancel_interactive_command(false);
    enter(&mut app, "Polyline");
    enter(&mut app, "0,0,0");
    enter(&mut app, "1,0,0");
    enter(&mut app, "Mid");
    enter(&mut app, "");
    assert_eq!(app.one_shot_snap_label(), None);
}

#[test]
fn transparent_plane_prompt_owns_a_separate_one_shot() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "Cen");
    enter(&mut app, "CPlane 3Point");
    assert_eq!(app.one_shot_snap_label(), None);
    enter(&mut app, "End");
    assert_eq!(app.one_shot_snap_label(), Some("End"));
    assert!(app.accept_plane_prompt_point(point(0., 0., 0.)));
    assert_eq!(app.one_shot_snap_label(), None);
    enter(&mut app, "Quad");
    assert!(!app.accept_plane_prompt_point(point(0., 0., 0.)));
    assert_eq!(app.one_shot_snap_label(), Some("Quad"));
    app.cancel_plane_prompt();
    assert_eq!(app.one_shot_snap_label(), Some("Cen"));
    assert!(app.accept_drafting_point(point(1., 0., 0.)));
    assert_eq!(app.one_shot_snap_label(), None);
}

#[test]
fn isolation_restores_previous_modes_and_explicit_edits_reset_restore_state() {
    let mut controls = SnapControls::default();
    controls.set(ObjectSnapKind::Mid, false);
    let prior = controls.persistent;
    controls.isolate(ObjectSnapKind::Center);
    assert_eq!(
        controls.persistent,
        ObjectSnapModes::only(ObjectSnapKind::Center)
    );
    controls.isolate(ObjectSnapKind::Center);
    assert_eq!(controls.persistent, prior);
    controls.isolate(ObjectSnapKind::End);
    controls.set(ObjectSnapKind::Point, true);
    let changed = controls.persistent;
    controls.isolate(ObjectSnapKind::Center);
    controls.isolate(ObjectSnapKind::Center);
    assert_eq!(controls.persistent, changed);
}

#[test]
fn snap_words_are_strict_and_never_replace_idle_point_commands() {
    for word in [
        "Point", "_End", "'Mid", "Cen", "Center", "Quadrant", "NoSnap",
    ] {
        assert!(parse_one_shot(word).is_some());
    }
    for word in ["", "Point 1,2,3", "Mid Delete", "NoSnap On", "Near"] {
        assert!(parse_one_shot(word).is_none());
    }
    let mut app = test_app();
    assert!(!app.try_one_shot_snap("Point"));
    enter(&mut app, "Point");
    assert_eq!(app.active_command, Some(InteractiveCommand::Point));
    assert!(app.try_one_shot_snap("Point"));
    assert_eq!(app.one_shot_snap_label(), Some("Point"));
}

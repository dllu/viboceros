use super::*;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn parse_action(input: &str, frame: Frame3) -> PlaneAction {
    parse(input, frame, None, Tolerance::DEFAULT)
        .unwrap()
        .unwrap()
}
fn edited(input: &str, frame: Frame3) -> Frame3 {
    let PlaneAction::Set(frame) = parse_action(input, frame) else {
        panic!("edit")
    };
    frame
}

#[test]
fn world_plane_axes_are_exact_oriented_and_reset_the_origin() {
    let initial = WorldPlane::Right.frame().with_origin(point(7., 8., 9.));
    for (preset, expected) in [
        ("Top", [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]),
        ("Bottom", [[1., 0., 0.], [0., -1., 0.], [0., 0., -1.]]),
        ("Front", [[1., 0., 0.], [0., 0., 1.], [0., -1., 0.]]),
        ("Back", [[-1., 0., 0.], [0., 0., 1.], [0., 1., 0.]]),
        ("Right", [[0., 1., 0.], [0., 0., 1.], [1., 0., 0.]]),
        ("Left", [[0., -1., 0.], [0., 0., 1.], [-1., 0., 0.]]),
    ] {
        let frame = edited(&format!("'_-cPlAnE _wOrLd _{preset}"), initial);
        assert_eq!(frame.origin(), point(0., 0., 0.));
        assert_eq!(frame.axes().map(|a| a.as_vector().to_array()), expected);
    }
}

#[test]
fn plane_edits_resolve_local_world_and_relative_points_before_changing_the_frame() {
    let initial = WorldPlane::Front.frame().with_origin(point(10., 20., 30.));
    assert_eq!(
        edited("CPlane 1,2,3", initial).origin(),
        point(11., 17., 32.)
    );
    assert_eq!(edited("CPlane w1,2,3", initial).origin(), point(1., 2., 3.));
    assert_eq!(edited("CPlane 3Point 0 r1,0 r0,1", initial), initial);
    assert_eq!(
        edited("CPlane Elevation 4", initial).origin(),
        point(10., 16., 30.)
    );
    assert_eq!(
        edited("CPlane Through w100,15,200", initial).origin(),
        point(10., 15., 30.)
    );
    assert_eq!(initial.origin(), point(10., 20., 30.));
}

#[test]
fn rotation_moves_the_origin_and_axes_and_keeps_a_right_handed_frame() {
    let initial = WorldPlane::Top.frame().with_origin(point(2., 0., 3.));
    let result = edited("CPlane Rotate w0,0,0 w0,0,2 90", initial);
    assert!(result.origin().distance_to(point(0., 2., 3.)).unwrap() < 1e-14);
    assert!(
        result
            .x_axis()
            .as_vector()
            .dot(Vector3::try_new(0., 1., 0.).unwrap())
            .unwrap()
            > 1.0 - 1e-15
    );
    assert_eq!(result.z_axis(), initial.z_axis());
}

#[test]
fn invalid_commands_and_degenerate_frames_cannot_mutate_plane_history() {
    let initial = WorldPlane::Top.frame();
    let state = ConstructionPlaneState::new(initial);
    for input in [
        "CPlane World",
        "CPlane World Camera",
        "CPlane World Top Extra",
        "CPlane Unknown",
        "CPlane 3Point 0 0 1,2",
        "CPlane 3Point 0 1,0 2,0",
        "CPlane Rotate 0 0 45",
        "CPlane Elevation NaN",
        "CPlane Rotate 0 0,0,1 inf",
        "CPlane r1,2",
        "CPlane 1,2,3,4",
    ] {
        assert!(
            parse(input, state.frame(), None, Tolerance::DEFAULT)
                .unwrap()
                .is_err(),
            "{input}"
        );
        assert_eq!(state, ConstructionPlaneState::new(initial));
    }
    assert!(parse("Line 0 1,2", initial, None, Tolerance::DEFAULT).is_none());
}

#[test]
fn plane_history_is_bounded_independent_and_branching() {
    let initial = WorldPlane::Top.frame();
    let mut state = ConstructionPlaneState::new(initial);
    assert!(!state.undo() && !state.redo());
    state.set(initial.with_origin(point(1., 0., 0.)));
    state.set(initial.with_origin(point(2., 0., 0.)));
    assert!(state.undo());
    assert_eq!(state.frame().origin(), point(1., 0., 0.));
    assert!(state.redo());
    assert_eq!(state.frame().origin(), point(2., 0., 0.));
    assert!(state.undo());
    state.set(initial.with_origin(point(3., 0., 0.)));
    assert!(!state.redo());
    for i in 4..100 {
        state.set(initial.with_origin(point(f64::from(i), 0., 0.)));
    }
    let mut undos = 0;
    while state.undo() {
        undos += 1;
    }
    assert_eq!(undos, HISTORY_LIMIT);
    let mut redos = 0;
    while state.redo() {
        redos += 1;
    }
    assert_eq!(redos, HISTORY_LIMIT);
}

#[test]
fn incomplete_commands_start_their_own_typed_prompts() {
    for (input, kind) in [
        ("CPlane", PlanePromptKind::Origin),
        ("CPlane 3Point", PlanePromptKind::ThreePoint),
        ("CPlane Elevation", PlanePromptKind::Elevation),
        ("CPlane Through", PlanePromptKind::Through),
        ("CPlane Rotate", PlanePromptKind::Rotate),
    ] {
        assert_eq!(
            parse_action(input, WorldPlane::Top.frame()),
            PlaneAction::Prompt(kind)
        );
    }
}

#[test]
fn successful_no_op_plane_edits_still_record_history_and_clear_redo() {
    let initial = WorldPlane::Top.frame();
    let a = initial.with_origin(point(1., 2., 3.));
    let b = initial.with_origin(point(4., 5., 6.));
    let mut state = ConstructionPlaneState::new(initial);
    assert!(state.set(a));
    assert!(!state.set(a));
    assert!(state.undo());
    assert_eq!(state.frame(), a);
    assert!(state.undo());
    assert_eq!(state.frame(), initial);
    state.set(b);
    state.undo();
    assert!(!state.set(initial));
    assert!(!state.redo());
}

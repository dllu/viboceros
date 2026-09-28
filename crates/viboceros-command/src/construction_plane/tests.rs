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
fn all_viewport_actions_resolve_the_active_plane_input_once() {
    let plane = WorldPlane::Front.frame().with_origin(point(10., 20., 30.));
    assert_eq!(
        parse_action("CPlane All 1,2,3", plane),
        PlaneAction::SetAllOrigin(point(11., 17., 32.))
    );
    assert_eq!(
        parse_action("CPlane Through All w4,5,6", plane),
        PlaneAction::SetThroughAll(point(4., 5., 6.))
    );
    assert_eq!(
        parse_action("CPlane All", plane),
        PlaneAction::Prompt(PlanePromptKind::AllOrigin)
    );
    assert_eq!(
        parse_action("CPlane Through All", plane),
        PlaneAction::Prompt(PlanePromptKind::ThroughAll)
    );
    assert_eq!(
        parse_action("_CPlane _All=_Yes w4,5,6", plane),
        PlaneAction::SetAllOrigin(point(4., 5., 6.))
    );
    assert_eq!(
        parse_action("CPlane Through All=No w4,5,6", plane),
        PlaneAction::Set(through(plane, point(4., 5., 6.)).unwrap())
    );
    assert_eq!(
        parse_action("CPlane Through All=Yes", plane),
        PlaneAction::Prompt(PlanePromptKind::ThroughAll)
    );
}

#[test]
fn view_option_is_a_camera_dependent_action() {
    let frame = WorldPlane::Front.frame();
    assert_eq!(
        parse_action("_CPlane _View", frame),
        PlaneAction::AlignToView
    );
    assert!(
        parse("CPlane View Extra", frame, None, Tolerance::DEFAULT)
            .unwrap()
            .is_err()
    );
}

#[test]
fn three_point_vertical_projects_x_and_z_axis_uses_normal_constructor() {
    let top = WorldPlane::Top.frame();
    let vertical = edited("CPlane 3Point w1,2,3 Vertical w4,5,7", top);
    assert_eq!(vertical.origin(), point(1., 2., 3.));
    assert_eq!(vertical.y_axis(), top.z_axis());
    let [x, y, z] = vertical.x_axis().as_vector().to_array();
    assert!((x - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-15);
    assert!((y - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-15);
    assert_eq!(z, 0.);

    let side = edited("_CPlane _3Point w1,2,3 _ZAxis w1,5,3", top);
    assert_eq!(side.origin(), point(1., 2., 3.));
    assert_eq!(side.x_axis().as_vector().to_array(), [0., 0., 1.]);
    assert_eq!(side.y_axis().as_vector().to_array(), [1., 0., 0.]);
    assert_eq!(side.z_axis().as_vector().to_array(), [0., 1., 0.]);

    let oblique = Frame3::try_from_directions(
        point(10., 20., 30.),
        Vector3::try_from([1., 1., 0.]).unwrap(),
        Vector3::try_from([-1., 1., 1.]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let vertical = edited("CPlane 3Point w1,2,3 Vertical w4,2,3", oblique);
    for (actual, expected) in vertical.x_axis().as_vector().to_array().into_iter().zip([
        0.9128709291752768,
        0.18257418583505536,
        -0.3651483716701108,
    ]) {
        assert!((actual - expected).abs() < 1e-14);
    }
    let z_axis = edited("CPlane 3Point w1,2,3 ZAxis w4,6,5", oblique);
    for (actual, expected) in z_axis
        .x_axis()
        .as_vector()
        .to_array()
        .into_iter()
        .zip([0.8, -0.6, 0.])
    {
        assert!((actual - expected).abs() < 1e-14);
    }
    assert!(
        parse(
            "CPlane 3Point w1,2,3 Vertical w1,2,6",
            top,
            None,
            Tolerance::DEFAULT
        )
        .unwrap()
        .is_err()
    );
    assert!(
        parse(
            "CPlane 3Point w1,2,3 ZAxis w1,2,3",
            top,
            None,
            Tolerance::DEFAULT
        )
        .unwrap()
        .is_err()
    );
}

#[test]
fn origin_and_through_remember_independent_all_options() {
    let frame = WorldPlane::Top.frame();
    let mut options = PlaneOptions::default();
    let cases = [
        (
            "CPlane All=Yes",
            PlaneAction::Prompt(PlanePromptKind::AllOrigin),
            true,
            false,
        ),
        (
            "CPlane",
            PlaneAction::Prompt(PlanePromptKind::AllOrigin),
            true,
            false,
        ),
        (
            "CPlane Through",
            PlaneAction::Prompt(PlanePromptKind::Through),
            true,
            false,
        ),
        (
            "CPlane Through All",
            PlaneAction::Prompt(PlanePromptKind::ThroughAll),
            true,
            true,
        ),
        (
            "CPlane Through",
            PlaneAction::Prompt(PlanePromptKind::ThroughAll),
            true,
            true,
        ),
        (
            "CPlane All",
            PlaneAction::Prompt(PlanePromptKind::Origin),
            false,
            true,
        ),
        (
            "CPlane Through All=No",
            PlaneAction::Prompt(PlanePromptKind::Through),
            false,
            false,
        ),
    ];
    for (input, expected, origin_all, through_all) in cases {
        let parsed = parse_with_options(input, frame, None, Tolerance::DEFAULT, options).unwrap();
        assert_eq!(parsed.action, Ok(expected), "{input}");
        options = parsed.options;
        assert_eq!(options.origin_all, origin_all, "{input}");
        assert_eq!(options.through_all, through_all, "{input}");
    }
    let parsed = parse_with_options(
        "CPlane Through All=Yes bad",
        frame,
        None,
        Tolerance::DEFAULT,
        options,
    )
    .unwrap();
    assert!(parsed.action.is_err());
    assert!(parsed.options.through_all);
    assert_eq!(
        parse_with_options(
            "CPlane Through w1,2,3",
            frame,
            None,
            Tolerance::DEFAULT,
            parsed.options
        )
        .unwrap()
        .action,
        Ok(PlaneAction::SetThroughAll(point(1., 2., 3.)))
    );
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

use super::*;

#[test]
fn mesh_snap_switch_uses_enable_disable_toggle_and_preserves_other_state() {
    let mut current = state();
    let initial = current.clone();
    for (input, expected) in [
        ("SnapToMeshes Enable", true),
        ("SnapToMeshes Enable", true),
        ("'_SnapToMeshes _Toggle", false),
        ("SnapToMeshes Disable", false),
        ("SnapToMeshes Toggle", true),
    ] {
        current.apply(parse(input).unwrap().unwrap()).unwrap();
        assert_eq!(current.snap_to_meshes, expected);
        let mut unchanged = current.clone();
        unchanged.snap_to_meshes = initial.snap_to_meshes;
        assert_eq!(unchanged, initial);
    }
    for input in [
        "SnapToMeshes",
        "SnapToMeshes On",
        "SnapToMeshes Enable _Delete",
        "SnapToMeshes Enable Disable",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn snap_size_parses_viewport_scope_and_rejects_invalid_spacing() {
    for (input, spacing, apply_to) in [
        ("SnapSize", None, ViewportTarget::Active),
        ("SnapSize 0.25", Some(0.25), ViewportTarget::Active),
        (
            "'_SnapSize _ApplyTo=_AllViewports 2",
            Some(2.0),
            ViewportTarget::All,
        ),
        (
            "SnapSize 0.5 ApplyTo=ActiveViewport",
            Some(0.5),
            ViewportTarget::Active,
        ),
        ("SnapSize ApplyTo=AllViewports", None, ViewportTarget::All),
    ] {
        let command = InterfaceCommand::SnapSize {
            spacing: spacing.map(|value| SnapSpacing::try_new(value).unwrap()),
            apply_to,
        };
        assert_eq!(parse(input), Some(Ok(command)));
        let mut current = state();
        let before = current.clone();
        current.apply(command).unwrap();
        assert_eq!(current, before);
    }
    for input in [
        "SnapSize 0",
        "SnapSize -1",
        "SnapSize NaN",
        "SnapSize Infinity",
        "SnapSize 1e999",
        "SnapSize bad",
        "SnapSize 1 2",
        "SnapSize 1 ApplyTo=Other",
        "SnapSize 1 ApplyTo=AllViewports ApplyTo=ActiveViewport",
    ] {
        assert!(
            matches!(parse(input), Some(Err(InterfaceError::Usage(_)))),
            "{input}"
        );
    }
    assert!(SnapSpacing::try_new(f64::from_bits(1)).is_some());
}

#[test]
fn zoom_factor_is_finite_positive_and_does_not_mutate_interface_state() {
    for (input, value) in [
        ("Zoom Factor 2", 2.0),
        ("'_Zoom _Factor 0.5", 0.5),
        ("zoom factor 1", 1.0),
        ("Zoom Factor 1e300", 1e300),
        ("Zoom Factor 5e-324", f64::from_bits(1)),
    ] {
        let action = InterfaceCommand::ZoomFactor(ZoomFactor::try_new(value).unwrap());
        assert_eq!(parse(input), Some(Ok(action)));
        let mut current = state();
        let original = current.clone();
        current.apply(action).unwrap();
        assert_eq!(current, original);
    }
    for value in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(ZoomFactor::try_new(value).is_none());
    }
    assert_eq!(
        parse("Zoom Factor"),
        Some(Ok(InterfaceCommand::ZoomFactorPrompt))
    );
    for input in [
        "Zoom Factor 0",
        "Zoom Factor -0",
        "Zoom Factor -1",
        "Zoom Factor NaN",
        "Zoom Factor inf",
        "Zoom Factor 1e999",
        "Zoom Factor 1e-999",
        "Zoom Factor abc",
        "Zoom Factor 2 extra",
        "ZE Factor 2",
        "Zoom All Factor 2",
    ] {
        assert!(
            matches!(parse(input), Some(Err(InterfaceError::Usage(_)))),
            "{input}"
        );
    }
}

#[test]
fn zoom_in_out_parse_and_leave_interface_state_unchanged() {
    for (input, expected) in [
        ("Zoom In", InterfaceCommand::ZoomIn),
        ("'_Zoom _Out", InterfaceCommand::ZoomOut),
        ("zoom in", InterfaceCommand::ZoomIn),
    ] {
        assert_eq!(parse(input), Some(Ok(expected)));
        let mut current = state();
        let original = current.clone();
        current.apply(expected).unwrap();
        assert_eq!(current, original);
    }
    for input in ["Zoom In extra", "Zoom All In", "Zoom Out 2"] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn viewport_navigation_commands_parse_without_arguments() {
    for (name, command) in [
        ("NextViewport", InterfaceCommand::NextViewport),
        ("PrevViewport", InterfaceCommand::PrevViewport),
        ("NextOrthoViewport", InterfaceCommand::NextOrthoViewport),
        (
            "NextPerspectiveViewport",
            InterfaceCommand::NextPerspectiveViewport,
        ),
    ] {
        assert_eq!(parse(name), Some(Ok(command)));
        assert_eq!(parse(&format!("'_{name}")), Some(Ok(command)));
        assert!(matches!(
            parse(&format!("{name} extra")),
            Some(Err(InterfaceError::Usage(_)))
        ));
        let mut current = state();
        let original = current.clone();
        current.apply(command).unwrap();
        assert_eq!(current, original);
    }
}

#[test]
fn zoom_ends_parses_all_without_changing_interface_state() {
    for input in ["ZoomEnds", "'_ZoomEnds _All", "zoomends all"] {
        assert_eq!(parse(input), Some(Ok(InterfaceCommand::ZoomEnds)));
        let mut current = state();
        let original = current.clone();
        current.apply(InterfaceCommand::ZoomEnds).unwrap();
        assert_eq!(current, original);
    }
    for (input, command) in [
        ("ZoomEnds Current", InterfaceCommand::ZoomEndsCurrent),
        ("'_ZoomEnds _Next", InterfaceCommand::ZoomEndsNext),
        ("zoomends previous", InterfaceCommand::ZoomEndsPrevious),
        ("ZoomEnds Mark", InterfaceCommand::ZoomEndsMark),
    ] {
        assert_eq!(parse(input), Some(Ok(command)));
        let mut current = state();
        let original = current.clone();
        current.apply(command).unwrap();
        assert_eq!(current, original);
    }
    for input in ["ZoomEnds Mark extra", "ZoomEnds All extra"] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn show_ends_commands_parse_without_changing_interface_state() {
    for (input, command) in [
        ("ShowEnds", InterfaceCommand::ShowEnds),
        ("'_ShowEndsOff", InterfaceCommand::ShowEndsOff),
        ("showends", InterfaceCommand::ShowEnds),
    ] {
        assert_eq!(parse(input), Some(Ok(command)));
        let mut current = state();
        let original = current.clone();
        current.apply(command).unwrap();
        assert_eq!(current, original);
    }
    for input in ["ShowEnds All", "ShowEndsOff extra"] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn view_zoom_scale_option_requires_a_finite_positive_reciprocal() {
    for (input, value) in [
        ("Options View Zoom ScaleFactor=0.9", 0.9),
        ("'_Options _View _Zoom _ScaleFactor=1.25", 1.25),
        ("Options View Zoom ScaleFactor=1", 1.0),
        ("Options View Zoom ScaleFactor=1e300", 1e300),
    ] {
        let action = InterfaceCommand::SetZoomScale(ZoomScale::try_new(value).unwrap());
        assert_eq!(parse(input), Some(Ok(action)));
        let mut current = state();
        let original = current.clone();
        current.apply(action).unwrap();
        assert_eq!(current, original);
    }
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::from_bits(1)] {
        assert!(ZoomScale::try_new(value).is_none());
    }
    for input in [
        "Options",
        "Options View Zoom",
        "Options Zoom ScaleFactor=0.9",
        "Options View Zoom ScaleFactor=0",
        "Options View Zoom ScaleFactor=-1",
        "Options View Zoom ScaleFactor=NaN",
        "Options View Zoom ScaleFactor=5e-324",
        "Options View Zoom ScaleFactor=0.9 extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn zoom_extents_border_accepts_independent_scales_and_rejects_duplicates() {
    let cases = [
        (
            "SetZoomExtentsBorder",
            InterfaceCommand::SetZoomExtentsBorder {
                parallel: None,
                perspective: None,
            },
        ),
        (
            "'_SetZoomExtentsBorder _ParallelView=1.5",
            InterfaceCommand::SetZoomExtentsBorder {
                parallel: ZoomScale::try_new(1.5),
                perspective: None,
            },
        ),
        (
            "SetZoomExtentsBorder PerspectiveView=0.8 ParallelView=1",
            InterfaceCommand::SetZoomExtentsBorder {
                parallel: ZoomScale::try_new(1.0),
                perspective: ZoomScale::try_new(0.8),
            },
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(parse(input), Some(Ok(expected)));
        let mut current = state();
        let original = current.clone();
        current.apply(expected).unwrap();
        assert_eq!(current, original);
    }
    for input in [
        "SetZoomExtentsBorder ParallelView",
        "SetZoomExtentsBorder ParallelView=0",
        "SetZoomExtentsBorder PerspectiveView=NaN",
        "SetZoomExtentsBorder PerspectiveView=5e-324",
        "SetZoomExtentsBorder ParallelView=1 ParallelView=2",
        "SetZoomExtentsBorder Other=1",
    ] {
        assert!(
            matches!(parse(input), Some(Err(InterfaceError::Usage(_)))),
            "{input}"
        );
    }
}

#[test]
fn zoom_extents_is_a_validated_host_action() {
    for (input, expected) in [
        ("Zoom All Extents", InterfaceCommand::ZoomAllExtents),
        ("'_Zoom _All _Selected", InterfaceCommand::ZoomAllSelected),
        ("ZEA", InterfaceCommand::ZoomAllExtents),
        ("zsa", InterfaceCommand::ZoomAllSelected),
    ] {
        assert_eq!(parse(input), Some(Ok(expected)));
        let mut current = state();
        let original = current.clone();
        current.apply(expected).unwrap();
        assert_eq!(current, original);
    }
    for input in [
        "ZEA extra",
        "ZSA extra",
        "Zoom All",
        "Zoom All All Extents",
        "Zoom All Selected extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
    for input in ["Zoom Extents", "'_Zoom _Extents", "ZE", "ze"] {
        assert_eq!(parse(input), Some(Ok(InterfaceCommand::ZoomExtents)));
        let mut current = state();
        let original = current.clone();
        current.apply(InterfaceCommand::ZoomExtents).unwrap();
        assert_eq!(current, original);
    }
    for input in ["Zoom Selected", "'_Zoom _Selected", "ZS", "zs"] {
        assert_eq!(parse(input), Some(Ok(InterfaceCommand::ZoomSelected)));
        let mut current = state();
        let original = current.clone();
        current.apply(InterfaceCommand::ZoomSelected).unwrap();
        assert_eq!(current, original);
    }
    for input in ["Zoom", "Zoom Window", "'_Zoom _Window"] {
        assert_eq!(parse(input), Some(Ok(InterfaceCommand::ZoomWindow)));
    }
    for input in ["Zoom Target", "'_Zoom _Target", "ZT", "zt"] {
        assert_eq!(parse(input), Some(Ok(InterfaceCommand::ZoomTarget)));
    }
    for (input, expected) in [
        ("SelWindow", InterfaceCommand::SelWindow),
        ("W", InterfaceCommand::SelWindow),
        ("'_SelCrossing", InterfaceCommand::SelCrossing),
        ("c", InterfaceCommand::SelCrossing),
    ] {
        assert_eq!(parse(input), Some(Ok(expected)));
        let mut current = state();
        let original = current.clone();
        current.apply(expected).unwrap();
        assert_eq!(current, original);
    }
    for input in ["SelWindow extra", "SelCrossing extra", "W 1", "C 1"] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
    for (input, mode) in [
        ("SelRectangular", RectSelectionMode::Automatic),
        (
            "'_SelRectangular _SelectionMode=_Window",
            RectSelectionMode::Window,
        ),
        (
            "SelRectangular SelectionMode=Crossing",
            RectSelectionMode::Crossing,
        ),
        (
            "SelRectangular InvertWindow",
            RectSelectionMode::InvertWindow,
        ),
        (
            "SelRectangular SelectionMode=InvertCrossing",
            RectSelectionMode::InvertCrossing,
        ),
    ] {
        assert_eq!(
            parse(input),
            Some(Ok(InterfaceCommand::SelRectangular(mode)))
        );
    }
    for input in [
        "SelRectangular SelectionMode=Other",
        "SelRectangular SelectionMode",
        "SelRectangular Other=Crossing",
        "SelRectangular Window extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
    for (input, mode) in [
        ("SelCircular", RectSelectionMode::Crossing),
        (
            "SelCircular SelectionMode=Window",
            RectSelectionMode::Window,
        ),
        (
            "'_SelCircular _InvertWindow",
            RectSelectionMode::InvertWindow,
        ),
        (
            "SelCircular SelectionMode=InvertCrossing",
            RectSelectionMode::InvertCrossing,
        ),
    ] {
        assert_eq!(parse(input), Some(Ok(InterfaceCommand::SelCircular(mode))));
    }
    for input in [
        "SelCircular SelectionMode=Other",
        "SelCircular Other=Window",
        "SelCircular Window extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
    for input in [
        "Zoom Extents extra",
        "ZE extra",
        "ZS extra",
        "Zoom Selected extra",
        "Zoom Window extra",
        "Zoom Target extra",
        "ZT extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn view_history_commands_are_transparent_and_reject_arguments() {
    for (input, action) in [
        ("UndoView", InterfaceCommand::UndoView),
        ("'_RedoView", InterfaceCommand::RedoView),
    ] {
        assert_eq!(parse(input), Some(Ok(action)));
        let mut current = state();
        let before = current.clone();
        current.apply(action).unwrap();
        assert_eq!(current, before);
    }
    for input in ["UndoView 2", "RedoView All"] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn set_view_world_parses_all_standard_directions_without_mutating_model_state() {
    for view in WorldView::ALL {
        let input = format!("'_SetView _World _{}", view.label());
        let action = InterfaceCommand::SetViewWorld(view);
        assert_eq!(parse(&input), Some(Ok(action)));
        let mut current = state();
        let before = current.clone();
        current.apply(action).unwrap();
        assert_eq!(current, before);
    }
    for input in [
        "SetView",
        "SetView World",
        "SetView CPlane Perspective",
        "SetView World Isometric",
        "SetView World Top extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn set_view_cplane_parses_six_directions_and_preserves_interface_state() {
    for direction in WorldPlane::ALL {
        let input = format!("'_SetView _CPlane _{}", direction.label());
        let action = InterfaceCommand::SetViewCPlane(direction);
        assert_eq!(parse(&input), Some(Ok(action)));
        let mut current = state();
        let original = current.clone();
        current.apply(action).unwrap();
        assert_eq!(current, original);
    }
    for input in [
        "SetView CPlane",
        "SetView CPlane Perspective",
        "SetView CPlane Top extra",
    ] {
        assert!(matches!(parse(input), Some(Err(InterfaceError::Usage(_)))));
    }
}

#[test]
fn plan_parses_as_a_transparent_interface_command() {
    for input in ["Plan", "'_Plan", "plan"] {
        let action = parse(input).unwrap().unwrap();
        assert_eq!(action, InterfaceCommand::Plan);
        let mut current = state();
        let before = current.clone();
        current.apply(action).unwrap();
        assert_eq!(current, before);
    }
    assert!(matches!(
        parse("Plan Top"),
        Some(Err(InterfaceError::Usage("Plan")))
    ));
}

fn state() -> InterfaceState {
    InterfaceState {
        grid_snap: true,
        osnap: true,
        snap_to_meshes: false,
        smart_track: false,
        display_modes: vec![DisplayMode::Wireframe; 4],
        active_viewport: 2,
    }
}

#[test]
fn switches_are_explicit_idempotent_and_toggle_in_both_directions() {
    for (name, read, initial) in [
        (
            "SetSnap",
            (|s: &InterfaceState| s.grid_snap) as fn(&InterfaceState) -> bool,
            true,
        ),
        (
            "DisableOsnap",
            (|s: &InterfaceState| s.osnap) as fn(&InterfaceState) -> bool,
            true,
        ),
        (
            "SmartTrack",
            (|s: &InterfaceState| s.smart_track) as fn(&InterfaceState) -> bool,
            false,
        ),
    ] {
        let mut s = state();
        assert_eq!(read(&s), initial);
        for (option, expected) in [
            ("On", true),
            ("On", true),
            ("Off", false),
            ("Off", false),
            ("Toggle", true),
            ("Toggle", false),
        ] {
            let option = match (name, option) {
                ("DisableOsnap", "On") => "Enable",
                ("DisableOsnap", "Off") => "Disable",
                _ => option,
            };
            s.apply(parse(&format!("'_-{name} _{option}")).unwrap().unwrap())
                .unwrap();
            assert_eq!(read(&s), expected, "{name} {option}");
        }
    }
    let mut s = state();
    for expected in [false, true] {
        s.apply(parse("_sNaP").unwrap().unwrap()).unwrap();
        assert_eq!(s.grid_snap, expected);
    }
}

#[test]
fn display_options_support_active_all_and_scripted_spelling() {
    let mut s = state();
    for command in [
        "SetDisplayMode Shaded",
        "-_SetDisplayMode _Mode=_Shaded",
        "'SetDisplayMode _Viewport _Active _Mode _Shaded",
    ] {
        s.apply(parse(command).unwrap().unwrap()).unwrap();
        assert_eq!(
            s.display_modes,
            [
                DisplayMode::Wireframe,
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Wireframe
            ]
        );
    }
    s.apply(
        parse("SetDisplayMode Mode=Ghosted Viewport=All")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(s.display_modes, [DisplayMode::Ghosted; 4]);
    assert!(s.grid_snap && s.osnap && !s.smart_track);
    assert_eq!(s.active_viewport, 2);
}

#[test]
fn malformed_known_commands_are_not_treated_as_modeling_input() {
    for command in [
        "Snap On",
        "SetSnap",
        "SetSnap Yes",
        "SetSnap On Off",
        "SmartTrack",
        "DisableOsnap",
        "DisableOsnap Yes",
        "DisableOsnap On",
        "DisableOsnap Off",
        "SetDisplayMode",
        "SetDisplayMode Mode",
        "SetDisplayMode Viewport=All",
        "SetDisplayMode Rendered",
        "SetDisplayMode Shaded Wireframe",
        "SetView",
        "SetDisplayMode Viewport=All Viewport=Active Shaded",
        "SetDisplayMode Mode=Shaded Wrong=All",
        "SetDisplayMode Viewport=Top Wireframe",
        "SetDisplayMode Mode=Shaded _Enter",
    ] {
        assert!(parse(command).unwrap().is_err(), "{command}");
    }
    for command in ["", "  ", "Point 1,2,3", "Osnap", "Unknown"] {
        assert!(parse(command).is_none(), "{command}");
    }
}

#[test]
fn invalid_viewport_state_cannot_be_partially_modified() {
    for modes in [vec![], vec![DisplayMode::Wireframe; 2]] {
        for command in [
            "Snap",
            "DisableOsnap Disable",
            "SmartTrack On",
            "SetDisplayMode Viewport=All Shaded",
        ] {
            let mut s = state();
            s.display_modes = modes.clone();
            let before = s.clone();
            assert_eq!(
                s.apply(parse(command).unwrap().unwrap()),
                Err(InterfaceError::InvalidViewport)
            );
            assert_eq!(s, before);
        }
    }
}

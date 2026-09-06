use super::*;

fn state() -> InterfaceState {
    InterfaceState {
        grid_snap: true,
        osnap: true,
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
        "SetDisplayMode Viewport=All Viewport=Active Shaded",
        "SetDisplayMode Mode=Shaded Wrong=All",
        "SetDisplayMode Viewport=Top Wireframe",
        "SetDisplayMode Mode=Shaded _Enter",
    ] {
        assert!(parse(command).unwrap().is_err(), "{command}");
    }
    for command in ["", "  ", "Point 1,2,3", "Osnap", "SetView", "Unknown"] {
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

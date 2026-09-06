//! Untimed interface state transitions through the GUI's actual command reducer.
use super::*;
use viboceros_command::interface::{DisplayMode, InterfaceState, parse};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct InterfaceFixture {
    pub grid_snap: bool,
    pub osnap: bool,
    pub smart_track: bool,
    pub active_viewport: usize,
    pub display_modes: [String; 4],
    pub commands: Vec<String>,
}

pub(super) fn run(fixture: &InterfaceFixture) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid interface fixture");
    if fixture.active_viewport >= 4 || fixture.commands.is_empty() || fixture.commands.len() > 128 {
        return Err(invalid());
    }
    let modes = fixture
        .display_modes
        .iter()
        .map(|name| DisplayMode::parse(name).ok_or_else(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    let commands = fixture
        .commands
        .iter()
        .map(|command| {
            if command.len() > 512 {
                return Err(invalid());
            }
            parse(command).ok_or_else(invalid)?.map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut state = InterfaceState {
        grid_snap: fixture.grid_snap,
        osnap: fixture.osnap,
        smart_track: fixture.smart_track,
        active_viewport: fixture.active_viewport,
        display_modes: modes,
    };
    let record = |state: &InterfaceState| {
        json!({
            "grid_snap":state.grid_snap, "osnap":state.osnap, "smart_track":state.smart_track,
            "active_viewport":state.active_viewport,
            "display_modes":state.display_modes.iter().map(|mode| mode.label()).collect::<Vec<_>>()
        })
    };
    let mut states = vec![record(&state)];
    for command in commands {
        state.apply(command).map_err(|_| invalid())?;
        states.push(record(&state));
    }
    Ok((json!({"states":states}), 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_interface_fixtures_cover_initial_switch_states_and_every_viewport() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/interface_commands.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let mut initials = BTreeSet::new();
        let mut viewports = BTreeSet::new();
        for (operation, result) in request.operations.iter().zip(response.results) {
            let Operation::InterfaceCommands { fixture, .. } = operation else {
                panic!("interface fixture")
            };
            initials.insert((fixture.grid_snap, fixture.osnap, fixture.smart_track));
            viewports.insert(fixture.active_viewport);
            assert_eq!(result.elapsed_ns, 0);
            assert_eq!(
                result.value["states"].as_array().unwrap().len(),
                fixture.commands.len() + 1
            );
            assert_eq!(result.value["states"][0]["osnap"], fixture.osnap);
            assert_eq!(
                result.value["states"][0]["active_viewport"],
                fixture.active_viewport
            );
        }
        assert_eq!(initials.len(), 8);
        assert_eq!(viewports.len(), 4);
    }

    #[test]
    fn interface_probes_reject_invalid_state_and_unrestricted_macros() {
        let mut fixture = InterfaceFixture {
            grid_snap: true,
            osnap: true,
            smart_track: false,
            active_viewport: 0,
            display_modes: [
                "Wireframe".into(),
                "Shaded".into(),
                "Ghosted".into(),
                "Wireframe".into(),
            ],
            commands: vec!["Snap".into()],
        };
        for command in [
            "Delete",
            "Snap _Delete",
            "SetDisplayMode Viewport=All",
            "Help",
            "SetSnap On\n_Delete",
        ] {
            fixture.commands = vec![command.into()];
            assert!(run(&fixture).is_err());
        }
        fixture.commands = vec!["Snap".into(); 129];
        assert!(run(&fixture).is_err());
        fixture.commands = vec![];
        assert!(run(&fixture).is_err());
        fixture.commands = vec!["Snap".into()];
        fixture.active_viewport = 4;
        assert!(run(&fixture).is_err());
        fixture.active_viewport = 0;
        fixture.display_modes[2] = "Rendered".into();
        assert!(run(&fixture).is_err());
    }
}

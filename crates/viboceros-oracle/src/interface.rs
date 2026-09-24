//! Untimed interface state transitions through the GUI's actual command reducer.
use super::*;
use viboceros_command::interface::{DisplayMode, InterfaceState, OrthoAngle, parse};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct InterfaceFixture {
    pub grid_snap: bool,
    #[serde(default)]
    pub ortho: Option<bool>,
    #[serde(default)]
    pub ortho_angle_degrees: Option<f64>,
    pub osnap: bool,
    #[serde(default, deserialize_with = "present_mesh_setting")]
    pub snap_to_meshes: Option<bool>,
    pub smart_track: bool,
    pub active_viewport: usize,
    pub display_modes: [String; 4],
    pub commands: Vec<String>,
}

fn present_mesh_setting<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
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
    if fixture.snap_to_meshes.is_none()
        && commands.iter().any(|command| {
            matches!(
                command,
                viboceros_command::interface::InterfaceCommand::SnapToMeshes(_)
            )
        })
    {
        return Err(invalid());
    }
    if fixture.ortho.is_none()
        && commands.iter().any(|command| {
            matches!(
                command,
                viboceros_command::interface::InterfaceCommand::SetOrtho(_)
            )
        })
    {
        return Err(invalid());
    }
    if fixture.ortho_angle_degrees.is_none()
        && commands.iter().any(|command| {
            matches!(
                command,
                viboceros_command::interface::InterfaceCommand::OrthoAngle(_)
            )
        })
    {
        return Err(invalid());
    }
    let mut state = InterfaceState {
        grid_snap: fixture.grid_snap,
        ortho: fixture.ortho.unwrap_or(false),
        ortho_angle: OrthoAngle::try_new(fixture.ortho_angle_degrees.unwrap_or(90.0))
            .ok_or_else(invalid)?,
        osnap: fixture.osnap,
        snap_to_meshes: fixture.snap_to_meshes.unwrap_or(false),
        smart_track: fixture.smart_track,
        active_viewport: fixture.active_viewport,
        display_modes: modes,
    };
    let record = |state: &InterfaceState| {
        let mut value = json!({
            "grid_snap":state.grid_snap, "osnap":state.osnap, "smart_track":state.smart_track,
            "active_viewport":state.active_viewport,
            "display_modes":state.display_modes.iter().map(|mode| mode.label()).collect::<Vec<_>>()
        });
        if fixture.snap_to_meshes.is_some() {
            value["snap_to_meshes"] = json!(state.snap_to_meshes);
        }
        if fixture.ortho.is_some() {
            value["ortho"] = json!(state.ortho);
        }
        if fixture.ortho_angle_degrees.is_some() {
            value["ortho_angle_degrees"] = json!(state.ortho_angle.degrees());
        }
        value
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
    fn ortho_fixture_records_switch_and_angle_transitions() {
        let fixture: InterfaceFixture = serde_json::from_value(json!({
            "grid_snap": false,
            "ortho": false,
            "ortho_angle_degrees": 90.0,
            "osnap": false,
            "smart_track": false,
            "active_viewport": 0,
            "display_modes": ["Wireframe", "Wireframe", "Wireframe", "Wireframe"],
            "commands": ["SetOrtho On", "OrthoAngle 45", "Ortho"]
        }))
        .unwrap();
        let (result, _) = run(&fixture).unwrap();
        assert_eq!(result["states"][0]["ortho"], false);
        assert_eq!(result["states"][1]["ortho"], true);
        assert_eq!(result["states"][2]["ortho_angle_degrees"], 45.0);
        assert_eq!(result["states"][3]["ortho"], false);
    }

    #[test]
    fn mesh_snap_switch_states_match_rhino_without_changing_other_settings() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/mesh_snap_switches.json"
        ))
        .unwrap();
        let observations: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/mesh_snap_switches.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 2);
        for (result, observed) in response
            .results
            .iter()
            .zip(observations["results"].as_array().unwrap())
        {
            assert_eq!(result.id, observed["id"]);
            assert_eq!(result.value, observed["value"]);
        }
    }

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
            snap_to_meshes: None,
            grid_snap: true,
            ortho: None,
            ortho_angle_degrees: None,
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
            "SnapToMeshes Toggle", // An explicit initial mesh setting is required.
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

    #[test]
    fn optional_mesh_setting_requires_an_explicit_boolean_when_present() {
        let base = json!({
            "grid_snap":false, "osnap":true, "smart_track":false, "active_viewport":0,
            "display_modes":["Wireframe", "Wireframe", "Wireframe", "Wireframe"],
            "commands":["Snap"]
        });
        let absent: InterfaceFixture = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(absent.snap_to_meshes, None);
        for value in [Value::Null, json!(0), json!("Enable"), json!([]), json!({})] {
            let mut input = base.clone();
            input["snap_to_meshes"] = value;
            assert!(serde_json::from_value::<InterfaceFixture>(input).is_err());
        }
        for enabled in [false, true] {
            let mut input = base.clone();
            input["snap_to_meshes"] = json!(enabled);
            let fixture: InterfaceFixture = serde_json::from_value(input).unwrap();
            assert_eq!(fixture.snap_to_meshes, Some(enabled));
        }
    }
}

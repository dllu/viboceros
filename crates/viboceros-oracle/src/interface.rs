//! Untimed interface state transitions through the GUI's actual command reducer.
use super::*;
use viboceros_command::interface::{DisplayMode, InterfaceState, OrthoAngle, parse};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct InterfaceFixture {
    #[serde(default, deserialize_with = "present_boolean_setting")]
    pub subd_control_net: Option<bool>,
    pub grid_snap: bool,
    #[serde(default)]
    pub ortho: Option<bool>,
    #[serde(default)]
    pub ortho_snap_to_cplane_z: Option<bool>,
    #[serde(default)]
    pub planar: Option<bool>,
    #[serde(default)]
    pub ortho_angle_degrees: Option<f64>,
    pub osnap: bool,
    #[serde(default, deserialize_with = "present_boolean_setting")]
    pub snap_to_meshes: Option<bool>,
    pub smart_track: bool,
    pub active_viewport: usize,
    pub display_modes: [String; 4],
    pub commands: Vec<String>,
}

fn present_boolean_setting<'de, D: serde::Deserializer<'de>>(
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
    if fixture.ortho_snap_to_cplane_z.is_none()
        && commands.iter().any(|command| {
            matches!(
                command,
                viboceros_command::interface::InterfaceCommand::OrthoSnapToCPlaneZ(_)
            )
        })
    {
        return Err(invalid());
    }
    if fixture.planar.is_none()
        && commands.iter().any(|command| {
            matches!(
                command,
                viboceros_command::interface::InterfaceCommand::SetPlanar(_)
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
    if fixture.subd_control_net.is_none()
        && commands.iter().any(|command| {
            matches!(
                command,
                viboceros_command::interface::InterfaceCommand::SubDDisplayToggle
            )
        })
    {
        return Err(invalid());
    }
    let mut state = InterfaceState {
        subd_control_net: fixture.subd_control_net.unwrap_or(false),
        grid_snap: fixture.grid_snap,
        ortho: fixture.ortho.unwrap_or(false),
        ortho_snap_to_cplane_z: fixture.ortho_snap_to_cplane_z.unwrap_or(false),
        planar: fixture.planar.unwrap_or(false),
        ortho_angle: OrthoAngle::try_new(fixture.ortho_angle_degrees.unwrap_or(90.0))
            .ok_or_else(invalid)?,
        osnap: fixture.osnap,
        snap_to_meshes: fixture.snap_to_meshes.unwrap_or(false),
        smart_track: fixture.smart_track,
        active_viewport: fixture.active_viewport,
        maximized_viewport: None,
        four_view_projection: Default::default(),
        display_modes: modes,
    };
    let record = |state: &InterfaceState| {
        let mut value = json!({
            "grid_snap":state.grid_snap, "osnap":state.osnap, "smart_track":state.smart_track,
            "active_viewport":state.active_viewport,
            "display_modes":state.display_modes.iter().map(|mode| mode.label()).collect::<Vec<_>>()
        });
        if fixture.subd_control_net.is_some() {
            value["subd_control_net"] = json!(state.subd_control_net);
        }
        if fixture.snap_to_meshes.is_some() {
            value["snap_to_meshes"] = json!(state.snap_to_meshes);
        }
        if fixture.ortho.is_some() {
            value["ortho"] = json!(state.ortho);
        }
        if fixture.ortho_snap_to_cplane_z.is_some() {
            value["ortho_snap_to_cplane_z"] = json!(state.ortho_snap_to_cplane_z);
        }
        if fixture.planar.is_some() {
            value["planar"] = json!(state.planar);
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
mod tests;

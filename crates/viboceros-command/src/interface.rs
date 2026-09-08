//! Document-independent interface commands shared by the GUI and headless probes.
//!
//! Parsing is side-effect free. Applying an action never touches model geometry,
//! selection, undo history, or a caller's unfinished modeling command.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayMode {
    Wireframe,
    Shaded,
    Ghosted,
}

impl DisplayMode {
    pub const ALL: [Self; 3] = [Self::Wireframe, Self::Shaded, Self::Ghosted];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Wireframe => "Wireframe",
            Self::Shaded => "Shaded",
            Self::Ghosted => "Ghosted",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| keyword(input, mode.label()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwitchAction {
    On,
    Off,
    Toggle,
}

impl SwitchAction {
    const fn apply(self, value: bool) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::Toggle => !value,
        }
    }

    fn parse(input: &str) -> Option<Self> {
        if keyword(input, "On") {
            Some(Self::On)
        } else if keyword(input, "Off") {
            Some(Self::Off)
        } else if keyword(input, "Toggle") {
            Some(Self::Toggle)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewportTarget {
    Active,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceCommand {
    ZoomExtents,
    ZoomSelected,
    SetSnap(SwitchAction),
    SetOsnap(SwitchAction),
    SmartTrack(SwitchAction),
    SetDisplayMode {
        viewport: ViewportTarget,
        mode: DisplayMode,
    },
}

pub const COMMAND_NAMES: [&str; 8] = [
    "Zoom",
    "ZE",
    "ZS",
    "DisableOsnap",
    "SetDisplayMode",
    "SetSnap",
    "SmartTrack",
    "Snap",
];

pub const HELP: &str = "Interface: Zoom Extents (ZE); Zoom Selected (ZS); Snap; SetSnap On|Off|Toggle; DisableOsnap Enable|Disable|Toggle; SmartTrack On|Off|Toggle; SetDisplayMode [Viewport=Active|All] Mode=Wireframe|Shaded|Ghosted. These commands preserve unfinished modeling commands. Shortcuts: F9 grid snap, F4 object snaps, Ctrl/Cmd+Alt+W/S/G display mode.";

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InterfaceError {
    #[error("Usage: {0}")]
    Usage(&'static str),
    #[error("interface state requires a valid active viewport")]
    InvalidViewport,
}

fn keyword(input: &str, expected: &str) -> bool {
    input.trim_start_matches('_').eq_ignore_ascii_case(expected)
}

/// `None` means this is not an interface command; a recognized but malformed
/// command returns `Some(Err(..))` so hosts can retain their current prompt.
pub fn parse(input: &str) -> Option<Result<InterfaceCommand, InterfaceError>> {
    let mut tokens = input.split_whitespace();
    let name = tokens.next()?.trim_start_matches(['\'', '_', '-']);
    let args = tokens.collect::<Vec<_>>();
    let switch = |usage| match args.as_slice() {
        [value] => SwitchAction::parse(value).ok_or(InterfaceError::Usage(usage)),
        _ => Err(InterfaceError::Usage(usage)),
    };
    Some(
        if name.eq_ignore_ascii_case("Zoom")
            || name.eq_ignore_ascii_case("ZE")
            || name.eq_ignore_ascii_case("ZS")
        {
            match args.as_slice() {
                [] if name.eq_ignore_ascii_case("ZE") => Ok(InterfaceCommand::ZoomExtents),
                [] if name.eq_ignore_ascii_case("ZS") => Ok(InterfaceCommand::ZoomSelected),
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Extents") => {
                    Ok(InterfaceCommand::ZoomExtents)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Selected") => {
                    Ok(InterfaceCommand::ZoomSelected)
                }
                _ => Err(InterfaceError::Usage("Zoom Extents|Selected | ZE | ZS")),
            }
        } else if name.eq_ignore_ascii_case("Snap") {
            if args.is_empty() {
                Ok(InterfaceCommand::SetSnap(SwitchAction::Toggle))
            } else {
                Err(InterfaceError::Usage("Snap"))
            }
        } else if name.eq_ignore_ascii_case("SetSnap") {
            switch("SetSnap On|Off|Toggle").map(InterfaceCommand::SetSnap)
        } else if name.eq_ignore_ascii_case("DisableOsnap") {
            match args.as_slice() {
                [value] if keyword(value, "Enable") => {
                    Ok(InterfaceCommand::SetOsnap(SwitchAction::On))
                }
                [value] if keyword(value, "Disable") => {
                    Ok(InterfaceCommand::SetOsnap(SwitchAction::Off))
                }
                [value] if keyword(value, "Toggle") => {
                    Ok(InterfaceCommand::SetOsnap(SwitchAction::Toggle))
                }
                _ => Err(InterfaceError::Usage("DisableOsnap Enable|Disable|Toggle")),
            }
        } else if name.eq_ignore_ascii_case("SmartTrack") {
            switch("SmartTrack On|Off|Toggle").map(InterfaceCommand::SmartTrack)
        } else if name.eq_ignore_ascii_case("SetDisplayMode") {
            parse_display_mode(&args)
        } else {
            return None;
        },
    )
}

fn parse_display_mode(args: &[&str]) -> Result<InterfaceCommand, InterfaceError> {
    let usage =
        InterfaceError::Usage("SetDisplayMode [Viewport=Active|All] Mode=Wireframe|Shaded|Ghosted");
    let mut viewport = None;
    let mut mode = None;
    let mut tokens = args.iter().copied();
    while let Some(token) = tokens.next() {
        let (name, value) = if let Some(pair) = token.split_once('=') {
            pair
        } else if keyword(token, "Viewport") || keyword(token, "Mode") {
            (token, tokens.next().ok_or_else(|| usage.clone())?)
        } else {
            ("Mode", token)
        };
        if keyword(name, "Viewport") && viewport.is_none() {
            viewport = Some(if keyword(value, "Active") {
                ViewportTarget::Active
            } else if keyword(value, "All") {
                ViewportTarget::All
            } else {
                return Err(usage);
            });
        } else if keyword(name, "Mode") && mode.is_none() {
            mode = Some(DisplayMode::parse(value).ok_or_else(|| usage.clone())?);
        } else {
            return Err(usage);
        }
    }
    Ok(InterfaceCommand::SetDisplayMode {
        viewport: viewport.unwrap_or(ViewportTarget::Active),
        mode: mode.ok_or(usage)?,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceState {
    pub grid_snap: bool,
    pub osnap: bool,
    pub smart_track: bool,
    pub display_modes: Vec<DisplayMode>,
    pub active_viewport: usize,
}

impl InterfaceState {
    /// Validate before mutation, including actions which do not use a viewport.
    pub fn apply(&mut self, command: InterfaceCommand) -> Result<String, InterfaceError> {
        if self.active_viewport >= self.display_modes.len() {
            return Err(InterfaceError::InvalidViewport);
        }
        let on_off = |value| if value { "On" } else { "Off" };
        Ok(match command {
            InterfaceCommand::ZoomExtents => "Zoom extents requested (active viewport)".into(),
            InterfaceCommand::ZoomSelected => "Zoom selected requested (active viewport)".into(),
            InterfaceCommand::SetSnap(action) => {
                self.grid_snap = action.apply(self.grid_snap);
                format!("Grid snap: {}", on_off(self.grid_snap))
            }
            InterfaceCommand::SetOsnap(action) => {
                self.osnap = action.apply(self.osnap);
                format!("Object snaps: {}", on_off(self.osnap))
            }
            InterfaceCommand::SmartTrack(action) => {
                self.smart_track = action.apply(self.smart_track);
                format!("SmartTrack: {}", on_off(self.smart_track))
            }
            InterfaceCommand::SetDisplayMode { viewport, mode } => {
                let target = match viewport {
                    ViewportTarget::Active => {
                        self.display_modes[self.active_viewport] = mode;
                        "active viewport"
                    }
                    ViewportTarget::All => {
                        self.display_modes.fill(mode);
                        "all viewports"
                    }
                };
                format!("Display mode: {} ({target})", mode.label())
            }
        })
    }
}

#[cfg(test)]
mod tests;

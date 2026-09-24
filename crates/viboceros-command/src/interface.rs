//! Document-independent interface commands shared by the GUI and headless probes.
//!
//! Parsing is side-effect free. Applying an action never touches model geometry,
//! selection, undo history, or a caller's unfinished modeling command.

use crate::construction_plane::WorldPlane;

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
pub enum WorldView {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
    Perspective,
}

impl WorldView {
    pub const ALL: [Self; 7] = [
        Self::Top,
        Self::Bottom,
        Self::Front,
        Self::Back,
        Self::Right,
        Self::Left,
        Self::Perspective,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Front => "Front",
            Self::Back => "Back",
            Self::Right => "Right",
            Self::Left => "Left",
            Self::Perspective => "Perspective",
        }
    }

    fn parse(input: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|view| keyword(input, view.label()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZoomFactor(u64);

impl ZoomFactor {
    /// Store only finite, strictly positive factors; bit equality is then
    /// numeric equality (NaN and both representations of zero are excluded).
    pub fn try_new(value: f64) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value.to_bits()))
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZoomScale(u64);

impl ZoomScale {
    pub fn try_new(value: f64) -> Option<Self> {
        (value.is_finite() && value > 0.0 && value.recip().is_finite())
            .then_some(Self(value.to_bits()))
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RectSelectionMode {
    Automatic,
    Window,
    Crossing,
    InvertWindow,
    InvertCrossing,
}

impl RectSelectionMode {
    pub fn parse(value: &str) -> Option<Self> {
        if keyword(value, "Window") {
            Some(Self::Window)
        } else if keyword(value, "Crossing") {
            Some(Self::Crossing)
        } else if keyword(value, "InvertWindow") {
            Some(Self::InvertWindow)
        } else if keyword(value, "InvertCrossing") {
            Some(Self::InvertCrossing)
        } else {
            None
        }
    }

    pub fn crossing(self, automatic_crossing: bool) -> bool {
        match self {
            Self::Automatic => automatic_crossing,
            Self::Window | Self::InvertWindow => false,
            Self::Crossing | Self::InvertCrossing => true,
        }
    }

    pub fn inverted(self) -> bool {
        matches!(self, Self::InvertWindow | Self::InvertCrossing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceCommand {
    SetZoomScale(ZoomScale),
    SetZoomExtentsBorder {
        parallel: Option<ZoomScale>,
        perspective: Option<ZoomScale>,
    },
    ZoomFactor(ZoomFactor),
    ZoomFactorPrompt,
    ZoomIn,
    ZoomOut,
    ZoomWindow,
    ZoomTarget,
    ZoomExtents,
    ZoomSelected,
    ZoomAllExtents,
    ZoomAllSelected,
    ZoomEnds,
    ZoomEndsCurrent,
    ZoomEndsNext,
    ZoomEndsPrevious,
    ShowEnds,
    ShowEndsOff,
    SelWindow,
    SelCrossing,
    SelRectangular(RectSelectionMode),
    SelCircular(RectSelectionMode),
    SelBoundary(RectSelectionMode),
    SelFence,
    SelFenceCurve,
    UndoView,
    RedoView,
    NextViewport,
    PrevViewport,
    NextOrthoViewport,
    NextPerspectiveViewport,
    Plan,
    SetViewWorld(WorldView),
    SetViewCPlane(WorldPlane),
    SetSnap(SwitchAction),
    SetOsnap(SwitchAction),
    SnapToMeshes(SwitchAction),
    SmartTrack(SwitchAction),
    SetDisplayMode {
        viewport: ViewportTarget,
        mode: DisplayMode,
    },
}

pub const COMMAND_NAMES: [&str; 33] = [
    "Options",
    "SetZoomExtentsBorder",
    "SnapToMeshes",
    "Zoom",
    "ZE",
    "ZS",
    "ZEA",
    "ZSA",
    "ZT",
    "ZoomEnds",
    "ShowEnds",
    "ShowEndsOff",
    "DisableOsnap",
    "SetDisplayMode",
    "SetSnap",
    "SmartTrack",
    "Snap",
    "UndoView",
    "RedoView",
    "NextViewport",
    "PrevViewport",
    "NextOrthoViewport",
    "NextPerspectiveViewport",
    "SetView",
    "Plan",
    "SelWindow",
    "SelCrossing",
    "SelRectangular",
    "SelCircular",
    "SelBoundary",
    "SelFence",
    "W",
    "C",
];

pub const HELP: &str = "Interface: Zoom [Window]|Target|[All] Extents|Selected (ZE, ZS, ZEA, ZSA, ZT); Zoom In|Out|Factor [positive number]; ZoomEnds [All|Current|Next|Previous]; ShowEnds; ShowEndsOff; SelWindow (W); SelCrossing (C); SelRectangular [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]; SelCircular [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]; SelBoundary [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]; SelFence [Curve]; UndoView; RedoView; NextViewport; PrevViewport; NextOrthoViewport; NextPerspectiveViewport; SetView World Top|Bottom|Front|Back|Right|Left|Perspective; SetView CPlane Top|Bottom|Front|Back|Right|Left; Plan; Options View Zoom ScaleFactor=<positive number>; SetZoomExtentsBorder [ParallelView=<positive number>] [PerspectiveView=<positive number>]; Snap; SetSnap On|Off|Toggle; DisableOsnap Enable|Disable|Toggle; SnapToMeshes Enable|Disable|Toggle; SmartTrack On|Off|Toggle; SetDisplayMode [Viewport=Active|All] Mode=Wireframe|Shaded|Ghosted. These commands preserve unfinished modeling commands. Shortcuts: Ctrl/Cmd+Tab next viewport, Ctrl/Cmd+Shift+Tab previous viewport, Home/End view history, Ctrl/Cmd+W zoom window, Ctrl/Cmd+Shift+E active extents, Ctrl/Cmd+Alt+E all extents, F9 grid snap, F4 object snaps, Ctrl/Cmd+Alt+W/S/G display mode.";

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

fn parse_region_mode(
    args: &[&str],
    default: RectSelectionMode,
    usage: &'static str,
) -> Result<RectSelectionMode, InterfaceError> {
    match args {
        [] => Ok(default),
        [option] => {
            let value = option
                .split_once('=')
                .filter(|(name, _)| keyword(name, "SelectionMode"))
                .map_or(*option, |(_, value)| value);
            RectSelectionMode::parse(value).ok_or(InterfaceError::Usage(usage))
        }
        _ => Err(InterfaceError::Usage(usage)),
    }
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
            || name.eq_ignore_ascii_case("ZEA")
            || name.eq_ignore_ascii_case("ZSA")
            || name.eq_ignore_ascii_case("ZT")
        {
            match args.as_slice() {
                [] if name.eq_ignore_ascii_case("Zoom") => Ok(InterfaceCommand::ZoomWindow),
                [] if name.eq_ignore_ascii_case("ZE") => Ok(InterfaceCommand::ZoomExtents),
                [] if name.eq_ignore_ascii_case("ZS") => Ok(InterfaceCommand::ZoomSelected),
                [] if name.eq_ignore_ascii_case("ZEA") => Ok(InterfaceCommand::ZoomAllExtents),
                [] if name.eq_ignore_ascii_case("ZSA") => Ok(InterfaceCommand::ZoomAllSelected),
                [] if name.eq_ignore_ascii_case("ZT") => Ok(InterfaceCommand::ZoomTarget),
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Extents") => {
                    Ok(InterfaceCommand::ZoomExtents)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Selected") => {
                    Ok(InterfaceCommand::ZoomSelected)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "In") => {
                    Ok(InterfaceCommand::ZoomIn)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Out") => {
                    Ok(InterfaceCommand::ZoomOut)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Window") => {
                    Ok(InterfaceCommand::ZoomWindow)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Target") => {
                    Ok(InterfaceCommand::ZoomTarget)
                }
                [option] if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Factor") => {
                    Ok(InterfaceCommand::ZoomFactorPrompt)
                }
                [option, value]
                    if name.eq_ignore_ascii_case("Zoom") && keyword(option, "Factor") =>
                {
                    value
                        .parse::<f64>()
                        .ok()
                        .and_then(ZoomFactor::try_new)
                        .map(InterfaceCommand::ZoomFactor)
                        .ok_or(InterfaceError::Usage(
                            "Zoom Factor <finite positive number>",
                        ))
                }
                [all, option]
                    if name.eq_ignore_ascii_case("Zoom")
                        && keyword(all, "All")
                        && keyword(option, "Extents") =>
                {
                    Ok(InterfaceCommand::ZoomAllExtents)
                }
                [all, option]
                    if name.eq_ignore_ascii_case("Zoom")
                        && keyword(all, "All")
                        && keyword(option, "Selected") =>
                {
                    Ok(InterfaceCommand::ZoomAllSelected)
                }
                _ => Err(InterfaceError::Usage(
                    "Zoom [Window]|Target|[All] Extents|Selected | Zoom In|Out|Factor [positive number] | ZE | ZS | ZEA | ZSA | ZT",
                )),
            }
        } else if name.eq_ignore_ascii_case("ZoomEnds") {
            match args.as_slice() {
                [] => Ok(InterfaceCommand::ZoomEnds),
                [option] if keyword(option, "All") => Ok(InterfaceCommand::ZoomEnds),
                [option] if keyword(option, "Current") => Ok(InterfaceCommand::ZoomEndsCurrent),
                [option] if keyword(option, "Next") => Ok(InterfaceCommand::ZoomEndsNext),
                [option] if keyword(option, "Previous") => Ok(InterfaceCommand::ZoomEndsPrevious),
                _ => Err(InterfaceError::Usage(
                    "ZoomEnds [All|Current|Next|Previous]",
                )),
            }
        } else if name.eq_ignore_ascii_case("ShowEnds") || name.eq_ignore_ascii_case("ShowEndsOff")
        {
            if args.is_empty() {
                Ok(if name.eq_ignore_ascii_case("ShowEnds") {
                    InterfaceCommand::ShowEnds
                } else {
                    InterfaceCommand::ShowEndsOff
                })
            } else {
                Err(InterfaceError::Usage("ShowEnds | ShowEndsOff"))
            }
        } else if name.eq_ignore_ascii_case("SelWindow") || name.eq_ignore_ascii_case("W") {
            if args.is_empty() {
                Ok(InterfaceCommand::SelWindow)
            } else {
                Err(InterfaceError::Usage("SelWindow"))
            }
        } else if name.eq_ignore_ascii_case("SelCrossing") || name.eq_ignore_ascii_case("C") {
            if args.is_empty() {
                Ok(InterfaceCommand::SelCrossing)
            } else {
                Err(InterfaceError::Usage("SelCrossing"))
            }
        } else if name.eq_ignore_ascii_case("SelRectangular") {
            const USAGE: &str =
                "SelRectangular [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
            parse_region_mode(&args, RectSelectionMode::Automatic, USAGE)
                .map(InterfaceCommand::SelRectangular)
        } else if name.eq_ignore_ascii_case("SelCircular") {
            const USAGE: &str =
                "SelCircular [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
            parse_region_mode(&args, RectSelectionMode::Crossing, USAGE)
                .map(InterfaceCommand::SelCircular)
        } else if name.eq_ignore_ascii_case("SelBoundary") {
            const USAGE: &str =
                "SelBoundary [SelectionMode=Window|Crossing|InvertWindow|InvertCrossing]";
            parse_region_mode(&args, RectSelectionMode::Crossing, USAGE)
                .map(InterfaceCommand::SelBoundary)
        } else if name.eq_ignore_ascii_case("SelFence") {
            match args.as_slice() {
                [] => Ok(InterfaceCommand::SelFence),
                [option] if keyword(option, "Curve") => Ok(InterfaceCommand::SelFenceCurve),
                _ => Err(InterfaceError::Usage("SelFence [Curve]")),
            }
        } else if name.eq_ignore_ascii_case("Plan") {
            if args.is_empty() {
                Ok(InterfaceCommand::Plan)
            } else {
                Err(InterfaceError::Usage("Plan"))
            }
        } else if name.eq_ignore_ascii_case("SetView") {
            match args.as_slice() {
                [world, view] if keyword(world, "World") => WorldView::parse(view)
                    .map(InterfaceCommand::SetViewWorld)
                    .ok_or(InterfaceError::Usage(
                        "SetView World Top|Bottom|Front|Back|Right|Left|Perspective",
                    )),
                [cplane, view] if keyword(cplane, "CPlane") => WorldPlane::ALL
                    .into_iter()
                    .find(|direction| keyword(view, direction.label()))
                    .map(InterfaceCommand::SetViewCPlane)
                    .ok_or(InterfaceError::Usage(
                        "SetView CPlane Top|Bottom|Front|Back|Right|Left",
                    )),
                _ => Err(InterfaceError::Usage(
                    "SetView World Top|Bottom|Front|Back|Right|Left|Perspective | SetView CPlane Top|Bottom|Front|Back|Right|Left",
                )),
            }
        } else if name.eq_ignore_ascii_case("UndoView") {
            if args.is_empty() {
                Ok(InterfaceCommand::UndoView)
            } else {
                Err(InterfaceError::Usage("UndoView"))
            }
        } else if name.eq_ignore_ascii_case("RedoView") {
            if args.is_empty() {
                Ok(InterfaceCommand::RedoView)
            } else {
                Err(InterfaceError::Usage("RedoView"))
            }
        } else if let Some(command) = [
            ("NextViewport", InterfaceCommand::NextViewport),
            ("PrevViewport", InterfaceCommand::PrevViewport),
            ("NextOrthoViewport", InterfaceCommand::NextOrthoViewport),
            (
                "NextPerspectiveViewport",
                InterfaceCommand::NextPerspectiveViewport,
            ),
        ]
        .into_iter()
        .find_map(|(name_option, command)| {
            name.eq_ignore_ascii_case(name_option).then_some(command)
        }) {
            if args.is_empty() {
                Ok(command)
            } else {
                Err(InterfaceError::Usage(match command {
                    InterfaceCommand::NextViewport => "NextViewport",
                    InterfaceCommand::PrevViewport => "PrevViewport",
                    InterfaceCommand::NextOrthoViewport => "NextOrthoViewport",
                    _ => "NextPerspectiveViewport",
                }))
            }
        } else if name.eq_ignore_ascii_case("Options") {
            match args.as_slice() {
                [view, zoom, scale] if keyword(view, "View") && keyword(zoom, "Zoom") => scale
                    .split_once('=')
                    .filter(|(key, _)| keyword(key, "ScaleFactor"))
                    .and_then(|(_, value)| value.parse::<f64>().ok())
                    .and_then(ZoomScale::try_new)
                    .map(InterfaceCommand::SetZoomScale)
                    .ok_or(InterfaceError::Usage(
                        "Options View Zoom ScaleFactor=<finite positive number>",
                    )),
                _ => Err(InterfaceError::Usage(
                    "Options View Zoom ScaleFactor=<finite positive number>",
                )),
            }
        } else if name.eq_ignore_ascii_case("SetZoomExtentsBorder") {
            parse_zoom_extents_border(&args)
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
        } else if name.eq_ignore_ascii_case("SnapToMeshes") {
            match args.as_slice() {
                [value] if keyword(value, "Enable") => {
                    Ok(InterfaceCommand::SnapToMeshes(SwitchAction::On))
                }
                [value] if keyword(value, "Disable") => {
                    Ok(InterfaceCommand::SnapToMeshes(SwitchAction::Off))
                }
                [value] if keyword(value, "Toggle") => {
                    Ok(InterfaceCommand::SnapToMeshes(SwitchAction::Toggle))
                }
                _ => Err(InterfaceError::Usage("SnapToMeshes Enable|Disable|Toggle")),
            }
        } else if name.eq_ignore_ascii_case("SetDisplayMode") {
            parse_display_mode(&args)
        } else {
            return None;
        },
    )
}

fn parse_zoom_extents_border(args: &[&str]) -> Result<InterfaceCommand, InterfaceError> {
    let usage = InterfaceError::Usage(
        "SetZoomExtentsBorder [ParallelView=<finite positive number>] [PerspectiveView=<finite positive number>]",
    );
    let mut parallel = None;
    let mut perspective = None;
    for token in args {
        let (name, value) = token.split_once('=').ok_or_else(|| usage.clone())?;
        let value = value
            .parse::<f64>()
            .ok()
            .and_then(ZoomScale::try_new)
            .ok_or_else(|| usage.clone())?;
        if keyword(name, "ParallelView") && parallel.is_none() {
            parallel = Some(value);
        } else if keyword(name, "PerspectiveView") && perspective.is_none() {
            perspective = Some(value);
        } else {
            return Err(usage);
        }
    }
    Ok(InterfaceCommand::SetZoomExtentsBorder {
        parallel,
        perspective,
    })
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
    pub snap_to_meshes: bool,
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
            InterfaceCommand::SetZoomScale(scale) => {
                format!("View zoom scale factor {} requested", scale.value())
            }
            InterfaceCommand::SetZoomExtentsBorder {
                parallel,
                perspective,
            } => format!(
                "Zoom extents border requested (parallel={}, perspective={})",
                parallel.map_or_else(|| "unchanged".into(), |value| value.value().to_string()),
                perspective.map_or_else(|| "unchanged".into(), |value| value.value().to_string())
            ),
            InterfaceCommand::ZoomFactor(factor) => {
                format!("Zoom factor {} requested (active viewport)", factor.value())
            }
            InterfaceCommand::ZoomFactorPrompt => "Zoom factor input requested".into(),
            InterfaceCommand::ZoomIn => "Zoom in requested (active viewport)".into(),
            InterfaceCommand::ZoomOut => "Zoom out requested (active viewport)".into(),
            InterfaceCommand::ZoomWindow => "Zoom window requested".into(),
            InterfaceCommand::ZoomTarget => "Zoom target requested".into(),
            InterfaceCommand::ZoomExtents => "Zoom extents requested (active viewport)".into(),
            InterfaceCommand::ZoomSelected => "Zoom selected requested (active viewport)".into(),
            InterfaceCommand::ZoomAllExtents => "Zoom extents requested (all viewports)".into(),
            InterfaceCommand::ZoomAllSelected => "Zoom selected requested (all viewports)".into(),
            InterfaceCommand::ZoomEnds => "Zoom curve ends requested (active viewport)".into(),
            InterfaceCommand::ZoomEndsCurrent => "Zoom current curve end requested".into(),
            InterfaceCommand::ZoomEndsNext => "Zoom next curve end requested".into(),
            InterfaceCommand::ZoomEndsPrevious => "Zoom previous curve end requested".into(),
            InterfaceCommand::ShowEnds => "End Analysis requested".into(),
            InterfaceCommand::ShowEndsOff => "End Analysis closed".into(),
            InterfaceCommand::SelWindow => "Window selection requested".into(),
            InterfaceCommand::SelCrossing => "Crossing selection requested".into(),
            InterfaceCommand::SelRectangular(mode) => {
                format!("Rectangular {mode:?} selection requested")
            }
            InterfaceCommand::SelCircular(mode) => format!("Circular {mode:?} selection requested"),
            InterfaceCommand::SelBoundary(mode) => format!("Boundary {mode:?} selection requested"),
            InterfaceCommand::SelFence => "Fence selection requested".into(),
            InterfaceCommand::SelFenceCurve => "Curve fence selection requested".into(),
            InterfaceCommand::UndoView => "Undo view requested (active viewport)".into(),
            InterfaceCommand::RedoView => "Redo view requested (active viewport)".into(),
            InterfaceCommand::NextViewport => "Next viewport requested".into(),
            InterfaceCommand::PrevViewport => "Previous viewport requested".into(),
            InterfaceCommand::NextOrthoViewport => "Next orthographic viewport requested".into(),
            InterfaceCommand::NextPerspectiveViewport => {
                "Next perspective viewport requested".into()
            }
            InterfaceCommand::Plan => "Plan view requested (active viewport)".into(),
            InterfaceCommand::SetViewWorld(view) => format!(
                "Set world {} view requested (active viewport)",
                view.label()
            ),
            InterfaceCommand::SetViewCPlane(direction) => format!(
                "Set CPlane {} view requested (active viewport)",
                direction.label()
            ),
            InterfaceCommand::SetSnap(action) => {
                self.grid_snap = action.apply(self.grid_snap);
                format!("Grid snap: {}", on_off(self.grid_snap))
            }
            InterfaceCommand::SetOsnap(action) => {
                self.osnap = action.apply(self.osnap);
                format!("Object snaps: {}", on_off(self.osnap))
            }
            InterfaceCommand::SnapToMeshes(action) => {
                self.snap_to_meshes = action.apply(self.snap_to_meshes);
                format!("Mesh wire snaps: {}", on_off(self.snap_to_meshes))
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

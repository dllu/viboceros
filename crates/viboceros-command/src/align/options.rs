use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlignmentMode {
    Left,
    Right,
    Top,
    Bottom,
    HorizCenter,
    VertCenter,
    Concentric,
}

impl AlignmentMode {
    pub const LABELS: &'static [&'static str] = &[
        "Left",
        "Right",
        "Top",
        "Bottom",
        "HorizCenter",
        "VertCenter",
        "Concentric",
    ];
    const VALUES: [Self; 7] = [
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
        Self::HorizCenter,
        Self::VertCenter,
        Self::Concentric,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::HorizCenter => "HorizCenter",
            Self::VertCenter => "VertCenter",
            Self::Concentric => "Concentric",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        Self::VALUES
            .into_iter()
            .find(|mode| option_name_eq(value, mode.label()))
    }
}

/// Shared typed/interactive state. Only the coordinate choice is remembered.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AlignmentOptions {
    pub mode: Option<AlignmentMode>,
    pub world: bool,
    pub target: Option<Point3>,
    pub automatic: bool,
}

impl AlignmentOptions {
    pub fn command_line(self) -> String {
        format!(
            "Align AlignTo={}{}",
            if self.world { "World" } else { "CPlane" },
            self.mode
                .map_or_else(String::new, |mode| format!(" {}", mode.label()))
        )
    }
    /// Update a copy, rejecting duplicate or contradictory entries atomically.
    pub fn parse(mut self, arguments: &[&str]) -> Result<Self, CommandError> {
        let mut remaining = arguments;
        let (mut mode_seen, mut frame_seen, mut target_seen) = (false, false, false);
        while let Some(token) = remaining.first() {
            if let Some(mode) = AlignmentMode::parse(token) {
                if mode_seen {
                    return Err(CommandError::Usage(USAGE));
                }
                mode_seen = true;
                self.mode = Some(mode);
                remaining = &remaining[1..];
            } else if option_name_eq(token, "Auto") {
                if target_seen {
                    return Err(CommandError::Usage(USAGE));
                }
                target_seen = true;
                self.automatic = true;
                self.target = None;
                remaining = &remaining[1..];
            } else if token.contains('=')
                || option_name_eq(token, "AlignTo")
                || option_name_eq(token, "Mode")
            {
                let (name, value, used) = if let Some((name, value)) = token.split_once('=') {
                    (name, value, 1)
                } else {
                    (
                        *token,
                        *remaining.get(1).ok_or(CommandError::Usage(USAGE))?,
                        2,
                    )
                };
                if option_name_eq(name, "AlignTo") && !frame_seen {
                    self.world = if option_name_eq(value, "World") {
                        true
                    } else if option_name_eq(value, "CPlane") {
                        false
                    } else {
                        return Err(CommandError::Usage(USAGE));
                    };
                    frame_seen = true;
                } else if option_name_eq(name, "Mode") && !mode_seen {
                    self.mode =
                        Some(AlignmentMode::parse(value).ok_or(CommandError::Usage(USAGE))?);
                    mode_seen = true;
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
                remaining = &remaining[used..];
            } else {
                if target_seen {
                    return Err(CommandError::Usage(USAGE));
                }
                let (point, used) = parse_point(remaining)?;
                self.target = Some(point);
                self.automatic = false;
                target_seen = true;
                remaining = &remaining[used..];
            }
        }
        Ok(self)
    }
}

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
    ToLine,
    ToPlane,
    ToFitPlane,
    ToCurve,
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
        "ToLine",
        "ToPlane",
        "ToFitPlane",
        "ToCurve",
    ];
    const VALUES: [Self; 11] = [
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
        Self::HorizCenter,
        Self::VertCenter,
        Self::Concentric,
        Self::ToLine,
        Self::ToPlane,
        Self::ToFitPlane,
        Self::ToCurve,
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
            Self::ToLine => "ToLine",
            Self::ToPlane => "ToPlane",
            Self::ToFitPlane => "ToFitPlane",
            Self::ToCurve => "ToCurve",
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
    pub references: [Option<Point3>; 3],
    pub three_point: bool,
    pub curve: Option<viboceros_document::ObjectId>,
}

impl AlignmentOptions {
    pub fn command_line(self) -> String {
        let mut line = format!(
            "Align AlignTo={}{}",
            if self.world { "World" } else { "CPlane" },
            self.mode
                .map_or_else(String::new, |mode| format!(" {}", mode.label()))
        );
        if self.three_point {
            line.push_str(" 3Point");
        }
        if let Some(id) = self.curve {
            line.push_str(&format!(" CurveId={id}"));
        }
        for p in self.target.iter().chain(self.references.iter().flatten()) {
            line.push_str(&format!(" {},{},{}", p.x(), p.y(), p.z()));
        }
        if self.automatic {
            line.push_str(" Auto");
        }
        line
    }
    pub const fn reference_count(self) -> usize {
        match self.mode {
            Some(AlignmentMode::ToLine) => 2,
            Some(AlignmentMode::ToPlane) => {
                if self.three_point {
                    3
                } else {
                    2
                }
            }
            _ => 0,
        }
    }
    pub fn ready(self) -> bool {
        if self.mode == Some(AlignmentMode::ToCurve) {
            self.curve.is_some()
        } else if self.mode == Some(AlignmentMode::ToFitPlane) {
            true
        } else if self.reference_count() == 0 {
            self.target.is_some() || self.automatic
        } else {
            self.references[..self.reference_count()]
                .iter()
                .all(Option::is_some)
        }
    }
    pub fn with_point(mut self, point: Point3) -> Result<Self, CommandError> {
        if matches!(
            self.mode,
            Some(AlignmentMode::ToFitPlane | AlignmentMode::ToCurve)
        ) {
            return Err(CommandError::Usage(USAGE));
        }
        if self.reference_count() == 0 {
            if self.target.is_some() || self.automatic {
                return Err(CommandError::Usage(USAGE));
            }
            self.target = Some(point);
        } else {
            let count = self.reference_count();
            *self.references[..count]
                .iter_mut()
                .find(|p| p.is_none())
                .ok_or(CommandError::Usage(USAGE))? = Some(point);
        }
        Ok(self)
    }
    /// Update a copy, rejecting duplicate or contradictory entries atomically.
    pub fn parse(mut self, arguments: &[&str]) -> Result<Self, CommandError> {
        let mut remaining = arguments;
        let (mut mode_seen, mut frame_seen, mut target_seen) = (false, false, false);
        let mut plane_seen = false;
        let mut curve_seen = false;
        let mut points = Vec::new();
        let previous_mode = self.mode;
        while let Some(token) = remaining.first() {
            if let Some(mode) = AlignmentMode::parse(token) {
                if mode_seen {
                    return Err(CommandError::Usage(USAGE));
                }
                mode_seen = true;
                self.mode = Some(mode);
                remaining = &remaining[1..];
            } else if option_name_eq(token, "3Point") {
                if plane_seen {
                    return Err(CommandError::Usage(USAGE));
                }
                plane_seen = true;
                self.three_point = true;
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
                || option_name_eq(token, "CurveId")
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
                } else if option_name_eq(name, "3Point") && !plane_seen {
                    self.three_point =
                        crate::parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                    plane_seen = true;
                } else if option_name_eq(name, "CurveId") && !curve_seen {
                    self.curve = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
                    curve_seen = true;
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
                remaining = &remaining[used..];
            } else {
                let (point, used) = parse_point(remaining)?;
                points.push(point);
                if points.len() > 3 {
                    return Err(CommandError::Usage(USAGE));
                }
                remaining = &remaining[used..];
            }
        }
        if mode_seen && previous_mode != self.mode {
            self.references = [None; 3];
            self.target = None;
            if !curve_seen {
                self.curve = None;
            }
            if !target_seen {
                self.automatic = false;
            }
            if !plane_seen {
                self.three_point = false;
            }
        }
        if self.three_point && self.mode != Some(AlignmentMode::ToPlane)
            || self.curve.is_some() && self.mode != Some(AlignmentMode::ToCurve)
            || self.projects() && (self.automatic || self.target.is_some())
            || self.references[self.reference_count()..]
                .iter()
                .any(Option::is_some)
        {
            return Err(CommandError::Usage(USAGE));
        }
        for point in points {
            self = self.with_point(point)?;
        }
        Ok(self)
    }

    pub const fn projects(self) -> bool {
        matches!(
            self.mode,
            Some(
                AlignmentMode::ToLine
                    | AlignmentMode::ToPlane
                    | AlignmentMode::ToFitPlane
                    | AlignmentMode::ToCurve
            )
        )
    }
}

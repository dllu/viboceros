//! Settings used by the staged offset commands.
use super::*;
use viboceros_geometry::{LengthUnitSystem, RibbonCornerStyle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Offset,
    Multiple,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Self> {
        if option_name_eq(name, "Offset") {
            Some(Self::Offset)
        } else if option_name_eq(name, "OffsetMultiple") {
            Some(Self::Multiple)
        } else {
            None
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Offset => "Offset",
            Self::Multiple => "OffsetMultiple",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cap {
    #[default]
    None,
    Flat,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub distance: Real,
    pub count: usize,
    pub corner: RibbonCornerStyle,
    pub input_layer: bool,
    pub both_sides: bool,
    pub through: bool,
    pub loose: bool,
    pub trim: bool,
    pub in_cplane: bool,
    pub tolerance: Real,
    pub cap: Cap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            distance: 1.,
            count: 1,
            corner: RibbonCornerStyle::Sharp,
            input_layer: false,
            both_sides: false,
            through: false,
            loose: false,
            trim: true,
            in_cplane: false,
            tolerance: 0.,
            cap: Cap::None,
        }
    }
}

impl Settings {
    pub fn remembered(self) -> Self {
        Self {
            both_sides: false,
            through: false,
            loose: false,
            ..self
        }
    }

    pub fn prompt(self, kind: Kind, selecting: bool) -> String {
        let point = if selecting {
            let hint = if kind == Kind::Offset {
                "Select a curve or surface edge"
            } else {
                "Select curves or surface edges. Enter continues"
            };
            format!("{hint} (Distance={})", self.distance)
        } else if self.through {
            "Through point".into()
        } else {
            format!("Side to offset (Distance={})", self.distance)
        };
        let mut options = format!("Loose={}", crate::cutting_options::yes_no(self.loose));
        if !self.loose {
            options.push_str(&format!(" Corner={:?}", self.corner));
        }
        if kind == Kind::Multiple {
            options.push_str(&format!(" OffsetCount={}", self.count));
        } else {
            if !self.through {
                options.push_str(" ThroughPoint");
            }
            if !self.loose {
                options.push_str(&format!(
                    " Trim={} Tolerance={}",
                    crate::cutting_options::yes_no(self.trim),
                    self.tolerance
                ));
            }
            if !self.both_sides {
                options.push_str(" BothSides");
            }
            options.push_str(&format!(
                " InCPlane={} Cap={:?}",
                crate::cutting_options::yes_no(self.in_cplane),
                self.cap
            ));
        }
        options.push_str(if self.input_layer {
            " OutputLayer=Input"
        } else {
            " OutputLayer=Current"
        });
        if selecting {
            options.push_str(" SubCrv");
        }
        format!("{point}. {options}")
    }

    pub fn fit_tolerance(self, model: Tolerance) -> Result<Tolerance, GeometryError> {
        if self.tolerance == 0. || self.loose {
            Ok(model)
        } else {
            Tolerance::try_new(self.tolerance, model.relative(), model.angular())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OptionName {
    Distance,
    Count,
    Corner,
    Tolerance,
    OutputLayer,
    Cap,
    Loose,
    BothSides,
    ThroughPoint,
    Trim,
    InCPlane,
}

impl OptionName {
    pub fn parse(kind: Kind, name: &str) -> Option<Self> {
        let common = [
            ("Distance", Self::Distance),
            ("Corner", Self::Corner),
            ("OutputLayer", Self::OutputLayer),
            ("Loose", Self::Loose),
        ];
        let specific: &[(&str, Self)] = match kind {
            Kind::Offset => &[
                ("ThroughPoint", Self::ThroughPoint),
                ("BothSides", Self::BothSides),
                ("Tolerance", Self::Tolerance),
                ("Cap", Self::Cap),
                ("Trim", Self::Trim),
                ("InCPlane", Self::InCPlane),
            ],
            Kind::Multiple => &[("OffsetCount", Self::Count)],
        };
        common
            .iter()
            .chain(specific)
            .find_map(|(key, option)| option_name_eq(name, key).then_some(*option))
    }

    pub const fn hint(self) -> &'static str {
        match self {
            Self::Distance => "Offset distance or first distance point",
            Self::Count => "Number of offsets",
            Self::Corner => "Corner (Sharp, Round, Smooth, Chamfer, None)",
            Self::Tolerance => "Offset tolerance (Default or a nonnegative length)",
            Self::OutputLayer => "OutputLayer (Current or Input)",
            Self::Cap => "Cap (None, Flat, Round)",
            Self::Loose => "Loose",
            Self::BothSides => "BothSides",
            Self::ThroughPoint => "ThroughPoint",
            Self::Trim => "Trim",
            Self::InCPlane => "InCPlane",
        }
    }

    pub const fn opens_menu(self) -> bool {
        !matches!(
            self,
            Self::Loose | Self::BothSides | Self::ThroughPoint | Self::Trim | Self::InCPlane
        )
    }

    pub fn set(
        self,
        settings: &mut Settings,
        value: Option<&str>,
        units: &LengthUnitSystem,
    ) -> Result<(), CommandError> {
        let invalid = || CommandError::Usage(self.hint());
        let value = value.unwrap_or("");
        match self {
            Self::Distance => {
                let distance = parse_length(value, units)?;
                if distance <= 0. {
                    return Err(GeometryError::InvalidCurveOffsetDistance.into());
                }
                settings.distance = distance;
                settings.through = false;
            }
            Self::Count => {
                settings.count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&count| (1..=MAX_OFFSET_OUTPUTS).contains(&count))
                    .ok_or_else(invalid)?;
            }
            Self::Tolerance => {
                let value = if option_name_eq(value, "Default") {
                    0.
                } else {
                    parse_length(value, units)?
                };
                if value < 0. {
                    return Err(invalid());
                }
                settings.tolerance = value;
            }
            Self::Corner => {
                settings.corner = if option_name_eq(value, "Smooth") {
                    RibbonCornerStyle::Smooth
                } else {
                    corner(value).ok_or_else(invalid)?.into()
                };
            }
            Self::OutputLayer => {
                settings.input_layer = layer(value).ok_or_else(invalid)?;
            }
            Self::Cap => {
                settings.cap = if option_name_eq(value, "None") {
                    Cap::None
                } else if option_name_eq(value, "Flat") {
                    Cap::Flat
                } else if option_name_eq(value, "Round") {
                    Cap::Round
                } else {
                    return Err(invalid());
                };
            }
            Self::Loose => {
                settings.loose = toggle(value, !settings.loose).ok_or_else(invalid)?;
            }
            Self::BothSides => {
                settings.both_sides = toggle(value, true).ok_or_else(invalid)?;
            }
            Self::ThroughPoint => {
                settings.through = toggle(value, true).ok_or_else(invalid)?;
            }
            Self::Trim => {
                settings.trim = toggle(value, !settings.trim).ok_or_else(invalid)?;
            }
            Self::InCPlane => {
                settings.in_cplane = toggle(value, !settings.in_cplane).ok_or_else(invalid)?;
            }
        }
        Ok(())
    }
}

fn toggle(value: &str, default: bool) -> Option<bool> {
    if value.is_empty() {
        Some(default)
    } else {
        parse_yes_no(value.trim_start_matches('_'))
    }
}

pub fn parse_length(value: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    viboceros_drafting::PointInput::parse_length_with_units(value.trim_start_matches('_'), units)
        .and_then(Result::ok)
        .filter(|d| d.is_finite())
        .ok_or_else(|| CommandError::InvalidNumber(value.to_owned()))
}

pub(super) fn corner(value: &str) -> Option<CurveOffsetCornerStyle> {
    [
        ("Sharp", CurveOffsetCornerStyle::Sharp),
        ("Round", CurveOffsetCornerStyle::Round),
        ("Chamfer", CurveOffsetCornerStyle::Chamfer),
        ("None", CurveOffsetCornerStyle::None),
    ]
    .into_iter()
    .find_map(|(name, corner)| option_name_eq(value, name).then_some(corner))
}

pub(super) fn layer(value: &str) -> Option<bool> {
    if option_name_eq(value, "Input") {
        Some(true)
    } else if option_name_eq(value, "Current") {
        Some(false)
    } else {
        None
    }
}

pub fn parse_settings(
    kind: Kind,
    words: &[&str],
    mut settings: Settings,
    units: &LengthUnitSystem,
) -> Result<(Settings, Option<OptionName>), CommandError> {
    let mut words = words.iter().copied().peekable();
    let mut seen = BTreeSet::new();
    let mut menu = None;
    while let Some(word) = words.next() {
        let (name, assignment) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let option = OptionName::parse(kind, name)
            .or_else(|| parse_length(word, units).ok().map(|_| OptionName::Distance));
        let Some(option) = option.filter(|option| seen.insert(*option)) else {
            return Err(CommandError::Usage(kind.name()));
        };
        let value = if OptionName::parse(kind, name).is_none() {
            Some(word)
        } else if assignment.is_some() {
            assignment
        } else if option.opens_menu() {
            if let Some(next) = words.next() {
                Some(next)
            } else {
                menu = Some(option);
                break;
            }
        } else {
            words.next_if(|value| parse_yes_no(value.trim_start_matches('_')).is_some())
        };
        option.set(&mut settings, value, units)?;
    }
    Ok((settings, menu))
}

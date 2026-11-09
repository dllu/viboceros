use super::*;
use viboceros_geometry::{LengthUnitSystem, RibbonCornerStyle};

pub const USAGE: &str = "Ribbon [distance|Distance=value] side-point [BothSides=Yes|No] [Corner=Sharp|Round|Smooth|Chamfer|None] [Loose=Yes|No] [InCPlane=Yes|No] [OutputLayer=Current|Input] [Tolerance=value|Default]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: Real,
    pub both_sides: bool,
    pub corner: RibbonCornerStyle,
    pub loose: bool,
    pub in_cplane: bool,
    pub input_layer: bool,
    pub tolerance: Real,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 1.,
            both_sides: false,
            corner: RibbonCornerStyle::Sharp,
            loose: false,
            in_cplane: false,
            input_layer: false,
            tolerance: 0.,
        }
    }
}
impl Options {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !self.distance.is_finite()
            || self.distance <= 0.
            || !self.tolerance.is_finite()
            || self.tolerance < 0.
        {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(self)
    }
    pub fn arguments(self) -> String {
        let yes = |b| if b { "Yes" } else { "No" };
        format!(
            "Distance={} BothSides={} Corner={} Loose={} InCPlane={} OutputLayer={} Tolerance={}",
            self.distance,
            yes(self.both_sides),
            self.corner_name(),
            yes(self.loose),
            yes(self.in_cplane),
            if self.input_layer { "Input" } else { "Current" },
            self.tolerance
        )
    }
    pub fn corner_name(self) -> &'static str {
        match self.corner {
            RibbonCornerStyle::Sharp => "Sharp",
            RibbonCornerStyle::Round => "Round",
            RibbonCornerStyle::Smooth => "Smooth",
            RibbonCornerStyle::Chamfer => "Chamfer",
            RibbonCornerStyle::None => "None",
        }
    }
    pub fn remembered(self) -> Self {
        Self {
            both_sides: false,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub options: Options,
    pub side: Option<Point3>,
}

pub fn length(text: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    viboceros_drafting::PointInput::parse_length_with_units(text.trim_start_matches('_'), units)
        .ok_or(CommandError::Usage(USAGE))?
        .map_err(|_| CommandError::Usage(USAGE))
}

pub fn parse(
    args: &[&str],
    mut options: Options,
    units: &LengthUnitSystem,
    context: CommandContext,
) -> Result<Input, CommandError> {
    let mut seen = [false; 7];
    let mut side = None;
    let mut i = 0;
    while i < args.len() {
        let word = args[i];
        let mut consumed = 1;
        let (name, value) = if let Some(pair) = word.split_once('=') {
            pair
        } else if option_name_eq(word, "BothSides") {
            ("BothSides", "Yes")
        } else if length(word, units).is_ok() {
            ("Distance", word)
        } else if let Some(point) = viboceros_drafting::PointInput::parse_with_units(word, units) {
            let point = point
                .and_then(|p| p.resolve(context.construction_plane, None))
                .map_err(|_| CommandError::Usage(USAGE))?;
            if side.replace(point).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            i += 1;
            continue;
        } else {
            consumed = 2;
            (word, *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?)
        };
        let index = if option_name_eq(name, "Distance") {
            options.distance = length(value, units)?;
            0
        } else if option_name_eq(name, "BothSides") {
            options.both_sides = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            1
        } else if option_name_eq(name, "Corner") {
            options.corner = if option_name_eq(value, "Sharp") {
                RibbonCornerStyle::Sharp
            } else if option_name_eq(value, "Round") {
                RibbonCornerStyle::Round
            } else if option_name_eq(value, "Smooth") {
                RibbonCornerStyle::Smooth
            } else if option_name_eq(value, "Chamfer") {
                RibbonCornerStyle::Chamfer
            } else if option_name_eq(value, "None") {
                RibbonCornerStyle::None
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            2
        } else if option_name_eq(name, "Loose") {
            options.loose = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            3
        } else if option_name_eq(name, "InCPlane") {
            options.in_cplane = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            4
        } else if option_name_eq(name, "OutputLayer") {
            options.input_layer = if option_name_eq(value, "Input") {
                true
            } else if option_name_eq(value, "Current") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            5
        } else if option_name_eq(name, "Tolerance") {
            options.tolerance = if option_name_eq(value, "Default") {
                0.
            } else {
                length(value, units)?
            };
            6
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if std::mem::replace(&mut seen[index], true) {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    Ok(Input {
        options: options.validate()?,
        side,
    })
}

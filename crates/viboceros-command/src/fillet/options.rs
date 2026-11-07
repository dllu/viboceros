//! Session preferences and unit-aware command options.
use super::*;
use viboceros_geometry::LengthUnitSystem;

#[derive(Clone, Debug)]
pub struct Arguments {
    pub preferences: Preferences,
    pub sources: Option<[ObjectId; 2]>,
    pub picks: [Option<Point3>; 2],
}

pub fn parse(
    words: &[&str],
    defaults: Preferences,
    units: &LengthUnitSystem,
) -> Result<Arguments, CommandError> {
    let mut result = Arguments {
        preferences: defaults,
        sources: None,
        picks: [None; 2],
    };
    let mut options = Vec::new();
    for word in words {
        let Some((name, value)) = word.split_once('=') else {
            options.push(*word);
            continue;
        };
        if option_name_eq(name, "Sources") {
            let (a, b) = value.split_once(',').ok_or(CommandError::Usage(USAGE))?;
            let ids = [
                a.parse().map_err(|_| CommandError::Usage(USAGE))?,
                b.parse().map_err(|_| CommandError::Usage(USAGE))?,
            ];
            if result.sources.replace(ids).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if let Some(index) = ["Pick1", "Pick2"]
            .iter()
            .position(|key| option_name_eq(name, key))
        {
            let (point, consumed) = parse_point(&[value])?;
            if consumed != 1 || result.picks[index].replace(point).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            options.push(*word);
        }
    }
    result.preferences = defaults.updated(&options, units)?;
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub radius: Real,
    pub join: bool,
    pub trim: bool,
    pub dynamic: bool,
    pub styles: CurveFilletExtensionStyles,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            radius: 1.,
            join: false,
            trim: true,
            dynamic: false,
            styles: CurveFilletExtensionStyles::default(),
        }
    }
}

impl Preferences {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !self.radius.is_finite() || self.radius < 0. {
            return Err(CommandError::Usage(
                "Fillet radius must be finite and nonnegative",
            ));
        }
        Ok(self)
    }

    /// Parse a complete option edit before changing remembered settings.
    pub fn updated(self, words: &[&str], units: &LengthUnitSystem) -> Result<Self, CommandError> {
        let mut result = self;
        let mut seen = BTreeSet::new();
        for word in words {
            let (name, value) = word.split_once('=').unwrap_or(("Radius", word));
            let key = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let value = value.trim_start_matches('_');
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "radius" => result.radius = parse_radius(value, units)?,
                "join" => result.join = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "trim" => result.trim = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "dynamic" => {
                    result.dynamic = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "extendarcsby" => {
                    result.styles.arc = if value.eq_ignore_ascii_case("Arc") {
                        CurveArcExtensionStyle::Arc
                    } else if value.eq_ignore_ascii_case("Line") {
                        CurveArcExtensionStyle::Line
                    } else {
                        return Err(CommandError::Usage(USAGE));
                    }
                }
                "extendothercurvesby" => {
                    result.styles.other = if value.eq_ignore_ascii_case("Line") {
                        CurveOtherExtensionStyle::Line
                    } else if value.eq_ignore_ascii_case("Smooth") {
                        CurveOtherExtensionStyle::Smooth
                    } else {
                        return Err(CommandError::Usage(USAGE));
                    }
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        result.validate()
    }

    pub fn arguments(self) -> String {
        format!(
            "Radius={} Join={} Trim={} Dynamic={} ExtendArcsBy={} ExtendOtherCurvesBy={}",
            self.radius,
            yes_no(self.join),
            yes_no(self.trim),
            yes_no(self.dynamic),
            if self.styles.arc == CurveArcExtensionStyle::Arc {
                "Arc"
            } else {
                "Line"
            },
            if self.styles.other == CurveOtherExtensionStyle::Line {
                "Line"
            } else {
                "Smooth"
            }
        )
    }
}

pub fn parse_radius(text: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    let value = viboceros_drafting::PointInput::parse_length_with_units(text, units)
        .and_then(Result::ok)
        .filter(|v| v.is_finite() && *v >= 0.)
        .ok_or(CommandError::Usage(
            "Fillet radius must be finite and nonnegative",
        ))?;
    Ok(value)
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

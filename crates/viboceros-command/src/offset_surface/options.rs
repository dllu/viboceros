//! Command preferences are independent of model history and direction picks.
use super::*;
use viboceros_geometry::LengthUnitSystem;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: Real,
    pub solid: bool,
    pub both_sides: bool,
    pub delete_input: bool,
    pub flip_all: bool,
    pub loose: bool,
    /// Zero follows the current document's absolute tolerance.
    pub tolerance: Real,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 1.,
            solid: false,
            both_sides: false,
            delete_input: false,
            flip_all: false,
            loose: false,
            tolerance: 0.,
        }
    }
}

impl Options {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !self.distance.is_finite() || !self.tolerance.is_finite() || self.tolerance < 0. {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(self)
    }
    pub fn remembered(self) -> Self {
        Self {
            flip_all: false,
            ..self
        }
    }
    pub fn effective_tolerance(self, model: Tolerance) -> Real {
        if self.tolerance == 0. {
            model.absolute()
        } else {
            self.tolerance
        }
    }
    pub fn normalize_tolerance(self, model: Tolerance) -> Self {
        Self {
            tolerance: if self.tolerance == model.absolute() {
                0.
            } else {
                self.tolerance
            },
            ..self
        }
    }
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        format!(
            "Corner=Sharp Distance={} Solid={} BothSides={} Loose={} Tolerance={} DeleteInput={}",
            self.distance,
            yes(self.solid),
            yes(self.both_sides),
            yes(self.loose),
            self.tolerance,
            yes(self.delete_input)
        )
    }
    pub fn prompt_arguments(self) -> String {
        let text = self.arguments();
        let text = if self.loose {
            text.replace(&format!(" Tolerance={}", self.tolerance), "")
        } else {
            text
        };
        if self.solid {
            text
        } else {
            text.split(" DeleteInput=").next().unwrap().to_owned()
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub options: Options,
    pub distance_supplied: bool,
    pub through_point: Option<Point3>,
}

pub fn distance(text: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    viboceros_drafting::PointInput::parse_length_with_units(text.trim_start_matches('_'), units)
        .ok_or(CommandError::Usage(USAGE))?
        .map_err(|_| CommandError::Usage(USAGE))
}

pub fn parse(
    args: &[&str],
    mut options: Options,
    units: &LengthUnitSystem,
) -> Result<Input, CommandError> {
    let mut seen = [false; 8];
    let mut through_point = None;
    let mut i = 0;
    while i < args.len() {
        let word = args[i];
        let mut consumed = 1;
        let (name, value) = if let Some(pair) = word.split_once('=') {
            pair
        } else if option_name_eq(word, "FlipAll") {
            ("FlipAll", if options.flip_all { "No" } else { "Yes" })
        } else if distance(word, units).is_ok() {
            ("Distance", word)
        } else {
            consumed = 2;
            (word, *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?)
        };
        if option_name_eq(name, "ThroughPoint") {
            let p = viboceros_drafting::PointInput::parse_with_units(value, units)
                .ok_or(CommandError::Usage(USAGE))?
                .and_then(|p| p.resolve(CommandContext::default().construction_plane, None))
                .map_err(|_| CommandError::Usage(USAGE))?;
            if through_point.replace(p).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            i += consumed;
            continue;
        }
        let index = if option_name_eq(name, "Corner") && option_name_eq(value, "Sharp") {
            5
        } else if option_name_eq(name, "Distance") {
            options.distance = distance(value, units)?;
            0
        } else if option_name_eq(name, "Tolerance") {
            options.tolerance = if option_name_eq(value, "Default") {
                0.
            } else {
                distance(value, units)?
            };
            7
        } else {
            let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "Solid") {
                options.solid = value;
                1
            } else if option_name_eq(name, "BothSides") {
                options.both_sides = value;
                2
            } else if option_name_eq(name, "DeleteInput") {
                options.delete_input = value;
                3
            } else if option_name_eq(name, "FlipAll") {
                options.flip_all = value;
                4
            } else if option_name_eq(name, "Loose") {
                options.loose = value;
                6
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        };
        if std::mem::replace(&mut seen[index], true) {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    options.validate()?;
    Ok(Input {
        options,
        distance_supplied: seen[0],
        through_point,
    })
}

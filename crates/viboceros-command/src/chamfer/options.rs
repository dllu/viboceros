//! Remembered construction settings and explicit ordered source bindings.
use super::*;
use std::collections::BTreeSet;
use viboceros_geometry::LengthUnitSystem;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub distances: [Real; 2],
    pub join: bool,
    pub trim: bool,
    pub styles: CurveChamferExtensionStyles,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            distances: [1., 1.],
            join: false,
            trim: true,
            styles: CurveChamferExtensionStyles::default(),
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<Self, CommandError> {
        if self.distances.iter().any(|v| !v.is_finite() || *v < 0.) {
            return Err(CommandError::Usage(
                "Chamfer distances must be finite and nonnegative",
            ));
        }
        Ok(self)
    }
    pub fn updated(self, words: &[&str], units: &LengthUnitSystem) -> Result<Self, CommandError> {
        let mut result = self;
        let mut seen = BTreeSet::new();
        for word in words {
            let (key, value) = word.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let key = key.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let value = value.trim_start_matches('_');
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "distances" => {
                    let (a, b) = value.split_once(',').ok_or(CommandError::Usage(USAGE))?;
                    result.distances = [parse_distance(a, units)?, parse_distance(b, units)?];
                }
                "join" => result.join = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "trim" => result.trim = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
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
        let yes = |v| if v { "Yes" } else { "No" };
        format!(
            "Distances={},{} Join={} Trim={} ExtendArcsBy={} ExtendOtherCurvesBy={}",
            self.distances[0],
            self.distances[1],
            yes(self.join),
            yes(self.trim),
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
pub fn parse_distance(text: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    viboceros_drafting::PointInput::parse_length_with_units(text, units)
        .and_then(Result::ok)
        .filter(|v| v.is_finite() && *v >= 0.)
        .ok_or(CommandError::Usage(
            "Chamfer distance must be finite and nonnegative",
        ))
}
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
        picks: [None, None],
    };
    let mut options = Vec::new();
    let mut index = 0;
    if words.first().is_some_and(|w| !w.contains('=')) {
        let second = words.get(1).ok_or(CommandError::Usage(USAGE))?;
        result.preferences.distances = [
            parse_distance(words[0], units)?,
            parse_distance(second, units)?,
        ];
        index = 2;
    }
    for &word in &words[index..] {
        let Some((key, value)) = word.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if option_name_eq(key, "Sources") {
            if result.sources.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let (a, b) = value.split_once(',').ok_or(CommandError::Usage(USAGE))?;
            result.sources = Some([
                a.parse().map_err(|_| CommandError::Usage(USAGE))?,
                b.parse().map_err(|_| CommandError::Usage(USAGE))?,
            ]);
        } else if option_name_eq(key, "Pick1") || option_name_eq(key, "Pick2") {
            let i = usize::from(option_name_eq(key, "Pick2"));
            if result.picks[i].is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let (p, n) = parse_point(&[value])?;
            if n != 1 {
                return Err(CommandError::Usage(USAGE));
            }
            result.picks[i] = Some(p);
        } else {
            options.push(word);
        }
    }
    result.preferences = result.preferences.updated(&options, units)?;
    Ok(result)
}

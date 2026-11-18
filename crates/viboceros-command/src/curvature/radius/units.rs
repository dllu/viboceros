//! Radius and Diameter accept Rhino's unit menu spellings and inch formats.
use super::*;

pub fn radius_display_units(input: &str) -> Result<Option<&'static str>, CommandError> {
    let input = input.trim_start_matches('_');
    for token in [
        "Decimal_Inches",
        "Fractional_Inches",
        "Decimal_Feet",
        "Feet_Inches",
    ] {
        if token.eq_ignore_ascii_case(input) {
            return Ok(Some(token));
        }
    }
    crate::distance_display_units(input)
}

#[derive(Default)]
pub(super) enum DisplayUnits {
    #[default]
    Model,
    Decimal(LengthUnitSystem),
    FractionalInches,
    FeetInches,
}

impl DisplayUnits {
    pub(super) fn parse(units: Option<&str>) -> Result<Self, CommandError> {
        Ok(
            match units.map(radius_display_units).transpose()?.flatten() {
                None => Self::Model,
                Some("Decimal_Inches") => Self::Decimal(LengthUnitSystem::Inches),
                Some("Decimal_Feet") => Self::Decimal(LengthUnitSystem::Feet),
                Some("Fractional_Inches") => Self::FractionalInches,
                Some("Feet_Inches") => Self::FeetInches,
                Some(units) => Self::Decimal(
                    crate::model_units::parse_units(units)
                        .ok_or(CommandError::Usage("Unknown radius display units"))?,
                ),
            },
        )
    }

    pub(super) fn scale(&self, document: &Document) -> Result<f64, CommandError> {
        let target = match self {
            Self::Model => None,
            Self::Decimal(unit) => Some(unit),
            Self::FractionalInches | Self::FeetInches => Some(&LengthUnitSystem::Inches),
        };
        measurement_display_scale(
            document,
            target,
            "Radius/Diameter display conversion requires physical source units",
        )
    }

    pub(super) fn format(&self, value: f64) -> String {
        match self {
            Self::Model => value.to_string(),
            Self::Decimal(unit) => format!("{value} {}", unit.name()),
            Self::FractionalInches => format!("{} inches", inches(value, false)),
            Self::FeetInches => inches(value, true),
        }
    }
}

fn inches(value: f64, include_feet: bool) -> String {
    // Rhino's standard eighth-inch display rounds only the text value.
    let integral = value.floor();
    let eighths = ((value - integral) * 8.0).round() as u8;
    let integral = if eighths == 8 {
        integral + 1.0
    } else {
        integral
    };
    let fraction = match eighths % 8 {
        0 => None,
        2 => Some("1/4".into()),
        4 => Some("1/2".into()),
        6 => Some("3/4".into()),
        n => Some(format!("{n}/8")),
    };
    if include_feet {
        let feet = (integral / 12.0).floor();
        let whole_inches = integral % 12.0;
        let part = match fraction {
            Some(fraction) => format!("{whole_inches}-{fraction}\""),
            None => format!("{whole_inches}\""),
        };
        return if feet > 0.0 {
            format!("{feet}'{part}")
        } else {
            part
        };
    }
    match fraction {
        Some(fraction) if integral == 0.0 => fraction,
        Some(fraction) => format!("{integral}-{fraction}"),
        None => integral.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_formats_native_feet_and_inches() {
        assert_eq!(DisplayUnits::FeetInches.format(28.625), "2'4-5/8\"");
        assert_eq!(DisplayUnits::FeetInches.format(57.25), "4'9-1/4\"");
        assert_eq!(DisplayUnits::FeetInches.format(11.99), "1'0\"");
    }
}

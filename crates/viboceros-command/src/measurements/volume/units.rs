//! Volume-only display preferences: a liter is a cubic decimeter, not a
//! document length unit. ModelUnits follows current metadata on every query.
use viboceros_geometry::LengthUnitSystem;

pub(super) const CHOICES: &[&str] = &[
    "ModelUnits",
    "Micron",
    "Millimeter",
    "Centimeter",
    "Liter",
    "Decimeter",
    "Meter",
    "Kilometer",
    "Microinch",
    "Mil",
    "Inch",
    "Foot",
    "Yard",
    "Mile",
];

pub(super) fn parse(input: &str) -> Option<&'static str> {
    CHOICES
        .iter()
        .copied()
        .find(|choice| crate::option_name_eq(input, choice))
}

pub(super) fn target(choice: &str) -> Option<LengthUnitSystem> {
    use LengthUnitSystem::*;
    Some(match choice {
        "ModelUnits" => return Option::None,
        "Micron" => Microns,
        "Millimeter" => Millimeters,
        "Centimeter" => Centimeters,
        "Liter" | "Decimeter" => Decimeters,
        "Meter" => Meters,
        "Kilometer" => Kilometers,
        "Microinch" => Microinches,
        "Mil" => Mils,
        "Inch" => Inches,
        "Foot" => Feet,
        "Yard" => Yards,
        "Mile" => Miles,
        _ => unreachable!("validated volume display unit"),
    })
}

pub(super) fn label(choice: &str) -> String {
    if choice == "Liter" {
        " liters".into()
    } else {
        target(choice)
            .map(|unit| format!(" cubic {}", unit.name()))
            .unwrap_or_default()
    }
}

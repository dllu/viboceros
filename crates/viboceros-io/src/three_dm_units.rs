//! OpenNURBS identifiers stay in the file-format adapter.
use crate::ThreeDmError;
use std::ffi::CString;
use viboceros_geometry::LengthUnitSystem;

pub(crate) fn encode(units: &LengthUnitSystem) -> Result<(u32, f64, CString), ThreeDmError> {
    units
        .validate()
        .map_err(|error| ThreeDmError::InvalidModel(error.to_string()))?;
    let (code, scale, name) = match units {
        LengthUnitSystem::None => (0, 1.0, ""),
        LengthUnitSystem::Microns => (1, 1.0, ""),
        LengthUnitSystem::Millimeters => (2, 1.0, ""),
        LengthUnitSystem::Centimeters => (3, 1.0, ""),
        LengthUnitSystem::Meters => (4, 1.0, ""),
        LengthUnitSystem::Kilometers => (5, 1.0, ""),
        LengthUnitSystem::Microinches => (6, 1.0, ""),
        LengthUnitSystem::Mils => (7, 1.0, ""),
        LengthUnitSystem::Inches => (8, 1.0, ""),
        LengthUnitSystem::Feet => (9, 1.0, ""),
        LengthUnitSystem::Miles => (10, 1.0, ""),
        LengthUnitSystem::Angstroms => (12, 1.0, ""),
        LengthUnitSystem::Nanometers => (13, 1.0, ""),
        LengthUnitSystem::Decimeters => (14, 1.0, ""),
        LengthUnitSystem::Dekameters => (15, 1.0, ""),
        LengthUnitSystem::Hectometers => (16, 1.0, ""),
        LengthUnitSystem::Megameters => (17, 1.0, ""),
        LengthUnitSystem::Gigameters => (18, 1.0, ""),
        LengthUnitSystem::Yards => (19, 1.0, ""),
        LengthUnitSystem::PrinterPoints => (20, 1.0, ""),
        LengthUnitSystem::PrinterPicas => (21, 1.0, ""),
        LengthUnitSystem::NauticalMiles => (22, 1.0, ""),
        LengthUnitSystem::AstronomicalUnits => (23, 1.0, ""),
        LengthUnitSystem::LightYears => (24, 1.0, ""),
        LengthUnitSystem::Parsecs => (25, 1.0, ""),
        LengthUnitSystem::Unset => (255, 1.0, ""),
        LengthUnitSystem::Custom {
            name,
            meters_per_unit,
        } => (11, *meters_per_unit, name.as_str()),
    };
    let name = CString::new(name)
        .map_err(|_| ThreeDmError::InvalidModel("custom unit name contains a NUL byte".into()))?;
    Ok((code, scale, name))
}

pub(crate) fn decode(
    code: u32,
    scale: f64,
    name: String,
) -> Result<LengthUnitSystem, ThreeDmError> {
    let units = match code {
        0 => LengthUnitSystem::None,
        1 => LengthUnitSystem::Microns,
        2 => LengthUnitSystem::Millimeters,
        3 => LengthUnitSystem::Centimeters,
        4 => LengthUnitSystem::Meters,
        5 => LengthUnitSystem::Kilometers,
        6 => LengthUnitSystem::Microinches,
        7 => LengthUnitSystem::Mils,
        8 => LengthUnitSystem::Inches,
        9 => LengthUnitSystem::Feet,
        10 => LengthUnitSystem::Miles,
        12 => LengthUnitSystem::Angstroms,
        13 => LengthUnitSystem::Nanometers,
        14 => LengthUnitSystem::Decimeters,
        15 => LengthUnitSystem::Dekameters,
        16 => LengthUnitSystem::Hectometers,
        17 => LengthUnitSystem::Megameters,
        18 => LengthUnitSystem::Gigameters,
        19 => LengthUnitSystem::Yards,
        20 => LengthUnitSystem::PrinterPoints,
        21 => LengthUnitSystem::PrinterPicas,
        22 => LengthUnitSystem::NauticalMiles,
        23 => LengthUnitSystem::AstronomicalUnits,
        24 => LengthUnitSystem::LightYears,
        25 => LengthUnitSystem::Parsecs,
        255 => LengthUnitSystem::Unset,
        11 => LengthUnitSystem::Custom {
            name,
            meters_per_unit: scale,
        },
        _ => return Err(ThreeDmError::MalformedBridge("unknown length unit system")),
    };
    encode(&units)?;
    Ok(units)
}

//! 3DM length-unit metadata. Changing this value does not rescale geometry.
use crate::ThreeDmError;
use std::ffi::CString;

/// OpenNURBS unit identifiers, including the distinction between unitless and unset.
/// Custom scales are metres per unit and must be finite and positive.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ThreeDmUnitSystem {
    None,
    Microns,
    #[default]
    Millimeters,
    Centimeters,
    Meters,
    Kilometers,
    Microinches,
    Mils,
    Inches,
    Feet,
    Miles,
    Angstroms,
    Nanometers,
    Decimeters,
    Dekameters,
    Hectometers,
    Megameters,
    Gigameters,
    Yards,
    PrinterPoints,
    PrinterPicas,
    NauticalMiles,
    AstronomicalUnits,
    LightYears,
    Parsecs,
    Unset,
    Custom {
        name: String,
        meters_per_unit: f64,
    },
}

impl ThreeDmUnitSystem {
    pub(crate) fn encode(&self) -> Result<(u32, f64, CString), ThreeDmError> {
        let (code, scale, name) = match self {
            Self::None => (0, 1.0, ""),
            Self::Microns => (1, 1.0, ""),
            Self::Millimeters => (2, 1.0, ""),
            Self::Centimeters => (3, 1.0, ""),
            Self::Meters => (4, 1.0, ""),
            Self::Kilometers => (5, 1.0, ""),
            Self::Microinches => (6, 1.0, ""),
            Self::Mils => (7, 1.0, ""),
            Self::Inches => (8, 1.0, ""),
            Self::Feet => (9, 1.0, ""),
            Self::Miles => (10, 1.0, ""),
            Self::Angstroms => (12, 1.0, ""),
            Self::Nanometers => (13, 1.0, ""),
            Self::Decimeters => (14, 1.0, ""),
            Self::Dekameters => (15, 1.0, ""),
            Self::Hectometers => (16, 1.0, ""),
            Self::Megameters => (17, 1.0, ""),
            Self::Gigameters => (18, 1.0, ""),
            Self::Yards => (19, 1.0, ""),
            Self::PrinterPoints => (20, 1.0, ""),
            Self::PrinterPicas => (21, 1.0, ""),
            Self::NauticalMiles => (22, 1.0, ""),
            Self::AstronomicalUnits => (23, 1.0, ""),
            Self::LightYears => (24, 1.0, ""),
            Self::Parsecs => (25, 1.0, ""),
            Self::Unset => (255, 1.0, ""),
            Self::Custom {
                name,
                meters_per_unit,
            } => {
                if !meters_per_unit.is_finite() || *meters_per_unit <= 0.0 {
                    return Err(ThreeDmError::InvalidModel(
                        "custom unit scale must be finite and positive".into(),
                    ));
                }
                (11, *meters_per_unit, name.as_str())
            }
        };
        let name = CString::new(name).map_err(|_| {
            ThreeDmError::InvalidModel("custom unit name contains a NUL byte".into())
        })?;
        Ok((code, scale, name))
    }

    pub(crate) fn decode(code: u32, scale: f64, name: String) -> Result<Self, ThreeDmError> {
        let units = match code {
            0 => Self::None,
            1 => Self::Microns,
            2 => Self::Millimeters,
            3 => Self::Centimeters,
            4 => Self::Meters,
            5 => Self::Kilometers,
            6 => Self::Microinches,
            7 => Self::Mils,
            8 => Self::Inches,
            9 => Self::Feet,
            10 => Self::Miles,
            12 => Self::Angstroms,
            13 => Self::Nanometers,
            14 => Self::Decimeters,
            15 => Self::Dekameters,
            16 => Self::Hectometers,
            17 => Self::Megameters,
            18 => Self::Gigameters,
            19 => Self::Yards,
            20 => Self::PrinterPoints,
            21 => Self::PrinterPicas,
            22 => Self::NauticalMiles,
            23 => Self::AstronomicalUnits,
            24 => Self::LightYears,
            25 => Self::Parsecs,
            255 => Self::Unset,
            11 => Self::Custom {
                name,
                meters_per_unit: scale,
            },
            _ => return Err(ThreeDmError::MalformedBridge("unknown length unit system")),
        };
        units.encode()?;
        Ok(units)
    }
}

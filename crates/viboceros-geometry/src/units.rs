//! Length-unit metadata and checked conversion factors.
use thiserror::Error;

/// Length units shared by the geometry, document, and interchange layers.
/// Assigning metadata alone never changes coordinates. Astronomical factors
/// retain the OpenNURBS definitions used by 3DM files, not newer SI revisions.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LengthUnitSystem {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(scale: f64) -> LengthUnitSystem {
        LengthUnitSystem::Custom {
            name: "custom".into(),
            meters_per_unit: scale,
        }
    }

    #[test]
    fn physical_conversion_factors_have_the_correct_direction() {
        use LengthUnitSystem::*;
        for (from, to, expected) in [
            (Meters, Millimeters, 1000.0),
            (Millimeters, Meters, 0.001),
            (Inches, Millimeters, 25.4),
            (Feet, Inches, 12.0),
            (Yards, Feet, 3.0),
            (Miles, Feet, 5280.0),
            (PrinterPoints, PrinterPicas, 1.0 / 12.0),
            (NauticalMiles, Meters, 1852.0),
            (custom(0.125), Centimeters, 12.5),
            (custom(0.125), custom(2.0), 0.0625),
        ] {
            let actual = from.scale_to(&to).unwrap();
            assert!((actual - expected).abs() <= expected.abs() * 1e-14);
            assert!((actual * to.scale_to(&from).unwrap() - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn unitless_and_unset_are_distinct() {
        use LengthUnitSystem::*;
        assert_eq!(None.meters_per_unit(), Ok(Option::None));
        assert_eq!(Unset.meters_per_unit(), Ok(Option::None));
        for units in [None, Meters, custom(0.25)] {
            assert_eq!(None.scale_to(&units), Ok(1.0));
            assert_eq!(units.scale_to(&None), Ok(1.0));
            assert_eq!(Unset.scale_to(&units), Err(UnitError::Unset));
            assert_eq!(units.scale_to(&Unset), Err(UnitError::Unset));
        }
        assert_eq!(Unset.scale_to(&Unset), Err(UnitError::Unset));
    }

    #[test]
    fn invalid_or_unrepresentable_scales_are_errors() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(custom(scale).validate(), Err(UnitError::InvalidCustomScale));
            assert!(custom(scale).scale_to(&LengthUnitSystem::None).is_err());
        }
        let tiny = custom(f64::from_bits(1));
        let huge = custom(f64::MAX);
        assert_eq!(tiny.scale_to(&tiny), Ok(1.0));
        assert_eq!(huge.scale_to(&huge), Ok(1.0));
        assert_eq!(tiny.scale_to(&huge), Err(UnitError::UnrepresentableScale));
        assert_eq!(huge.scale_to(&tiny), Err(UnitError::UnrepresentableScale));
        assert_eq!(
            LengthUnitSystem::Custom {
                name: "bad\0name".into(),
                meters_per_unit: 1.0
            }
            .validate(),
            Err(UnitError::InvalidCustomName)
        );
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum UnitError {
    #[error("custom unit scale must be finite and positive")]
    InvalidCustomScale,
    #[error("custom unit name contains a NUL byte")]
    InvalidCustomName,
    #[error("cannot convert an unset unit system")]
    Unset,
    #[error("unit conversion factor overflows or underflows")]
    UnrepresentableScale,
}

impl LengthUnitSystem {
    /// Human-readable model-unit name; custom names are retained verbatim.
    /// UI callers should bound or truncate untrusted custom names as needed.
    pub fn name(&self) -> &str {
        match self {
            Self::None => "Unitless",
            Self::Microns => "Microns",
            Self::Millimeters => "Millimetres",
            Self::Centimeters => "Centimetres",
            Self::Meters => "Metres",
            Self::Kilometers => "Kilometres",
            Self::Microinches => "Microinches",
            Self::Mils => "Mils",
            Self::Inches => "Inches",
            Self::Feet => "Feet",
            Self::Miles => "Miles",
            Self::Angstroms => "Angstroms",
            Self::Nanometers => "Nanometres",
            Self::Decimeters => "Decimetres",
            Self::Dekameters => "Dekametres",
            Self::Hectometers => "Hectometres",
            Self::Megameters => "Megametres",
            Self::Gigameters => "Gigametres",
            Self::Yards => "Yards",
            Self::PrinterPoints => "Printer points",
            Self::PrinterPicas => "Printer picas",
            Self::NauticalMiles => "Nautical miles",
            Self::AstronomicalUnits => "Astronomical units",
            Self::LightYears => "Light years",
            Self::Parsecs => "Parsecs",
            Self::Unset => "Unset units",
            Self::Custom { name, .. } => name,
        }
    }

    pub fn validate(&self) -> Result<(), UnitError> {
        if let Self::Custom {
            name,
            meters_per_unit,
        } = self
        {
            if !meters_per_unit.is_finite() || *meters_per_unit <= 0.0 {
                return Err(UnitError::InvalidCustomScale);
            }
            if name.contains('\0') {
                return Err(UnitError::InvalidCustomName);
            }
        }
        Ok(())
    }

    /// Metres per physical unit; unitless and unset have no physical scale.
    pub fn meters_per_unit(&self) -> Result<Option<f64>, UnitError> {
        self.validate()?;
        Ok(Some(match self {
            Self::None | Self::Unset => return Ok(None),
            Self::Microns => 1e-6,
            Self::Millimeters => 1e-3,
            Self::Centimeters => 1e-2,
            Self::Meters => 1.0,
            Self::Kilometers => 1e3,
            Self::Microinches => 2.54e-8,
            Self::Mils => 2.54e-5,
            Self::Inches => 0.0254,
            Self::Feet => 0.3048,
            Self::Miles => 1609.344,
            Self::Angstroms => 1e-10,
            Self::Nanometers => 1e-9,
            Self::Decimeters => 0.1,
            Self::Dekameters => 10.0,
            Self::Hectometers => 100.0,
            Self::Megameters => 1e6,
            Self::Gigameters => 1e9,
            Self::Yards => 0.9144,
            Self::PrinterPoints => 0.0254 / 72.0,
            Self::PrinterPicas => 0.0254 / 6.0,
            Self::NauticalMiles => 1852.0,
            Self::AstronomicalUnits => 1.4959787e11,
            Self::LightYears => 9.4607304725808e15,
            Self::Parsecs => 3.08567758e16,
            Self::Custom {
                meters_per_unit, ..
            } => *meters_per_unit,
        }))
    }

    /// Factor multiplying coordinates expressed in self to express them in target.
    /// Unitless conversions leave coordinates unchanged; unset is an error,
    /// including unset-to-unset and unset-to-unitless.
    pub fn scale_to(&self, target: &Self) -> Result<f64, UnitError> {
        self.validate()?;
        target.validate()?;
        if matches!(self, Self::Unset) || matches!(target, Self::Unset) {
            return Err(UnitError::Unset);
        }
        if matches!(self, Self::None) || matches!(target, Self::None) {
            return Ok(1.0);
        }
        let scale = self.meters_per_unit()?.expect("physical source")
            / target.meters_per_unit()?.expect("physical target");
        if scale.is_finite() && scale > 0.0 {
            Ok(scale)
        } else {
            Err(UnitError::UnrepresentableScale)
        }
    }
}

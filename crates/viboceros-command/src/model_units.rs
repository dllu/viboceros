//! Explicit command-line access to model-unit settings, not layout units.

use crate::{Command, CommandError, parse_finite_real};
use viboceros_document::Document;
use viboceros_geometry::LengthUnitSystem;

const USAGE: &str = "Units | Units unit_name Scale=Yes|No | Units Custom MetersPerUnit=value Scale=Yes|No Name=name";

pub(super) struct UnitsCommand;

impl Command for UnitsCommand {
    fn name(&self) -> &'static str {
        "Units"
    }

    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        let mut rest = input.trim();
        if !rest
            .split_whitespace()
            .next()
            .is_some_and(|first| first.eq_ignore_ascii_case("Custom"))
        {
            return Ok(rest.split_whitespace().collect());
        }
        // The final Name= value owns the remaining text. Preserve its internal
        // whitespace and Unicode without interpreting names as more options.
        let mut arguments = Vec::with_capacity(4);
        for _ in 0..3 {
            let (token, remaining) = rest
                .split_once(char::is_whitespace)
                .ok_or(CommandError::Usage(USAGE))?;
            arguments.push(token);
            rest = remaining.trim_start();
        }
        arguments.push(rest);
        Ok(arguments)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Ok(format!(
                "Model units: {}; absolute tolerance: {}; relative tolerance: {}; angular tolerance: {} radians. Change with: {USAGE}",
                describe_units(document.units()),
                document.tolerance().absolute(),
                document.tolerance().relative(),
                document.tolerance().angular(),
            ));
        }
        let (units, option) = match arguments {
            [unit_name, option] => (
                parse_units(unit_name).ok_or(CommandError::Usage(USAGE))?,
                *option,
            ),
            [custom, factor, option, name] if custom.eq_ignore_ascii_case("Custom") => {
                let meters_per_unit = parse_finite_real(option_value(factor, "MetersPerUnit")?)?;
                let name = option_value(name, "Name")?;
                if name.trim().is_empty() {
                    return Err(CommandError::Usage(USAGE));
                }
                (
                    LengthUnitSystem::Custom {
                        name: name.to_owned(),
                        meters_per_unit,
                    },
                    *option,
                )
            }
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let value = option_value(option, "Scale")?;
        let rescale = if value.eq_ignore_ascii_case("Yes") {
            true
        } else if value.eq_ignore_ascii_case("No") {
            false
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        let previous = describe_units(document.units());
        let changed = document.set_units(units, rescale)?;
        Ok(if !changed {
            format!(
                "Model units unchanged: {}",
                describe_units(document.units())
            )
        } else {
            format!(
                "Model units: {previous} -> {}; Scale={}; numeric tolerances unchanged",
                describe_units(document.units()),
                if rescale { "Yes" } else { "No" },
            )
        })
    }
}

fn option_value<'a>(token: &'a str, expected: &str) -> Result<&'a str, CommandError> {
    let (name, value) = token.split_once('=').ok_or(CommandError::Usage(USAGE))?;
    if !name.eq_ignore_ascii_case(expected) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(value)
}

fn describe_units(units: &LengthUnitSystem) -> String {
    match units {
        LengthUnitSystem::Custom {
            name,
            meters_per_unit,
        } => {
            format!("{name} (Custom; {meters_per_unit} meters per unit)")
        }
        _ => units.name().to_owned(),
    }
}

pub(super) fn parse_units(input: &str) -> Option<LengthUnitSystem> {
    use LengthUnitSystem::*;
    let normalized = input.to_ascii_lowercase().replace("metre", "meter");
    Some(match normalized.as_str() {
        "none" | "unitless" => None,
        "microns" => Microns,
        "millimeters" | "mm" => Millimeters,
        "centimeters" | "cm" => Centimeters,
        "meters" | "m" => Meters,
        "kilometers" | "km" => Kilometers,
        "microinches" => Microinches,
        "mils" => Mils,
        "inches" | "in" => Inches,
        "feet" | "ft" => Feet,
        "miles" => Miles,
        "angstroms" => Angstroms,
        "nanometers" => Nanometers,
        "decimeters" => Decimeters,
        "dekameters" => Dekameters,
        "hectometers" => Hectometers,
        "megameters" => Megameters,
        "gigameters" => Gigameters,
        "yards" | "yd" => Yards,
        "printerpoints" => PrinterPoints,
        "printerpicas" => PrinterPicas,
        "nauticalmiles" => NauticalMiles,
        "astronomicalunits" => AstronomicalUnits,
        "lightyears" => LightYears,
        "parsecs" => Parsecs,
        "unset" => Unset,
        _ => return Option::None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_document::Geometry;
    use viboceros_geometry::{Point3, Tolerance};

    #[test]
    fn custom_units_preserve_names_and_physical_size_through_history_and_3dm() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        registry.execute(&mut document, "Point 1000,0,0").unwrap();
        let id = document.objects().next().unwrap().id();
        let name = "fixture  尺 = quarter metre";
        let command = format!("Units Custom MetersPerUnit=0.25 Scale=Yes Name={name}");
        registry.execute(&mut document, &command).unwrap();
        let custom = LengthUnitSystem::Custom {
            name: name.into(),
            meters_per_unit: 0.25,
        };
        assert_eq!(document.units(), &custom);
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(4.0, 0.0, 0.0).unwrap())
        );
        let report = registry.execute(&mut document, "Units").unwrap();
        assert!(report.contains(name));
        assert!(report.contains("0.25 meters per unit"));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("custom units.3dm");
        registry
            .execute(&mut document, &format!("Export3dm {}", path.display()))
            .unwrap();
        let stored = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
        assert_eq!(stored.units, custom);
        assert_eq!(
            stored.objects[0].geometry,
            viboceros_io::ThreeDmGeometry::Point(Point3::try_new(4.0, 0.0, 0.0).unwrap())
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.units(), &LengthUnitSystem::Millimeters);
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(1000.0, 0.0, 0.0).unwrap())
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.units(), &custom);
        registry
            .execute(&mut document, "Units Meters Scale=Yes")
            .unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap())
        );
    }

    #[test]
    fn custom_metadata_changes_noops_and_invalid_input_preserve_expected_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        registry.execute(&mut document, "Point 8,0,0").unwrap();
        let objects = document.objects().cloned().collect::<Vec<_>>();
        let command = "Units custom metersperunit=0.125 scale=no name=eighth metre";
        registry.execute(&mut document, command).unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
        registry.execute(&mut document, "Point 2,0,0").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        registry.execute(&mut document, command).unwrap();
        assert_eq!(format!("{document:?}"), before);
        for command in [
            "Units Custom",
            "Units Custom MetersPerUnit=1",
            "Units Custom MetersPerUnit=1 Scale=Yes",
            "Units Custom MetersPerUnit=1 Scale=Yes Name=",
            "Units Custom MetersPerUnit=1 Scale=Yes Name=   ",
            "Units Custom MetersPerUnit=1 Scale=Yes Name=a\0b",
            "Units Custom MetersPerUnit=0 Scale=Yes Name=bad",
            "Units Custom MetersPerUnit=-1 Scale=Yes Name=bad",
            "Units Custom MetersPerUnit=NaN Scale=No Name=bad",
            "Units Custom MetersPerUnit=inf Scale=No Name=bad",
            "Units Custom MetersPerUnit=1e309 Scale=No Name=bad",
            "Units Custom MetersPerUnit=1e-320 Scale=Yes Name=overflow",
            "Units Custom MetersPerUnit=1 Scale=Maybe Name=bad",
            "Units Custom Scale=Yes MetersPerUnit=1 Name=bad",
            "Units Custom MetersPerUnit=1 Scale=No bogus=name",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command:?}"
            );
            assert_eq!(format!("{document:?}"), before, "{command:?}");
        }
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 2);
    }

    #[test]
    fn unit_changes_are_explicit_atomic_and_undoable() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        let id = document
            .add_geometry(Geometry::Point(
                Point3::try_new(1000.0, 2000.0, 3000.0).unwrap(),
            ))
            .unwrap();
        let tolerance = document.tolerance();
        let original = document.object(id).unwrap().clone();
        registry
            .execute(&mut document, "Units Meters Scale=Yes")
            .unwrap();
        assert_eq!(document.units(), &LengthUnitSystem::Meters);
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap())
        );
        assert_eq!(document.tolerance(), tolerance);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.units(), &LengthUnitSystem::Millimeters);
        assert_eq!(document.object(id).unwrap(), &original);
        registry.execute(&mut document, "Redo").unwrap();
        let scaled = document.object(id).unwrap().clone();
        registry
            .execute(&mut document, "Units Inches Scale=No")
            .unwrap();
        assert_eq!(document.units(), &LengthUnitSystem::Inches);
        assert_eq!(document.object(id).unwrap(), &scaled);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.units(), &LengthUnitSystem::Meters);
    }

    #[test]
    fn queries_noops_and_invalid_input_preserve_redo_and_the_complete_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        assert!(
            registry
                .execute(&mut document, "Units")
                .unwrap()
                .contains("Model units:")
        );
        registry
            .execute(&mut document, "_Units mm scale=yes")
            .unwrap();
        assert_eq!(format!("{document:?}"), before);
        for command in [
            "Units Meters",
            "Units Meters Yes",
            "Units Meters Scale=Maybe",
            "Units bogus Scale=Yes",
            "Units Meters Scale=Yes extra",
            "Units Unset Scale=Yes",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
            assert_eq!(format!("{document:?}"), before, "{command}");
        }
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 1);
    }

    #[test]
    fn scaling_includes_hidden_and_locked_objects_without_changing_attributes() {
        use viboceros_document::ObjectAttributes;
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        let mut ids = Vec::new();
        let attributes = ObjectAttributes::on_layer(document.current_layer_id());
        for attributes in [
            attributes.clone(),
            attributes.clone().with_visibility(false),
            attributes.with_locked(true),
        ] {
            let id = document
                .add_geometry_with_attributes(
                    Geometry::Point(Point3::try_new(1000.0, 0.0, 0.0).unwrap()),
                    attributes.clone(),
                )
                .unwrap();
            ids.push((id, attributes));
        }
        registry
            .execute(&mut document, "Units m Scale=Yes")
            .unwrap();
        for (id, attributes) in ids {
            let object = document.object(id).unwrap();
            assert_eq!(object.attributes(), &attributes);
            assert_eq!(
                object.geometry(),
                &Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap())
            );
        }
    }

    #[test]
    fn failed_rescaling_preserves_geometry_units_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        for x in [1.0, f64::MAX] {
            document
                .add_geometry(Geometry::Point(Point3::try_new(x, 0.0, 0.0).unwrap()))
                .unwrap();
        }
        registry.execute(&mut document, "Point 2,3,4").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        assert!(
            registry
                .execute(&mut document, "Units Microns Scale=Yes")
                .is_err()
        );
        assert_eq!(format!("{document:?}"), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 3);
    }

    #[test]
    fn builtin_units_and_british_spellings_are_recognized() {
        for name in [
            "None",
            "Microns",
            "Millimeters",
            "Centimeters",
            "Meters",
            "Kilometers",
            "Microinches",
            "Mils",
            "Inches",
            "Feet",
            "Miles",
            "Angstroms",
            "Nanometers",
            "Decimeters",
            "Dekameters",
            "Hectometers",
            "Megameters",
            "Gigameters",
            "Yards",
            "PrinterPoints",
            "PrinterPicas",
            "NauticalMiles",
            "AstronomicalUnits",
            "LightYears",
            "Parsecs",
            "Unset",
        ] {
            assert!(parse_units(name).is_some(), "{name}");
        }
        assert_eq!(
            parse_units("MILLIMETRES"),
            Some(LengthUnitSystem::Millimeters)
        );
        assert_eq!(parse_units("Metres"), Some(LengthUnitSystem::Meters));
        assert_eq!(parse_units("Custom"), Option::None);
    }
}

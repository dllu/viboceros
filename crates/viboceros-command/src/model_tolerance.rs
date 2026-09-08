//! Explicit CLI access to validated document tolerances.

use crate::{Command, CommandError, parse_finite_real};
use viboceros_document::Document;
use viboceros_geometry::Tolerance;

const USAGE: &str =
    "Tolerance [Absolute=value] [Relative=value] [AngleDegrees=value|AngleRadians=value]";

pub(super) struct ToleranceCommand;

impl Command for ToleranceCommand {
    fn name(&self) -> &'static str {
        "Tolerance"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let old = document.tolerance();
        let mut values = [old.absolute(), old.relative(), old.angular()];
        let mut seen = [false; 3];
        for argument in arguments {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let (index, degrees) = if name.eq_ignore_ascii_case("Absolute") {
                (0, false)
            } else if name.eq_ignore_ascii_case("Relative") {
                (1, false)
            } else if name.eq_ignore_ascii_case("AngleDegrees") {
                (2, true)
            } else if name.eq_ignore_ascii_case("AngleRadians") {
                (2, false)
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if std::mem::replace(&mut seen[index], true) {
                return Err(CommandError::Usage(USAGE));
            }
            let value = parse_finite_real(value)?;
            values[index] = if degrees { value.to_radians() } else { value };
        }
        // Validate the complete proposal before recording any change. This also
        // catches a positive degree input that underflows during conversion.
        let tolerance = Tolerance::try_new(values[0], values[1], values[2])?;
        document.set_tolerance(tolerance);
        Ok(format!(
            "Tolerance: Absolute={} (model units), Relative={}, AngleRadians={}",
            tolerance.absolute(),
            tolerance.relative(),
            tolerance.angular()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;

    #[test]
    fn export_persists_tolerances_and_import_keeps_the_destination_policy() {
        let registry = CommandRegistry::with_builtins();
        let mut source = Document::new(Tolerance::DEFAULT);
        registry.execute(&mut source, "Point 1,2,3").unwrap();
        registry
            .execute(
                &mut source,
                "Tolerance Absolute=0.0125 Relative=0.00025 AngleDegrees=0.5",
            )
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("model tolerances.3dm");
        let before = format!("{source:?}");
        registry
            .execute(&mut source, &format!("Export3dm {}", path.display()))
            .unwrap();
        assert_eq!(format!("{source:?}"), before);
        let model = viboceros_io::read_3dm_file(&path, Tolerance::NUMERICAL_VALIDATION).unwrap();
        assert_eq!(model.tolerance, source.tolerance());
        let destination_policy = Tolerance::try_new(0.001, 1e-6, 0.01).unwrap();
        let mut destination = Document::new(destination_policy);
        registry
            .execute(&mut destination, &format!("Import3dm {}", path.display()))
            .unwrap();
        assert_eq!(destination.tolerance(), destination_policy);
        assert_eq!(destination.objects().len(), 1);
        registry.execute(&mut destination, "Undo").unwrap();
        assert_eq!(destination.tolerance(), destination_policy);
        registry.execute(&mut destination, "Redo").unwrap();
        assert_eq!(destination.tolerance(), destination_policy);
    }

    #[test]
    fn unencodable_tolerances_do_not_replace_an_existing_3dm_file() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preserved.3dm");
        let command = format!("Export3dm {}", path.display());
        registry.execute(&mut document, &command).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for tolerance in [
            Tolerance::try_new(0.01, 1.0, 0.01).unwrap(),
            Tolerance::try_new(0.01, 0.01, 4.0).unwrap(),
        ] {
            document.set_tolerance(tolerance);
            let before = format!("{document:?}");
            assert!(registry.execute(&mut document, &command).is_err());
            assert_eq!(format!("{document:?}"), before);
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn setting_all_fields_is_one_undo_step_and_partial_edits_keep_other_fields() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        registry
            .execute(
                &mut document,
                "Tolerance AngleDegrees=1 Relative=0.0001 Absolute=0.01",
            )
            .unwrap();
        let expected = Tolerance::try_new(0.01, 0.0001, 1.0_f64.to_radians()).unwrap();
        assert_eq!(document.tolerance(), expected);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.tolerance(), Tolerance::DEFAULT);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.tolerance(), expected);
        registry
            .execute(&mut document, "_Tolerance absolute=0.001")
            .unwrap();
        assert_eq!(
            document.tolerance(),
            Tolerance::try_new(0.001, expected.relative(), expected.angular()).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.tolerance(), expected);
    }

    #[test]
    fn queries_noops_and_invalid_proposals_preserve_document_and_redo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        assert!(
            registry
                .execute(&mut document, "Tolerance")
                .unwrap()
                .contains("AngleRadians=")
        );
        registry
            .execute(&mut document, "Tolerance Absolute=1e-9")
            .unwrap();
        assert_eq!(format!("{document:?}"), before);
        for command in [
            "Tolerance 0.01",
            "Tolerance Absolute=",
            "Tolerance Absolute=NaN",
            "Tolerance Relative=inf",
            "Tolerance Absolute=0.01 Relative=-1",
            "Tolerance AngleRadians=0",
            "Tolerance AngleDegrees=5e-324",
            "Tolerance Absolute=1 Absolute=2",
            "Tolerance AngleDegrees=1 AngleRadians=1",
            "Tolerance Angle=1",
            "Tolerance Relative=1e309",
            "Tolerance Absolute=1 Unknown=2",
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
    fn new_geometry_uses_changed_tolerance_without_modifying_existing_geometry() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        registry
            .execute(&mut document, "Line 0,0,0 0.005,0,0")
            .unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "Tolerance Absolute=0.01")
            .unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        assert!(
            registry
                .execute(&mut document, "Line 0,1,0 0.005,1,0")
                .is_err()
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        registry.execute(&mut document, "Undo").unwrap();
        registry
            .execute(&mut document, "Line 0,1,0 0.005,1,0")
            .unwrap();
        assert_eq!(document.objects().len(), 2);
    }
}

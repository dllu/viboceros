//! Four-point direction-angle query; no document or selection mutation.
use super::*;
use crate::parse_point;

pub(crate) struct AngleCommand;

impl Command for AngleCommand {
    fn name(&self) -> &'static str {
        "Angle"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, _document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (a, first) = parse_point(arguments)?;
        let (b, second) = parse_point(&arguments[first..])?;
        let (c, third) = parse_point(&arguments[first + second..])?;
        let (d, fourth) = parse_point(&arguments[first + second + third..])?;
        require_consumed(
            arguments,
            first + second + third + fourth,
            "Angle first_start first_end second_start second_end",
        )?;
        let angle = a.vector_to(b)?.angle_to(c.vector_to(d)?)?.to_degrees();
        Ok(format!("Angle = {} degrees", format_measurement(angle)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;

    #[test]
    fn four_point_angles_preserve_document_selection_and_redo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Point 4,5,6").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        for (input, expected) in [
            ("Angle 0,0 1,0 8,9 8,10", 90.),
            ("Angle 0,0 1,0 0,0 -1,0", 180.),
            ("Angle 0,0 1,0 0,0 1,0", 0.),
            ("Angle 0 0 0 0 0 1 0 0 0 1 0 1", 45.),
        ] {
            let output = registry.execute(&mut document, input).unwrap();
            let angle: f64 = output.split_whitespace().nth(2).unwrap().parse().unwrap();
            assert!((angle - expected).abs() < 1e-12);
            assert_eq!(format!("{document:?}"), before);
        }
        for input in [
            "Angle",
            "Angle 0,0 0,0 0,0 1,0",
            "Angle 0,0 1,0 0,0 0,0",
            "Angle 0,0 1,0 0,0 nan,0",
            "Angle 0,0 1,0 0,0 1,0 extra",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(format!("{document:?}"), before);
        }
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), 2);
    }
}

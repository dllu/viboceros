//! Four-point and selected-object angle queries; no document or selection mutation.
use super::*;
use crate::parse_point;
mod objects;

pub(crate) struct AngleCommand;

impl Command for AngleCommand {
    fn name(&self) -> &'static str {
        "Angle"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<crate::ObjectSelectionPrompt>, CommandError> {
        if matches!(arguments, [mode] if crate::option_name_eq(mode, "TwoObjects")) {
            return Ok(Some(crate::ObjectSelectionPrompt {
                command: "Angle TwoObjects",
                filter: crate::ObjectSelectionFilter::Beziers,
                options: vec![],
                menus: vec![],
                choices: vec![],
                workflow: crate::ObjectSelectionWorkflow::OptionsDuringSelection,
            }));
        }
        Ok(None)
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty()
            || matches!(arguments, [mode] if crate::option_name_eq(mode, "TwoObjects"))
        {
            return Ok(format!(
                "Angle = {} degrees",
                format_measurement(objects::measure(document)?)
            ));
        }
        let (a, first) = parse_point(arguments)?;
        let (b, second) = parse_point(&arguments[first..])?;
        let (c, third) = parse_point(&arguments[first + second..])?;
        let (d, fourth) = parse_point(&arguments[first + second + third..])?;
        require_consumed(
            arguments,
            first + second + third + fourth,
            "Angle first_start first_end second_start second_end",
        )?;
        let angle = a
            .direction_to(b)?
            .as_vector()
            .angle_to(c.direction_to(d)?.as_vector())?
            .to_degrees();
        Ok(format!("Angle = {} degrees", format_measurement(angle)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;

    #[test]
    fn four_point_angles_do_not_require_representable_endpoint_differences() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let before = format!("{document:?}");
        for (input, expected) in [
            ("Angle -1e308,0,0 1e308,0,0 0,-1e308,0 0,1e308,0", 90.),
            ("Angle -1e308,0,0 1e308,0,0 1e308,0,0 -1e308,0,0", 180.),
            ("Angle -1e308,-1e308,0 1e308,1e308,0 0,0,0 1,0,0", 45.),
            ("Angle 0,0,0 5e-324,0,0 0,0,0 0,5e-324,0", 90.),
        ] {
            let output = registry.execute(&mut document, input).unwrap();
            let angle: f64 = output.split_whitespace().nth(2).unwrap().parse().unwrap();
            assert!((angle - expected).abs() < 1e-12, "{output}");
            assert_eq!(format!("{document:?}"), before);
        }
    }

    #[test]
    fn four_point_reports_match_captured_rhino_angles() {
        // Rhino 8.32.26160.13001; docs/angle-rhino-reference.json.
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let before = format!("{document:?}");
        for (command, expected, rotated) in [
            ("Angle 0,0,0 1,0,0 4,5,6 5,5,6", 0.000, false),
            ("Angle 0,0,0 1,0,0 4,5,6 3,5,6", 180.000, false),
            ("Angle 0,0,0 1,0,0 4,5,6 4,6,6", 90.000, false),
            ("Angle 0,0,0 1,0,0 0,0,0 1,1,0", 45.000, false),
            ("Angle 0,0,0 1,0,0 0,0,0 -1,1,0", 135.000, false),
            ("Angle 0,0,0 0,0,1 0,0,0 1,1,1", 54.736, false),
            ("Angle 0,0,0 0,0,1 0,0,0 1,1,1", 54.736, true),
        ] {
            let mut context = crate::CommandContext::default();
            if rotated {
                context.construction_plane = viboceros_geometry::Frame3::try_from_directions(
                    viboceros_geometry::Point3::try_new(10., -20., 30.).unwrap(),
                    viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
                    viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap();
            }
            let output = registry
                .execute_in_context(&mut document, command, context)
                .unwrap();
            let actual: f64 = output.split_whitespace().nth(2).unwrap().parse().unwrap();
            assert!((actual - expected).abs() <= 0.0005, "{command}: {actual}");
            assert_eq!(format!("{document:?}"), before);
        }
    }

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

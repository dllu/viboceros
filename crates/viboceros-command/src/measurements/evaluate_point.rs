//! Read-only world/CPlane coordinate reporting.
use super::*;
use crate::{CommandContext, parse_point};

pub(crate) struct EvaluatePointCommand;

impl Command for EvaluatePointCommand {
    fn name(&self) -> &'static str {
        "EvaluatePt"
    }
    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        _document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let mut point = None;
        let mut index = 0;
        let mut label_seen = false;
        while index < arguments.len() {
            if let Some((key, value)) = arguments[index].split_once('=') {
                if label_seen
                    || !crate::option_name_eq(key, "Label")
                    || crate::parse_yes_no(value) != Some(false)
                {
                    return Err(CommandError::Usage(
                        "EvaluatePt point [Label=No]; labels are not implemented",
                    ));
                }
                label_seen = true;
                index += 1;
            } else if point.is_none() {
                let (value, consumed) = parse_point(&arguments[index..])?;
                point = Some(value);
                index += consumed;
            } else {
                return Err(CommandError::Usage("EvaluatePt point [Label=No]"));
            }
        }
        let point = point.ok_or(CommandError::Usage("EvaluatePt point [Label=No]"))?;
        let local = context.construction_plane.coordinates_of(point)?;
        let coordinates = |p: [f64; 3]| p.map(format_measurement).join(",");
        Ok(format!(
            "World coordinates = {}\nCPlane coordinates = {}",
            coordinates(point.to_array()),
            coordinates(local)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_geometry::{Frame3, Point3, Vector3};

    #[test]
    fn evaluate_point_reports_translated_rotated_coordinates_without_mutating_document() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Point 1,2,3").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry.execute(&mut doc, "Point 4,5,6").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        let before = format!("{doc:?}");
        let frame = Frame3::try_from_directions(
            Point3::try_new(10., 20., 30.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for input in [
            "EvaluatePt 13,24,35",
            "_EvaluatePt _Label=_No 13 24 35",
            "EvaluatePt 13,24,35 Label=No",
        ] {
            assert_eq!(
                registry
                    .execute_in_context(
                        &mut doc,
                        input,
                        CommandContext {
                            construction_plane: frame
                        }
                    )
                    .unwrap(),
                "World coordinates = 13,24,35\nCPlane coordinates = 4,5,3"
            );
            assert_eq!(format!("{doc:?}"), before);
        }
        for input in [
            "EvaluatePt",
            "EvaluatePt NaN,0,0",
            "EvaluatePt 0,0,0 extra",
            "EvaluatePt 0,0,0 Label=Yes",
            "EvaluatePt 0,0,0 Label=No Label=No",
        ] {
            assert!(registry.execute(&mut doc, input).is_err());
            assert_eq!(format!("{doc:?}"), before);
        }
    }

    #[test]
    fn evaluate_point_preserves_numeric_range_and_rejects_unrepresentable_local_coordinates() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        assert_eq!(
            registry
                .execute(&mut doc, "EvaluatePt -0,5e-324,1e308")
                .unwrap(),
            "World coordinates = 0,5e-324,1e308\nCPlane coordinates = 0,5e-324,1e308"
        );
        let context = CommandContext {
            construction_plane: CommandContext::default()
                .construction_plane
                .with_origin(Point3::try_new(-1e308, 0., 0.).unwrap()),
        };
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute_in_context(&mut doc, "EvaluatePt 1e308,0,0", context)
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
}

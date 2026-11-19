//! World and construction-plane coordinate reporting and annotation.
use super::*;
use crate::{CommandContext, parse_point};
use viboceros_geometry::{Frame3, GeometryError, Point3, TextDot};

const USAGE: &str = "EvaluatePt point [Label=On|Off] [Style=Dot] [CoordinateSystem=World|CPlane]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvaluatePointOptions {
    pub label: bool,
    cplane: bool,
}

impl EvaluatePointOptions {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        let mut options = Self::default();
        let mut seen = [false; 3];
        for argument in arguments {
            let key = options.set(argument)?;
            if std::mem::replace(&mut seen[key], true) {
                return Err(CommandError::Usage(USAGE));
            }
        }
        Ok(options)
    }

    pub fn with_option(mut self, argument: &str) -> Result<Self, CommandError> {
        self.set(argument)?;
        Ok(self)
    }

    fn set(&mut self, argument: &str) -> Result<usize, CommandError> {
        let (key, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let value = value.trim_start_matches('_');
        if crate::option_name_eq(key, "Label") {
            if value.eq_ignore_ascii_case("On") {
                self.label = true;
            } else if value.eq_ignore_ascii_case("Off") {
                self.label = false;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            Ok(0)
        } else if crate::option_name_eq(key, "Style") && value.eq_ignore_ascii_case("Dot") {
            Ok(1)
        } else if crate::option_name_eq(key, "CoordinateSystem") {
            if value.eq_ignore_ascii_case("CPlane") {
                self.cplane = true;
            } else if value.eq_ignore_ascii_case("World") {
                self.cplane = false;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            Ok(2)
        } else {
            Err(CommandError::Usage(USAGE))
        }
    }

    pub fn coordinate_system(self) -> &'static str {
        if self.cplane { "CPlane" } else { "World" }
    }

    pub fn preview_label(
        self,
        point: Point3,
        plane: Frame3,
        document: &Document,
    ) -> Option<TextDot> {
        if !self.label {
            return None;
        }
        let local = plane.coordinates_of(point).ok()?;
        self.create_label(point, local, document).ok()
    }

    fn create_label(
        self,
        point: Point3,
        local: [f64; 3],
        document: &Document,
    ) -> Result<TextDot, GeometryError> {
        let values = if self.cplane { local } else { point.to_array() };
        let meters_per_unit = document
            .units()
            .meters_per_unit()
            .map_err(|_| GeometryError::InvalidAnnotationCoordinates)?;
        let label = document
            .annotation_style()
            .format_coordinates(values, meters_per_unit)
            .ok_or(GeometryError::InvalidAnnotationCoordinates)?;
        TextDot::try_new(point, label)
    }
}

fn format_coordinates(values: [f64; 3]) -> String {
    values.map(format_measurement).join(",")
}

pub(crate) struct EvaluatePointCommand;

impl Command for EvaluatePointCommand {
    fn name(&self) -> &'static str {
        "EvaluatePt"
    }

    fn records_history_for(&self, arguments: &[&str]) -> bool {
        arguments.iter().any(|argument| {
            EvaluatePointOptions::default()
                .with_option(argument)
                .is_ok_and(|options| options.label)
        })
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let mut point = None;
        let mut index = 0;
        let mut options = Vec::new();
        while index < arguments.len() {
            if arguments[index].contains('=') {
                options.push(arguments[index]);
                index += 1;
            } else if point.is_none() {
                let (value, consumed) = parse_point(&arguments[index..])?;
                point = Some(value);
                index += consumed;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let point = point.ok_or(CommandError::Usage(USAGE))?;
        let options = EvaluatePointOptions::parse(&options)?;
        let local = context.construction_plane.coordinates_of(point)?;
        if options.label {
            let dot = options.create_label(point, local, document)?;
            document.add_geometry(Geometry::TextDot(dot))?;
        }
        Ok(format!(
            "World coordinates = {}\nCPlane coordinates = {}",
            format_coordinates(point.to_array()),
            format_coordinates(local)
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
            "_EvaluatePt _Label=_Off 13 24 35",
            "EvaluatePt 13,24,35 Label=Off",
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

        doc.begin_transaction("Caller edit").unwrap();
        assert!(registry.execute(&mut doc, "EvaluatePt 1,2,3").is_ok());
        assert!(!doc.commit_transaction().unwrap());
        for input in [
            "EvaluatePt",
            "EvaluatePt NaN,0,0",
            "EvaluatePt 0,0,0 extra",
            "EvaluatePt 0,0,0 Style=Leader",
            "EvaluatePt 0,0,0 Label=No",
            "EvaluatePt 0,0,0 Label=Off Label=Off",
        ] {
            assert!(registry.execute(&mut doc, input).is_err());
            assert_eq!(format!("{doc:?}"), before);
        }
    }

    #[test]
    fn evaluate_point_signed_plane_coordinates_match_live_rhino() {
        // Rhino 8.32.26160.13001: docs/evaluate-point-rhino-reference.json.
        let context = CommandContext {
            construction_plane: Frame3::try_from_directions(
                Point3::try_new(1., 2., 3.).unwrap(),
                Vector3::try_new(0., -1., 0.).unwrap(),
                Vector3::try_new(1., 0., 0.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        };
        let mut document = Document::default();
        let before = format!("{document:?}");
        assert_eq!(
            CommandRegistry::with_builtins()
                .execute_in_context(&mut document, "EvaluatePt -5,0,2 Label=Off", context)
                .unwrap(),
            "World coordinates = -5,0,2\nCPlane coordinates = 2,-6,-1"
        );
        assert_eq!(format!("{document:?}"), before);
    }

    #[test]
    fn evaluate_point_inline_label_defaults_to_world_and_undoes_as_one_edit() {
        let registry = CommandRegistry::with_builtins();
        let point = Point3::try_new(1.234, 2.129, -3.977).unwrap();
        let context = CommandContext {
            construction_plane: CommandContext::default()
                .construction_plane
                .with_origin(point),
        };
        let mut document = Document::default();
        let report = registry
            .execute_in_context(
                &mut document,
                "EvaluatePt Label=On 1.234,2.129,-3.977 Style=Dot",
                context,
            )
            .unwrap();
        assert_eq!(
            report,
            "World coordinates = 1.234,2.129,-3.977\nCPlane coordinates = 0,0,0"
        );
        let Geometry::TextDot(dot) = document.objects().next().unwrap().geometry() else {
            panic!("expected a text dot");
        };
        assert_eq!(dot.point(), point);
        assert_eq!(dot.text(), "1.23,2.13,-3.98");
        assert_eq!(document.undo_label(), Some("EvaluatePt"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 0);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), 1);
    }

    #[test]
    fn evaluate_point_uses_the_active_style_after_open_and_save() {
        // OpenNURBS fixture: millimetre model, decimal inches in the active
        // style, length factor two, 0.005 rounding, and three decimal places.
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../viboceros-io/tests/fixtures/evaluate-point-annotation-style.3dm");
        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("coordinates.3dm");
        let registry = CommandRegistry::with_builtins();
        for path in [&source, &saved] {
            let (mut document, _, _) =
                crate::open_3dm_with_named_views(path.to_str().unwrap()).unwrap();
            assert!(!document.annotation_style().archive().is_empty());
            assert_eq!(
                registry
                    .execute(&mut document, "EvaluatePt 25.4,-12.7,0.23 Label=On")
                    .unwrap(),
                "World coordinates = 25.4,-12.7,0.23\nCPlane coordinates = 25.4,-12.7,0.23"
            );
            let Geometry::TextDot(dot) = document.objects().last().unwrap().geometry() else {
                panic!("expected a text dot");
            };
            assert_eq!(dot.point().to_array(), [25.4, -12.7, 0.23]);
            assert_eq!(dot.text(), "2.000,-1.000,0.020");
            if path == &source {
                registry
                    .execute(&mut document, &format!("SaveAs \"{}\"", saved.display()))
                    .unwrap();
            }
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

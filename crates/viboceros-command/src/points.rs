//! Multiple independent point objects, staged before document mutation.
use super::*;

pub(super) struct PointsCommand;

impl Command for PointsCommand {
    fn name(&self) -> &'static str {
        "Points"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut points = Vec::new();
        let mut cursor = 0;
        while cursor < arguments.len() {
            if option_name_eq(arguments[cursor], "Undo") {
                points.pop();
                cursor += 1;
            } else {
                let (point, consumed) = parse_point(&arguments[cursor..])?;
                points.push(point);
                cursor += consumed;
            }
        }
        let count = points.len();
        for point in points {
            document.add_geometry(Geometry::Point(point))?;
        }
        Ok(format!("Created {count} point objects"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_points_stage_undo_and_preserve_duplicates_in_one_history_entry() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, "Points 1,2,3 4,5,6 Undo 1,2,3 7 8 9")
            .unwrap();
        let points = document
            .objects()
            .map(|o| {
                if let Geometry::Point(p) = o.geometry() {
                    p.to_array()
                } else {
                    panic!()
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(points, [[1., 2., 3.], [1., 2., 3.], [7., 8., 9.]]);
        assert_eq!(document.undo_label(), Some("Points"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 0);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().len(), 3);
    }

    #[test]
    fn empty_sequences_and_invalid_coordinates_preserve_redo() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Point 9,9,9").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        for input in ["Points", "Points Undo", "Points 1,2,3 Undo Undo"] {
            registry.execute(&mut document, input).unwrap();
            assert_eq!(document.objects().len(), 0);
            assert_eq!(document.redo_label(), Some("Point"));
        }
        for input in [
            "Points 1,2,3 4,5,NaN",
            "Points 1,2,3 bad",
            "Points 1,2,3 4 5",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            assert_eq!(document.objects().len(), 0);
            assert_eq!(document.redo_label(), Some("Point"));
        }
    }
}

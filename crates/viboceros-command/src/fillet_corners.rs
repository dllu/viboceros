//! Rounds corners of selected polylines and straight polycurves with exact arcs.

use super::*;

const USAGE: &str = "FilletCorners radius | FilletCorners Radius=radius";

pub(super) struct FilletCornersCommand;

impl Command for FilletCornersCommand {
    fn name(&self) -> &'static str {
        "FilletCorners"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [argument] = arguments else {
            return Err(CommandError::Usage(USAGE));
        };
        let value = if let Some((name, value)) = argument.split_once('=') {
            if !option_name_eq(name, "Radius") {
                return Err(CommandError::Usage(USAGE));
            }
            value
        } else {
            argument
        };
        let radius = parse_finite_real(value)?;
        let mut replacements = Vec::new();
        for object in document.selected_objects() {
            let curve = match object.geometry() {
                Geometry::Polyline(polyline) => {
                    polyline.try_fillet_corners(radius, document.tolerance())?
                }
                Geometry::PolyCurve(polycurve) => {
                    polycurve.try_fillet_corners(radius, document.tolerance())?
                }
                _ => return Err(CommandError::FilletCornersRequiresStraightCurves),
            };
            replacements.push((object.id(), Geometry::PolyCurve(curve)));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = document.replace_object_geometries(replacements)?;
        Ok(format!("Rounded corners of {count} curve(s)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CurveSegment3, PolyCurve3};

    #[test]
    fn rounds_selected_open_and_closed_polylines_in_one_undo_step() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Polyline 0,0,0 4,0,0 4,4,0")
            .unwrap();
        registry
            .execute(&mut document, "Polyline 10,0,0 14,0,0 14,4,0 10,4,0 10,0,0")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "FilletCorners Radius=0.5")
            .unwrap();
        assert!(
            document
                .selected_objects()
                .all(|object| matches!(object.geometry(), Geometry::PolyCurve(_)))
        );
        assert_eq!(document.undo_label(), Some("FilletCorners"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn invalid_or_oversized_radius_does_not_edit_any_selected_curve() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Polyline 0,0,0 1,0,0 1,1,0")
            .unwrap();
        registry.execute(&mut document, "SelLast").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        for command in [
            "FilletCorners 0",
            "FilletCorners 1.1",
            "FilletCorners Radius=No",
        ] {
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }

        registry.execute(&mut document, "Line 3,0,0 4,0,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(
            registry
                .execute(&mut document, "FilletCorners Radius=0.25")
                .is_err()
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn selected_straight_polycurve_is_replaced_and_undoable() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let source = PolyCurve3::try_new(vec![
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 0.), p(4., 0.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Polyline(
                Polyline3::try_new(vec![p(4., 0.), p(4., 4.), p(8., 4.)], Tolerance::DEFAULT)
                    .unwrap(),
            ),
        ])
        .unwrap();
        let id = document
            .add_geometry(Geometry::PolyCurve(source.clone()))
            .unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "FilletCorners 0.5")
            .unwrap();
        let Geometry::PolyCurve(result) = document.object(id).unwrap().geometry() else {
            panic!("a straight polycurve remains a polycurve")
        };
        assert_eq!(result.segments().len(), 5);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::PolyCurve(source)
        );
    }
}

//! Pointwise curve-radius queries sharing differential evaluation and markers.
use super::*;

pub(crate) struct RadiusCommand {
    pub diameter: bool,
}

impl Command for RadiusCommand {
    fn name(&self) -> &'static str {
        if self.diameter { "Diameter" } else { "Radius" }
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let usage = if self.diameter {
            "Diameter [MarkDiameter=Yes|No] point-on-selected-curve"
        } else {
            "Radius [MarkRadius=Yes|No] point-on-selected-curve"
        };
        let mark_option = if self.diameter {
            "MarkDiameter"
        } else {
            "MarkRadius"
        };
        let (mut cursor, mut mark, mut point) = (0, false, None);
        while cursor < arguments.len() {
            if let Some((name, value)) = arguments[cursor].split_once('=')
                && option_name_eq(name, mark_option)
            {
                mark = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
                cursor += 1;
            } else if point.is_none() {
                let (value, consumed) = parse_point(&arguments[cursor..])?;
                point = Some(value);
                cursor += consumed;
            } else {
                return Err(CommandError::Usage(usage));
            }
        }
        let point = point.ok_or(CommandError::Usage(usage))?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let tolerance = document.tolerance();
        let mut best = None;
        for object in document.selected_objects() {
            let curve = geometry_curve_ref(object.geometry()).ok_or(CommandError::Usage(usage))?;
            let parameter = curve.closest_parameter(point, tolerance)?;
            let distance = point.distance_to(curve.evaluate(parameter)?)?;
            if best.as_ref().is_none_or(|(d, _)| distance < *d) {
                best = Some((distance, Target::Curve(curve, parameter)));
            }
        }
        let evaluation = best.ok_or(CommandError::NoObjectsSelected)?.1.evaluate()?;
        let Evaluation::Curve { curvature, .. } = &evaluation else {
            unreachable!()
        };
        let magnitude = curvature.length()?;
        let length = |factor: f64| -> Result<String, CommandError> {
            if magnitude == 0. {
                return Ok("infinite".into());
            }
            let value = factor / magnitude;
            if !value.is_finite() {
                return Err(GeometryError::Degenerate {
                    context: "unrepresentable curvature radius",
                }
                .into());
            }
            Ok(value.to_string())
        };
        let report = format!("Radius = {}; Diameter = {}", length(1.)?, length(2.)?);
        if mark {
            // Construct all markers before changing the document. Registry
            // transactions also make insertion failures atomic.
            for marker in evaluation.markers(tolerance)? {
                document.add_geometry(marker)?;
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_and_diameter_measure_local_curvature_without_changing_history() {
        let registry = CommandRegistry::with_builtins();
        for (source, pick, radius) in [
            ("Circle 0,0,0 2", "2,0,0", "2"),
            ("Line 0,0,0 5,0,0", "2,0,0", "infinite"),
        ] {
            let mut doc = Document::default();
            registry.execute(&mut doc, source).unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
            registry.execute(&mut doc, "Point 9,9,9").unwrap();
            registry.execute(&mut doc, "Undo").unwrap();
            let before = format!("{doc:?}");
            for name in ["Radius", "Diameter"] {
                let report = registry
                    .execute(&mut doc, &format!("{name} {pick}"))
                    .unwrap();
                assert!(
                    report.starts_with(&format!("Radius = {radius};")),
                    "{report}"
                );
                assert_eq!(format!("{doc:?}"), before);
            }
        }
    }

    #[test]
    fn radius_is_local_on_ellipses_and_native_nurbs_and_chooses_nearest_curve() {
        let registry = CommandRegistry::with_builtins();
        for nurbs in [false, true] {
            let mut doc = Document::default();
            registry
                .execute(&mut doc, "Ellipse 0,0,0 4,0,0 0,2,0")
                .unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
            if nurbs {
                registry.execute(&mut doc, "ToNURBS").unwrap();
            }
            registry.execute(&mut doc, "Line 20,0,0 20,1,0").unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
            for (pick, expected) in [("4,0,0", 1.), ("0,2,0", 8.)] {
                let output = registry
                    .execute(&mut doc, &format!("Radius {pick}"))
                    .unwrap();
                let actual: f64 = output
                    .split_whitespace()
                    .nth(2)
                    .unwrap()
                    .trim_end_matches(';')
                    .parse()
                    .unwrap();
                assert!((actual - expected).abs() < 1e-10, "{output}");
            }
            assert_eq!(
                registry.execute(&mut doc, "Diameter 20,0.5,0").unwrap(),
                "Radius = infinite; Diameter = infinite"
            );
        }
    }

    #[test]
    fn radius_markers_are_atomic_and_undoable_and_invalid_inputs_preserve_redo() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry
            .execute(&mut doc, "Radius MarkRadius=Yes 2,0,0")
            .unwrap();
        assert_eq!(doc.objects().count(), 3);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), 1);
        let before = format!("{doc:?}");
        for input in [
            "Radius",
            "Radius MarkRadius=Maybe 2,0,0",
            "Radius 2,0,0 extra",
            "Diameter MarkRadius=Yes 2,0,0",
        ] {
            assert!(registry.execute(&mut doc, input).is_err());
            assert_eq!(format!("{doc:?}"), before);
        }
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().count(), 3);
    }
}

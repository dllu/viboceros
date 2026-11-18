//! Pointwise curve-radius queries sharing differential evaluation and markers.
use super::*;
use crate::measurements::measurement_display_scale;
use viboceros_document::HistoryGroup;
use viboceros_geometry::LengthUnitSystem;
mod units;
use units::DisplayUnits;
pub use units::radius_display_units;

pub(crate) struct RadiusCommand {
    pub diameter: bool,
}

pub struct RadiusMeasurement {
    evaluation: Evaluation,
    magnitude: f64,
}

impl RadiusMeasurement {
    pub fn evaluate(curve: CurveRef<'_>, parameter: f64) -> Result<Self, CommandError> {
        let evaluation = Target::Curve(curve, parameter).evaluate()?;
        let Evaluation::Curve { curvature, .. } = &evaluation else {
            unreachable!()
        };
        Ok(Self {
            magnitude: curvature.length()?,
            evaluation,
        })
    }

    pub fn nearest(
        document: &Document,
        point: Point3,
        restrict_to_selection: bool,
    ) -> Result<Self, CommandError> {
        let tolerance = document.tolerance();
        let mut best = None;
        let candidates = document
            .selected_objects()
            .take(if restrict_to_selection { usize::MAX } else { 0 })
            .chain(
                document
                    .selectable_objects()
                    .take(if restrict_to_selection { 0 } else { usize::MAX }),
            );
        for object in candidates {
            let Some(curve) = geometry_curve_ref(object.geometry()) else {
                if restrict_to_selection {
                    return Err(CommandError::Usage(
                        "Select only curves for a radius measurement",
                    ));
                }
                continue;
            };
            let parameter = curve.closest_parameter(point, tolerance)?;
            let distance = point.distance_to(curve.evaluate(parameter)?)?;
            if best.as_ref().is_none_or(|(d, _, _)| distance < *d) {
                best = Some((distance, curve, parameter));
            }
        }
        let (_, curve, parameter) = best.ok_or(CommandError::Usage(
            "Radius/Diameter requires an eligible curve",
        ))?;
        Self::evaluate(curve, parameter)
    }

    pub fn model_value(&self, diameter: bool) -> Result<Option<f64>, CommandError> {
        length(self.magnitude, diameter, 1.0)
    }

    pub fn display_value(
        &self,
        document: &Document,
        diameter: bool,
        units: Option<&str>,
    ) -> Result<String, CommandError> {
        let target = DisplayUnits::parse(units)?;
        let scale = target.scale(document)?;
        display_length(self.magnitude, diameter, scale, &target)
    }

    fn markers(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, CommandError> {
        Ok(self.evaluation.markers(tolerance)?)
    }

    pub fn finish(
        &self,
        document: &mut Document,
        diameter: bool,
        mark: bool,
        units: Option<&str>,
    ) -> Result<String, CommandError> {
        let target = DisplayUnits::parse(units)?;
        let scale = target.scale(document)?;
        let name = if diameter { "Diameter" } else { "Radius" };
        if mark {
            crate::run_command_transaction(document, name, |doc| {
                self.report_and_mark(doc, true, scale, &target)
            })
        } else {
            self.report_and_mark(document, false, scale, &target)
        }
    }

    pub fn finish_grouped(
        &self,
        document: &mut Document,
        group: &mut HistoryGroup,
        units: Option<&str>,
    ) -> Result<String, CommandError> {
        let target = DisplayUnits::parse(units)?;
        let scale = target.scale(document)?;
        document.begin_group_transaction(group)?;
        match self.report_and_mark(document, true, scale, &target) {
            Ok(report) => {
                if let Err(error) = document.commit_group_transaction(group) {
                    document.rollback_transaction()?;
                    return Err(error.into());
                }
                Ok(report)
            }
            Err(error) => {
                document.rollback_transaction()?;
                Err(error)
            }
        }
    }

    fn report_and_mark(
        &self,
        document: &mut Document,
        mark: bool,
        scale: f64,
        target: &DisplayUnits,
    ) -> Result<String, CommandError> {
        let report = curvature_report(self.magnitude, scale, target)?;
        if mark {
            for marker in self.markers(document.tolerance())? {
                document.add_geometry(marker)?;
            }
        }
        Ok(report)
    }
}

fn length(magnitude: f64, diameter: bool, scale: f64) -> Result<Option<f64>, CommandError> {
    if magnitude == 0.0 {
        return Ok(None);
    }
    let factor = if diameter { 2.0 } else { 1.0 };
    Ok(Some(viboceros_geometry::scaled_quotient(
        factor, scale, magnitude,
    )?))
}

fn display_length(
    magnitude: f64,
    diameter: bool,
    scale: f64,
    target: &DisplayUnits,
) -> Result<String, CommandError> {
    let Some(value) = length(magnitude, diameter, scale)? else {
        return Ok("infinite".into());
    };
    Ok(target.format(value))
}

fn curvature_report(
    magnitude: f64,
    scale: f64,
    target: &DisplayUnits,
) -> Result<String, CommandError> {
    Ok(format!(
        "Radius = {}; Diameter = {}",
        display_length(magnitude, false, scale, target)?,
        display_length(magnitude, true, scale, target)?
    ))
}

/// A single preselected circular curve can be measured without a pick.
pub fn preselected_circular_radius(document: &Document) -> Result<Option<f64>, CommandError> {
    let mut objects = document.selected_objects();
    let Some(object) = objects.next() else {
        return Ok(None);
    };
    if objects.next().is_some() {
        return Ok(None);
    }
    Ok(match object.geometry() {
        Geometry::Circle(circle) => Some(circle.radius()),
        Geometry::Arc(arc) => Some(arc.radius()),
        Geometry::NurbsCurve(curve) => curve.circular_radius(document.tolerance())?,
        Geometry::Ellipse(ellipse) if ellipse.radius_x() == ellipse.radius_y() => {
            Some(ellipse.radius_x())
        }
        _ => None,
    })
}

fn radius_report(radius: f64) -> Result<String, CommandError> {
    let diameter = 2. * radius;
    if !radius.is_finite() || !diameter.is_finite() {
        return Err(GeometryError::Degenerate {
            context: "unrepresentable curvature radius",
        }
        .into());
    }
    Ok(format!("Radius = {radius}; Diameter = {diameter}"))
}

impl Command for RadiusCommand {
    fn name(&self) -> &'static str {
        if self.diameter { "Diameter" } else { "Radius" }
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty()
            && let Some(radius) = preselected_circular_radius(document)?
        {
            return radius_report(radius);
        }
        let usage = if self.diameter {
            "Diameter [MarkDiameter=Yes|No] [Units=name] point-on-curve"
        } else {
            "Radius [MarkRadius=Yes|No] [Units=name] point-on-curve"
        };
        let mark_option = if self.diameter {
            "MarkDiameter"
        } else {
            "MarkRadius"
        };
        let (mut cursor, mut mark, mut point) = (0, false, None);
        let (mut units_seen, mut target) = (false, None);
        while cursor < arguments.len() {
            if let Some((name, value)) = arguments[cursor].split_once('=')
                && option_name_eq(name, mark_option)
            {
                mark = parse_yes_no(value).ok_or(CommandError::Usage(usage))?;
                cursor += 1;
            } else if option_name_eq(arguments[cursor], mark_option) {
                mark = !mark;
                cursor += 1;
            } else if arguments[cursor]
                .split_once('=')
                .is_some_and(|(name, _)| option_name_eq(name, "Units"))
                && !units_seen
            {
                target = arguments[cursor].split_once('=').map(|(_, units)| units);
                units_seen = true;
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
        let preselected = document.selected_object_count() != 0;
        if units_seen && preselected {
            return Err(CommandError::Usage(
                "Radius/Diameter Units is unavailable with preselected objects",
            ));
        }
        let target = DisplayUnits::parse(target)?;
        let scale = target.scale(document)?;
        RadiusMeasurement::nearest(document, point, preselected)?
            .report_and_mark(document, mark, scale, &target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_radius_recognizes_preselected_native_and_nurbs_circles_read_only() {
        let registry = CommandRegistry::with_builtins();
        for nurbs in [false, true] {
            let mut doc = Document::default();
            registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
            if nurbs {
                registry.execute(&mut doc, "ToNURBS").unwrap();
            }
            let before = format!("{doc:?}");
            for name in ["Radius", "Diameter"] {
                let report = registry.execute(&mut doc, name).unwrap();
                let radius: f64 = report
                    .split_whitespace()
                    .nth(2)
                    .unwrap()
                    .trim_end_matches(';')
                    .parse()
                    .unwrap();
                assert!((radius - 2.).abs() < 1e-10, "{report}");
                assert_eq!(format!("{doc:?}"), before);
            }
        }
    }

    #[test]
    fn unselected_radius_ignores_hidden_locked_and_noncurve_objects() {
        let registry = CommandRegistry::with_builtins();
        for exclusion in ["Hide", "Lock", "Layer Hide Excluded", "Layer Lock Excluded"] {
            let mut doc = Document::default();
            registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
            registry.execute(&mut doc, "Layer New Excluded").unwrap();
            registry
                .execute(&mut doc, "Layer Current Excluded")
                .unwrap();
            registry.execute(&mut doc, "Circle 0,0,0 1").unwrap();
            let excluded = doc.objects().last().unwrap().id();
            registry.execute(&mut doc, "Layer Current Default").unwrap();
            doc.select_object(excluded, viboceros_document::SelectionMode::Replace)
                .unwrap();
            registry.execute(&mut doc, exclusion).unwrap();
            registry.execute(&mut doc, "SelNone").unwrap();
            registry.execute(&mut doc, "Layer Current Default").unwrap();
            registry.execute(&mut doc, "Point 1,0,0").unwrap();
            let before = format!("{doc:?}");
            assert_eq!(
                registry.execute(&mut doc, "Radius 1,0,0").unwrap(),
                "Radius = 2; Diameter = 4"
            );
            assert_eq!(format!("{doc:?}"), before);
        }
    }

    #[test]
    fn explicit_radius_selection_limits_candidates_and_empty_search_is_read_only() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let empty = format!("{doc:?}");
        assert!(registry.execute(&mut doc, "Radius 0,0,0").is_err());
        assert_eq!(format!("{doc:?}"), empty);
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry.execute(&mut doc, "Circle 0,0,0 1").unwrap();
        let before = format!("{doc:?}");
        assert_eq!(
            registry.execute(&mut doc, "Diameter 1,0,0").unwrap(),
            "Radius = 2; Diameter = 4"
        );
        assert_eq!(format!("{doc:?}"), before);
        registry.execute(&mut doc, "SelNone").unwrap();
        assert_eq!(
            registry.execute(&mut doc, "Diameter 1,0,0").unwrap(),
            "Radius = 1; Diameter = 2"
        );
    }

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
            "Radius extra",
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

    #[test]
    fn radius_display_units_convert_reports_without_scaling_geometry_or_history() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Units Meters Scale=No").unwrap();
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        registry.execute(&mut doc, "Point 9,9,9").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        let before = format!("{doc:?}");
        for name in ["Radius", "Diameter"] {
            assert_eq!(
                registry
                    .execute(&mut doc, &format!("{name} Units=cm 2,0,0"))
                    .unwrap(),
                "Radius = 200 Centimetres; Diameter = 400 Centimetres"
            );
            assert_eq!(
                registry
                    .execute(&mut doc, &format!("{name} 2,0,0 Units=Model_Units"))
                    .unwrap(),
                "Radius = 2; Diameter = 4"
            );
            for input in [
                format!("{name} 2,0,0 Units=unknown"),
                format!("{name} 2,0,0 Units=cm Units=m"),
            ] {
                assert!(registry.execute(&mut doc, &input).is_err(), "{input}");
            }
            assert_eq!(format!("{doc:?}"), before);
        }
        registry.execute(&mut doc, "Line 20,0,0 20,1,0").unwrap();
        assert_eq!(
            registry
                .execute(&mut doc, "Radius 20,0.5,0 Units=cm")
                .unwrap(),
            "Radius = infinite; Diameter = infinite"
        );
        registry.execute(&mut doc, "SelAll").unwrap();
        let before_selected = format!("{doc:?}");
        assert!(registry.execute(&mut doc, "Radius 2,0,0 Units=cm").is_err());
        assert_eq!(format!("{doc:?}"), before_selected);
        registry.execute(&mut doc, "SelNone").unwrap();
        registry.execute(&mut doc, "Units None Scale=No").unwrap();
        let before_unitless = format!("{doc:?}");
        assert!(registry.execute(&mut doc, "Radius 2,0,0 Units=cm").is_err());
        assert_eq!(format!("{doc:?}"), before_unitless);
    }

    #[test]
    fn radius_units_can_represent_a_diameter_whose_model_unit_value_overflows() {
        let report = curvature_report(
            1e-308,
            1e-3,
            &DisplayUnits::Decimal(LengthUnitSystem::Kilometers),
        )
        .unwrap();
        let values = report
            .split([' ', ';'])
            .filter_map(|token| token.parse::<f64>().ok())
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2, "{report}");
        assert!((values[0] / 1e305 - 1.).abs() < 1e-12, "{report}");
        assert!((values[1] / 2e305 - 1.).abs() < 1e-12, "{report}");
    }
}

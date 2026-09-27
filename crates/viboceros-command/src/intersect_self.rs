//! Curve self-contacts, computed from distinct parameter intervals.

use super::*;

const USAGE: &str = "IntersectSelf";
const MAX_SELF_PAIRS: usize = 1_000_000;
const MAX_SELF_DEPTH: usize = 18;

pub(super) struct IntersectSelfCommand;

impl Command for IntersectSelfCommand {
    fn name(&self) -> &'static str {
        "IntersectSelf"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, USAGE)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, USAGE)?;
        let sources = document
            .selected_objects()
            .map(|object| {
                object
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage("IntersectSelf requires curves"))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut outputs = Vec::new();
        for curve in &sources {
            let mut contacts = SelfContacts::new(document.tolerance());
            contacts.search(curve)?;
            outputs.extend(contacts.output);
            if outputs.len() > MAX_INTERSECT_OUTPUTS {
                return Err(CommandError::TooManyIntersectOutputs {
                    maximum: MAX_INTERSECT_OUTPUTS,
                });
            }
        }
        let point_count = outputs
            .iter()
            .filter(|geometry| matches!(geometry, Geometry::Point(_)))
            .count();
        let overlap_count = outputs.len() - point_count;
        let mut selected_overlap_ids = Vec::with_capacity(overlap_count);
        for geometry in outputs {
            let is_overlap = matches!(geometry, Geometry::NurbsCurve(_));
            let id = document.add_geometry(geometry)?;
            if is_overlap {
                selected_overlap_ids.push(id);
            }
        }
        document.select_objects_direct(selected_overlap_ids, SelectionMode::Replace)?;
        Ok(match (point_count, overlap_count) {
            (0, 0) => "Found 0 self-intersections.".to_owned(),
            (points, 0) => format!(
                "Found {points} self-intersection point{}.",
                if points == 1 { "" } else { "s" }
            ),
            (0, overlaps) => format!(
                "Found {overlaps} self-intersection overlap{}.",
                if overlaps == 1 { "" } else { "s" }
            ),
            (points, overlaps) => format!(
                "Found {points} self-intersection point(s) and {overlaps} self-intersection overlap(s)."
            ),
        })
    }
}

struct SelfContacts {
    tolerance: Tolerance,
    output: Vec<Geometry>,
    pair_count: usize,
}

impl SelfContacts {
    fn new(tolerance: Tolerance) -> Self {
        Self {
            tolerance,
            output: Vec::new(),
            pair_count: 0,
        }
    }

    fn search(&mut self, curve: &NurbsCurve) -> Result<(), CommandError> {
        let spans = curve
            .spans()
            .map(|(start, end)| curve.try_trimmed(start..=end))
            .collect::<Result<Vec<_>, _>>()?;
        for (index, first) in spans.iter().enumerate() {
            for (later, second) in spans[index + 1..].iter().enumerate() {
                let second_index = index + later + 1;
                let junction = if second_index == index + 1 {
                    Some(first.evaluate(*first.domain().end())?)
                } else {
                    None
                };
                let seam = if index == 0 && second_index == spans.len() - 1 {
                    let start = first.evaluate(*first.domain().start())?;
                    let end = second.evaluate(*second.domain().end())?;
                    model_points_near(start, end, self.tolerance).then_some(start)
                } else {
                    None
                };
                self.compare(first, second, junction, seam)?;
            }
        }
        for span in &spans {
            self.within(span, 0, spans.len() == 1)?;
        }
        let overlaps = self
            .output
            .iter()
            .filter_map(|geometry| match geometry {
                Geometry::NurbsCurve(curve) => Some(curve),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut redundant_points = Vec::new();
        for (index, geometry) in self.output.iter().enumerate() {
            let Geometry::Point(point) = geometry else {
                continue;
            };
            for overlap in &overlaps {
                let parameter =
                    CurveRef::NurbsCurve(overlap).closest_parameter(*point, self.tolerance)?;
                if model_points_near(overlap.evaluate(parameter)?, *point, self.tolerance) {
                    redundant_points.push(index);
                    break;
                }
            }
        }
        for index in redundant_points.into_iter().rev() {
            self.output.remove(index);
        }
        Ok(())
    }

    fn within(
        &mut self,
        curve: &NurbsCurve,
        depth: usize,
        whole_curve: bool,
    ) -> Result<(), CommandError> {
        if is_projection_monotone(curve)? {
            return Ok(());
        }
        if depth == MAX_SELF_DEPTH {
            return Err(GeometryError::Degenerate {
                context: "self-intersection subdivision limit",
            }
            .into());
        }
        let domain = curve.domain();
        let middle = 0.5 * *domain.start() + 0.5 * *domain.end();
        if middle <= *domain.start() || middle >= *domain.end() {
            return Err(GeometryError::Degenerate {
                context: "self-intersection parameter resolution",
            }
            .into());
        }
        let (left, right) = curve.try_split(middle)?;
        let junction = left.evaluate(*left.domain().end())?;
        let start = left.evaluate(*left.domain().start())?;
        let end = right.evaluate(*right.domain().end())?;
        let seam = (whole_curve && model_points_near(start, end, self.tolerance)).then_some(start);
        self.compare(&left, &right, Some(junction), seam)?;
        self.within(&left, depth + 1, false)?;
        self.within(&right, depth + 1, false)
    }

    fn compare(
        &mut self,
        first: &NurbsCurve,
        second: &NurbsCurve,
        junction: Option<Point3>,
        seam: Option<Point3>,
    ) -> Result<(), CommandError> {
        self.pair_count += 1;
        if self.pair_count > MAX_SELF_PAIRS {
            return Err(CommandError::TooManyIntersectPairs {
                maximum: MAX_SELF_PAIRS,
            });
        }
        for event in first.intersection_events_with_curve(second, self.tolerance)? {
            let geometry = match event {
                CurveCurveIntersectionEvent::Point(contact) => {
                    let point = contact.point();
                    if junction
                        .is_some_and(|shared| model_points_near(point, shared, self.tolerance))
                        || seam
                            .is_some_and(|shared| model_points_near(point, shared, self.tolerance))
                    {
                        continue;
                    }
                    Geometry::Point(point)
                }
                CurveCurveIntersectionEvent::Overlap(overlap) => {
                    Geometry::NurbsCurve(first.try_trimmed(overlap.first_interval())?)
                }
            };
            if self
                .output
                .iter()
                .any(|existing| same_contact(existing, &geometry, self.tolerance))
            {
                continue;
            }
            if self.output.len() == MAX_INTERSECT_OUTPUTS {
                return Err(CommandError::TooManyIntersectOutputs {
                    maximum: MAX_INTERSECT_OUTPUTS,
                });
            }
            self.output.push(geometry);
        }
        Ok(())
    }
}

/// Same-sign rational weights and monotone controls along one projection make
/// the projected curve injective, so the span cannot cross itself.
fn is_projection_monotone(curve: &NurbsCurve) -> Result<bool, GeometryError> {
    let controls = curve.control_points();
    if controls.len() < 2 {
        return Ok(false);
    }
    let positive = controls[0].weight().is_sign_positive();
    if controls
        .iter()
        .any(|point| point.weight().is_sign_positive() != positive)
    {
        return Ok(false);
    }
    let axis = controls[0]
        .point()
        .vector_to(controls[controls.len() - 1].point())?;
    if axis.length()? == 0.0 {
        return Ok(false);
    }
    controls.windows(2).try_fold(true, |monotone, pair| {
        Ok(monotone && pair[0].point().vector_to(pair[1].point())?.dot(axis)? > 0.0)
    })
}

fn same_contact(first: &Geometry, second: &Geometry, tolerance: Tolerance) -> bool {
    match (first, second) {
        (Geometry::Point(a), Geometry::Point(b)) => model_points_near(*a, *b, tolerance),
        (Geometry::NurbsCurve(a), Geometry::NurbsCurve(b)) => {
            let endpoints = |curve: &NurbsCurve| {
                let domain = curve.domain();
                Some((
                    curve.evaluate(*domain.start()).ok()?,
                    curve.evaluate(*domain.end()).ok()?,
                ))
            };
            let (Some((a0, a1)), Some((b0, b1))) = (endpoints(a), endpoints(b)) else {
                return false;
            };
            let aligned =
                model_points_near(a0, b0, tolerance) && model_points_near(a1, b1, tolerance);
            let reversed =
                model_points_near(a0, b1, tolerance) && model_points_near(a1, b0, tolerance);
            if !aligned && !reversed {
                return false;
            }
            let sample = |curve: &NurbsCurve, fraction: Real| {
                let domain = curve.domain();
                curve
                    .evaluate((*domain.start()).mul_add(1.0 - fraction, *domain.end() * fraction))
                    .ok()
            };
            [0.25, 0.5, 0.75].into_iter().all(|fraction| {
                let other_fraction = if aligned { fraction } else { 1.0 - fraction };
                match (sample(a, fraction), sample(b, other_fraction)) {
                    (Some(left), Some(right)) => model_points_near(left, right, tolerance),
                    _ => false,
                }
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    #[test]
    fn crossing_closed_and_backtracking_polylines() {
        let registry = CommandRegistry::with_builtins();
        let cases = [
            (
                "Polyline 0,0,0 2,2,0 0,2,0 2,0,0",
                "Found 1 self-intersection point.",
                Some([1.0, 1.0, 0.0]),
            ),
            (
                "Polyline 0,0,0 2,0,0 2,2,0 0,2,0 0,0,0",
                "Found 0 self-intersections.",
                None,
            ),
            (
                "Polyline 0,0,0 2,0,0 1,0,0 1,2,0",
                "Found 1 self-intersection overlap.",
                None,
            ),
        ];
        for (construction, expected, point) in cases {
            let mut document = Document::default();
            registry.execute(&mut document, construction).unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
            assert_eq!(
                registry.execute(&mut document, "IntersectSelf").unwrap(),
                expected
            );
            if let Some(expected_point) = point {
                assert!(document.objects().any(|object| matches!(object.geometry(), Geometry::Point(actual) if actual.to_array() == expected_point)));
                assert_eq!(document.selected_object_count(), 0);
            } else if expected.contains("overlap") {
                assert_eq!(document.selected_object_count(), 1);
            }
        }
    }

    #[test]
    fn curved_single_span_crossing_and_simple_closed_seam() {
        let registry = CommandRegistry::with_builtins();
        for (controls, expected) in [
            (
                [
                    point(-1.0, 0.0),
                    point(3.0, 2.0),
                    point(-3.0, 2.0),
                    point(1.0, 0.0),
                ],
                "Found 1 self-intersection point.",
            ),
            (
                [
                    point(0.0, 0.0),
                    point(1.0, 2.0),
                    point(-1.0, 2.0),
                    point(0.0, 0.0),
                ],
                "Found 0 self-intersections.",
            ),
        ] {
            let mut document = Document::default();
            let curve =
                NurbsCurve::try_new(3, controls.to_vec(), vec![0., 0., 0., 0., 1., 1., 1., 1.])
                    .unwrap();
            let id = document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
            document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            assert_eq!(
                registry.execute(&mut document, "IntersectSelf").unwrap(),
                expected
            );
            if expected.contains("1 self-intersection point") {
                let marker = document
                    .objects()
                    .find_map(|object| match object.geometry() {
                        Geometry::Point(point) => Some(*point),
                        _ => None,
                    })
                    .unwrap();
                assert!(marker.distance_to(point(0.0, 0.6)).unwrap() < 1e-10);
            }
        }
        let mut document = Document::default();
        registry.execute(&mut document, "Circle 0,0,0 2").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        assert_eq!(
            registry.execute(&mut document, "IntersectSelf").unwrap(),
            "Found 0 self-intersections."
        );
    }
}

//! Exact extraction of selected polycurve and polyline segments.

use super::*;
use viboceros_geometry::{CurveSegment3, PolyCurve3};

const USAGE: &str =
    "ExtractSubCrv Segments=All|0,2,... [Copy=Yes|No] [Join=Yes|No] [OutputLayer=Current|Input]";

#[derive(Clone)]
enum SegmentChoice {
    All,
    Indices(Vec<usize>),
}

#[derive(Clone)]
struct Options {
    segments: SegmentChoice,
    copy: bool,
    join: bool,
    current_layer: bool,
}

struct Plan {
    source: ObjectId,
    extracted: Vec<Geometry>,
    remainder: Vec<Geometry>,
}

pub(super) struct ExtractSubcurveCommand;

impl Command for ExtractSubcurveCommand {
    fn name(&self) -> &'static str {
        "ExtractSubCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        let mut plans = Vec::new();
        let mut total_outputs = 0usize;
        for object in document.selected_objects() {
            let segments = match object.geometry() {
                Geometry::PolyCurve(curve) => curve
                    .segments()
                    .iter()
                    .enumerate()
                    .map(|(index, segment)| {
                        segment.try_reparameterized(curve.segment_domain(index)?)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?,
                Geometry::Polyline(curve) => curve.segments().map(CurveSegment3::Line).collect(),
                _ => return Err(CommandError::UnsupportedExtractSubcurveGeometry),
            };
            let count = segments.len();
            let selected = match &options.segments {
                SegmentChoice::All => (0..count).collect::<Vec<_>>(),
                SegmentChoice::Indices(indices) => {
                    if let Some(&index) = indices.iter().find(|&&index| index >= count) {
                        return Err(CommandError::ExtractSubcurveIndexOutOfRange {
                            index,
                            segment_count: count,
                        });
                    }
                    indices.clone()
                }
            };
            let mut mask = vec![false; count];
            for &index in &selected {
                mask[index] = true;
            }
            let closed = match object.geometry() {
                Geometry::PolyCurve(curve) => curve.is_closed()?,
                Geometry::Polyline(curve) => curve.is_closed(),
                _ => unreachable!(),
            };
            let extracted = if options.join {
                runs(&mask, closed)
                    .into_iter()
                    .map(|run| geometry_for_run(&segments, &run, document.tolerance()))
                    .collect::<Result<Vec<_>, CommandError>>()?
            } else {
                selected
                    .iter()
                    .map(|&index| Ok(Geometry::from(segments[index].clone().into_curve())))
                    .collect::<Result<Vec<_>, CommandError>>()?
            };
            let remainder = if options.copy {
                Vec::new()
            } else {
                let retained = mask.iter().map(|value| !*value).collect::<Vec<_>>();
                runs(&retained, closed)
                    .into_iter()
                    .map(|run| geometry_for_run(&segments, &run, document.tolerance()))
                    .collect::<Result<Vec<_>, CommandError>>()?
            };
            total_outputs = total_outputs
                .checked_add(extracted.len())
                .and_then(|count| count.checked_add(remainder.len().saturating_sub(1)))
                .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("ExtractSubCrv"))?;
            plans.push(Plan {
                source: object.id(),
                extracted,
                remainder,
            });
        }
        if plans.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let source_count = plans.len();
        let extracted_count = plans.iter().map(|plan| plan.extracted.len()).sum::<usize>();
        let mut pieces = Vec::with_capacity(total_outputs);
        for plan in &plans {
            pieces.extend(
                plan.extracted
                    .iter()
                    .cloned()
                    .map(|geometry| (plan.source, geometry)),
            );
        }
        for plan in &plans {
            pieces.extend(
                plan.remainder
                    .iter()
                    .skip(1)
                    .cloned()
                    .map(|geometry| (plan.source, geometry)),
            );
        }
        let copied = document.copy_object_pieces_into_source_groups(pieces)?;
        let results = copied[..extracted_count].to_vec();
        if options.current_layer {
            document.set_objects_layer(results.iter().copied(), document.current_layer_id())?;
        }
        if !options.copy {
            document.replace_object_geometries(plans.iter().filter_map(|plan| {
                plan.remainder
                    .first()
                    .cloned()
                    .map(|geometry| (plan.source, geometry))
            }))?;
            document.delete_objects(
                plans
                    .iter()
                    .filter(|plan| plan.remainder.is_empty())
                    .map(|plan| plan.source),
            )?;
        }
        document.select_command_results(results)?;
        Ok(format!(
            "Extracted {extracted_count} curve(s) from {source_count} object(s); source segments {}",
            if options.copy { "copied" } else { "removed" }
        ))
    }
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let mut segments = None;
    let mut copy = None;
    let mut join = None;
    let mut current_layer = None;
    for &argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        if key.eq_ignore_ascii_case("Segments") && segments.is_none() {
            if value.eq_ignore_ascii_case("All") {
                segments = Some(SegmentChoice::All);
            } else {
                let indices = value
                    .split(',')
                    .map(|part| {
                        part.parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let unique = indices.iter().copied().collect::<BTreeSet<_>>();
                if indices.is_empty() || unique.len() != indices.len() {
                    return Err(CommandError::Usage(USAGE));
                }
                segments = Some(SegmentChoice::Indices(indices));
            }
        } else if key.eq_ignore_ascii_case("Copy") && copy.is_none() {
            copy = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("Join") && join.is_none() {
            join = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if key.eq_ignore_ascii_case("OutputLayer") && current_layer.is_none() {
            current_layer = Some(if value.eq_ignore_ascii_case("Current") {
                true
            } else if value.eq_ignore_ascii_case("Input") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            });
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(Options {
        segments: segments.ok_or(CommandError::Usage(USAGE))?,
        copy: copy.unwrap_or(false),
        join: join.unwrap_or(false),
        current_layer: current_layer.unwrap_or(true),
    })
}

fn runs(mask: &[bool], closed: bool) -> Vec<Vec<usize>> {
    let mut groups = Vec::<Vec<usize>>::new();
    for (index, &selected) in mask.iter().enumerate() {
        if selected {
            if index > 0 && mask[index - 1] {
                groups
                    .last_mut()
                    .expect("preceding segment was selected")
                    .push(index);
            } else {
                groups.push(vec![index]);
            }
        }
    }
    if closed && groups.len() > 1 && mask[0] && mask[mask.len() - 1] {
        let first = groups.remove(0);
        let mut last = groups.pop().expect("multiple groups");
        last.extend(first);
        groups.insert(0, last);
    }
    groups
}

fn geometry_for_run(
    segments: &[CurveSegment3],
    indices: &[usize],
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    if indices.len() == 1 {
        return Ok(Geometry::from(segments[indices[0]].clone().into_curve()));
    }
    let selected = indices
        .iter()
        .map(|&index| segments[index].clone())
        .collect::<Vec<_>>();
    let mut parameters = Vec::with_capacity(selected.len() + 1);
    parameters.push(*selected[0].domain().start());
    for (position, segment) in selected.iter().enumerate() {
        let end = if position == 0 || indices[position] == indices[position - 1] + 1 {
            *segment.domain().end()
        } else {
            let previous = *parameters.last().expect("first parameter exists");
            previous + (*segment.domain().end() - *segment.domain().start())
        };
        parameters.push(end);
    }
    if selected
        .iter()
        .all(|segment| matches!(segment, CurveSegment3::Line(_)))
    {
        let mut vertices = Vec::with_capacity(selected.len() + 1);
        let first = &selected[0];
        vertices.push(first.evaluate(*first.domain().start())?);
        for segment in &selected {
            vertices.push(segment.evaluate(*segment.domain().end())?);
        }
        Ok(Geometry::Polyline(Polyline3::try_with_parameters(
            vertices, parameters, tolerance,
        )?))
    } else {
        Ok(Geometry::PolyCurve(PolyCurve3::try_with_segment_domains(
            selected, parameters,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn selected_polycurve(document: &mut Document) -> ObjectId {
        let vertices = [
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
            point(2.0, 1.0),
        ];
        let segments = vertices
            .windows(2)
            .map(|pair| {
                CurveSegment3::Line(
                    LineSegment::try_new(pair[0], pair[1], document.tolerance()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let curve =
            PolyCurve3::try_with_segment_domains(segments, vec![-6.0, -2.0, 4.0, 9.0]).unwrap();
        let id = document.add_geometry(Geometry::PolyCurve(curve)).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        id
    }

    #[test]
    fn extracts_exact_parent_domain_segments_in_requested_order_without_deleting_source() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_polycurve(&mut document);
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractSubCrv Segments=2,0 Copy=Yes OutputLayer=Input"
                )
                .unwrap(),
            "Extracted 2 curve(s) from 1 object(s); source segments copied"
        );
        let results = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(results.len(), 2);
        for (object, domain, start, end) in [
            (results[0], 4.0..=9.0, point(1.0, 1.0), point(2.0, 1.0)),
            (results[1], -6.0..=-2.0, point(0.0, 0.0), point(1.0, 0.0)),
        ] {
            let Geometry::Line(line) = object.geometry() else {
                panic!("native line expected")
            };
            assert_eq!(line.domain(), domain);
            assert_eq!(line.start(), start);
            assert_eq!(line.end(), end);
        }
        assert!(matches!(
            document.object(source).unwrap().geometry(),
            Geometry::PolyCurve(_)
        ));
        assert_eq!(document.undo_label(), Some("ExtractSubCrv"));
        document.undo().unwrap();
        assert_eq!(document.objects().count(), 1);
        assert!(matches!(
            document.object(source).unwrap().geometry(),
            Geometry::PolyCurve(_)
        ));
    }

    #[test]
    fn removing_disjoint_segments_splits_the_remainder_and_keeps_undo_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let polyline = Polyline3::try_with_parameters(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(2.0, 0.0),
                point(3.0, 0.0),
                point(4.0, 0.0),
            ],
            vec![10.0, 11.0, 13.0, 16.0, 20.0],
            document.tolerance(),
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Polyline(polyline)).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "ExtractSubCrv Segments=1,3 Copy=No Join=No")
                .unwrap(),
            "Extracted 2 curve(s) from 1 object(s); source segments removed"
        );
        assert_eq!(document.objects().count(), 4);
        let Geometry::Line(remainder) = document.object(source).unwrap().geometry() else {
            panic!("first retained run must keep source identity")
        };
        assert_eq!(remainder.domain(), 10.0..=11.0);
        let selected = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(selected.len(), 2);
        for (object, domain) in selected.into_iter().zip([11.0..=13.0, 16.0..=20.0]) {
            let Geometry::Line(line) = object.geometry() else {
                panic!("extracted line expected")
            };
            assert_eq!(line.domain(), domain);
        }
        document.undo().unwrap();
        assert_eq!(document.objects().count(), 1);
        assert!(matches!(
            document.object(source).unwrap().geometry(),
            Geometry::Polyline(_)
        ));
    }

    #[test]
    fn join_wraps_a_closed_polycurve_across_its_seam() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let vertices = [
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
            point(0.0, 1.0),
            point(0.0, 0.0),
        ];
        let segments = vertices
            .windows(2)
            .map(|pair| {
                CurveSegment3::Line(
                    LineSegment::try_new(pair[0], pair[1], document.tolerance()).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let source = document
            .add_geometry(Geometry::PolyCurve(PolyCurve3::try_new(segments).unwrap()))
            .unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractSubCrv Segments=3,0 Join=Yes Copy=Yes"
                )
                .unwrap(),
            "Extracted 1 curve(s) from 1 object(s); source segments copied"
        );
        let selected = document.selected_objects().collect::<Vec<_>>();
        let [result] = selected.as_slice() else {
            panic!("joined extraction should select one curve")
        };
        let Geometry::Polyline(curve) = result.geometry() else {
            panic!("joined straight segments should remain a polyline")
        };
        assert_eq!(
            curve.vertices(),
            &[point(0.0, 1.0), point(0.0, 0.0), point(1.0, 0.0)]
        );
        assert!(document.object(source).is_some());
    }

    #[test]
    fn joined_contiguous_segments_keep_exact_parent_breaks() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        selected_polycurve(&mut document);
        registry
            .execute(
                &mut document,
                "ExtractSubCrv Segments=0,1 Join=Yes Copy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        let [result] = selected.as_slice() else {
            panic!("one joined result expected")
        };
        let Geometry::Polyline(curve) = result.geometry() else {
            panic!("joined straight segments should remain a polyline")
        };
        assert_eq!(curve.parameters(), &[-6.0, -2.0, 4.0]);
        assert_eq!(
            curve.vertices(),
            &[point(0.0, 0.0), point(1.0, 0.0), point(1.0, 1.0)]
        );
    }

    #[test]
    fn joined_mixed_segments_keep_native_curves_and_parent_domains() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let line = LineSegment::try_new(point(0.0, 0.0), point(1.0, 0.0), tolerance).unwrap();
        let arc = CircularArc3::try_from_three_points(
            point(1.0, 0.0),
            point(1.5, 0.5),
            point(2.0, 0.0),
            tolerance,
        )
        .unwrap();
        let tail = LineSegment::try_new(point(2.0, 0.0), point(3.0, 0.0), tolerance).unwrap();
        let curve = PolyCurve3::try_with_segment_domains(
            vec![
                CurveSegment3::Line(line),
                CurveSegment3::Arc(arc),
                CurveSegment3::Line(tail),
            ],
            vec![-4.0, -1.0, 3.0, 8.0],
        )
        .unwrap();
        let source = document.add_geometry(Geometry::PolyCurve(curve)).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractSubCrv Segments=0,1 Join=Yes Copy=Yes",
            )
            .unwrap();
        let selected = document.selected_objects().collect::<Vec<_>>();
        let [result] = selected.as_slice() else {
            panic!("one joined result expected")
        };
        let Geometry::PolyCurve(extracted) = result.geometry() else {
            panic!("mixed segments should remain a polycurve")
        };
        assert!(matches!(
            extracted.segments(),
            [CurveSegment3::Line(_), CurveSegment3::Arc(_)]
        ));
        assert_eq!(extracted.segment_domain(0).unwrap(), -4.0..=-1.0);
        assert_eq!(extracted.segment_domain(1).unwrap(), -1.0..=3.0);
        assert!(document.object(source).is_some());
    }

    #[test]
    fn output_layer_choice_keeps_source_groups() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let current = document.current_layer_id();
        let source = selected_polycurve(&mut document);
        let input = document.add_layer("Input", ColorRgb::BLACK).unwrap();
        document.set_objects_layer([source], input).unwrap();
        let group = document.add_group(None, [source]).unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "ExtractSubCrv Segments=0 Copy=Yes")
            .unwrap();
        let current_result = document.selected_objects().next().unwrap();
        assert_eq!(current_result.attributes().layer_id(), current);
        assert!(current_result.group_ids().contains(&group));

        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                "ExtractSubCrv Segments=1 Copy=Yes OutputLayer=Input",
            )
            .unwrap();
        let input_result = document.selected_objects().next().unwrap();
        assert_eq!(input_result.attributes().layer_id(), input);
        assert!(input_result.group_ids().contains(&group));
    }

    #[test]
    fn invalid_selection_preserves_source_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = selected_polycurve(&mut document);
        let original_undo = document.undo_label().map(str::to_owned);
        for command in [
            "ExtractSubCrv Segments=0,0",
            "ExtractSubCrv Segments=3",
            "ExtractSubCrv Segments=0 Copy=Maybe",
        ] {
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(document.objects().count(), 1);
            assert!(matches!(
                document.object(source).unwrap().geometry(),
                Geometry::PolyCurve(_)
            ));
            assert_eq!(document.undo_label(), original_undo.as_deref());
        }
        let short = document
            .add_geometry(Geometry::Polyline(
                Polyline3::try_new(
                    vec![point(10.0, 0.0), point(11.0, 0.0)],
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([source, short], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        assert!(matches!(
            registry.execute(&mut document, "ExtractSubCrv Segments=2 Copy=No"),
            Err(CommandError::ExtractSubcurveIndexOutOfRange { .. })
        ));
        assert_eq!(document.objects().count(), 2);
        assert_eq!(document.undo_label(), before.as_deref());
        assert!(matches!(
            document.object(source).unwrap().geometry(),
            Geometry::PolyCurve(_)
        ));
        assert!(matches!(
            document.object(short).unwrap().geometry(),
            Geometry::Polyline(_)
        ));
    }
}

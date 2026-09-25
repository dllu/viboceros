//! Match the selected end of the first curve to the second curve.
use super::*;
use viboceros_geometry::{
    CurveBlendContinuity, CurveMatchPreserveEnd, CurveRef, try_average_match_curve_ends,
    try_match_curve_end,
};

const USAGE: &str = "Match [Pick1=x,y,z] [Pick2=x,y,z] [Continuity=Position|Tangency|Curvature] [PreserveOtherEnd=None|Position|Tangency|Curvature] [AverageCurves=Yes|No]";

pub(super) struct MatchCurveCommand;

struct MatchOptions {
    picks: [Option<Point3>; 2],
    continuity: CurveBlendContinuity,
    preserve: CurveMatchPreserveEnd,
    average: bool,
}

impl Command for MatchCurveCommand {
    fn name(&self) -> &'static str {
        "Match"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse_options(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_options(arguments)?;
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let [source_id, reference_id] = selected.as_slice() else {
            return Err(CommandError::MatchRequiresTwoOpenCurves);
        };
        let source = document
            .object(*source_id)
            .and_then(|object| object.geometry().curve_ref())
            .map(CurveRef::to_owned)
            .ok_or(CommandError::MatchRequiresTwoOpenCurves)?;
        let reference = document
            .object(*reference_id)
            .and_then(|object| object.geometry().curve_ref())
            .map(CurveRef::to_owned)
            .ok_or(CommandError::MatchRequiresTwoOpenCurves)?;
        if source.as_ref().is_closed()? || reference.as_ref().is_closed()? {
            return Err(CommandError::MatchRequiresTwoOpenCurves);
        }
        let (default_source, default_reference) = fillet::nearest_ends(&source, &reference)?;
        let source_end = pick_end(
            source.as_ref(),
            options.picks[0].unwrap_or(default_source),
            document.tolerance(),
        )?;
        let reference_end = pick_end(
            reference.as_ref(),
            options.picks[1].unwrap_or(default_reference),
            document.tolerance(),
        )?;
        if options.average {
            let (first, second) = try_average_match_curve_ends(
                &source,
                source_end,
                &reference,
                reference_end,
                options.continuity,
                options.preserve,
                document.tolerance(),
            )?;
            document.replace_object_geometries([
                (*source_id, Geometry::NurbsCurve(first)),
                (*reference_id, Geometry::NurbsCurve(second)),
            ])?;
            Ok("Averaged and matched curve ends".to_owned())
        } else {
            let matched = try_match_curve_end(
                &source,
                source_end,
                &reference,
                reference_end,
                options.continuity,
                options.preserve,
                document.tolerance(),
            )?;
            document.replace_object_geometries([(*source_id, Geometry::NurbsCurve(matched))])?;
            Ok("Matched curve end".to_owned())
        }
    }
}

fn parse_options(arguments: &[&str]) -> Result<MatchOptions, CommandError> {
    let mut picks = [None, None];
    let mut continuity = None;
    let mut preserve = None;
    let mut average = None;
    for argument in arguments {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "Pick1") || option_name_eq(name, "Pick2") {
            let index = if option_name_eq(name, "Pick1") { 0 } else { 1 };
            let (point, consumed) = parse_point(&[value])?;
            if consumed != 1 || picks[index].replace(point).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(name, "Continuity") {
            let value = match value.to_ascii_lowercase().as_str() {
                "position" => CurveBlendContinuity::Position,
                "tangency" => CurveBlendContinuity::Tangency,
                "curvature" => CurveBlendContinuity::Curvature,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            if continuity.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(name, "PreserveOtherEnd") {
            let value = match value.to_ascii_lowercase().as_str() {
                "none" => CurveMatchPreserveEnd::None,
                "position" => CurveMatchPreserveEnd::Position,
                "tangency" => CurveMatchPreserveEnd::Tangency,
                "curvature" => CurveMatchPreserveEnd::Curvature,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            if preserve.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(name, "AverageCurves") {
            let value = if value.eq_ignore_ascii_case("Yes") || value.eq_ignore_ascii_case("True") {
                true
            } else if value.eq_ignore_ascii_case("No") || value.eq_ignore_ascii_case("False") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if average.replace(value).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(MatchOptions {
        picks,
        continuity: continuity.unwrap_or(CurveBlendContinuity::Tangency),
        preserve: preserve.unwrap_or(CurveMatchPreserveEnd::Position),
        average: average.unwrap_or(false),
    })
}

fn pick_end(curve: CurveRef<'_>, pick: Point3, tolerance: Tolerance) -> Result<bool, CommandError> {
    let start_distance = pick.distance_to(curve.start_point()?)?;
    let end_distance = pick.distance_to(curve.end_point()?)?;
    if (start_distance - end_distance).abs() <= tolerance.absolute() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(end_distance < start_distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CurveContinuityLevel, curve_end_continuity};

    #[test]
    fn match_replaces_first_selected_curve_and_undo_restores_it() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,3,0").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids.clone(), SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "Match Continuity=Tangency PreserveOtherEnd=Position"
                )
                .unwrap(),
            "Matched curve end"
        );
        let source = document
            .object(ids[0])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        let reference = document
            .object(ids[1])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert_eq!(
            curve_end_continuity(source, true, reference, false, document.tolerance())
                .unwrap()
                .level,
            CurveContinuityLevel::Tangency
        );
        assert_eq!(source.start_point().unwrap().to_array(), [0.0, 0.0, 0.0]);
        document.undo().unwrap();
        assert!(matches!(
            document.object(ids[0]).unwrap().geometry(),
            Geometry::Line(_)
        ));
    }

    #[test]
    fn invalid_options_and_selection_leave_document_unchanged() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
        let id = document.objects().next().unwrap().id();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        assert!(registry.execute(&mut document, "Match").is_err());
        assert!(
            registry
                .execute(&mut document, "Match Continuity=Bad")
                .is_err()
        );
        assert!(matches!(
            document.object(id).unwrap().geometry(),
            Geometry::Line(_)
        ));
    }

    #[test]
    fn averaging_replaces_both_curves_in_one_undoable_step() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,3,0").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids.clone(), SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "Match Continuity=Tangency AverageCurves=Yes PreserveOtherEnd=Position"
                )
                .unwrap(),
            "Averaged and matched curve ends"
        );
        let first = document
            .object(ids[0])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        let second = document
            .object(ids[1])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert_eq!(first.end_point().unwrap().to_array(), [3.5, 0.5, 0.0]);
        assert_eq!(second.start_point().unwrap().to_array(), [3.5, 0.5, 0.0]);
        assert_eq!(
            curve_end_continuity(first, true, second, false, document.tolerance())
                .unwrap()
                .level,
            CurveContinuityLevel::Tangency
        );
        document.undo().unwrap();
        for id in ids {
            assert!(matches!(
                document.object(id).unwrap().geometry(),
                Geometry::Line(_)
            ));
        }
    }

    #[test]
    fn average_curvature_matches_an_arc_and_preserves_source_far_end() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source_id = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(3.0, 0.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let reference_id = document
            .add_geometry(Geometry::Arc(
                CircularArc3::try_from_three_points(
                    Point3::try_new(4.0, 1.0, 0.0).unwrap(),
                    Point3::try_new(5.0, 2.0, 0.0).unwrap(),
                    Point3::try_new(6.0, 1.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([source_id, reference_id], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Match Continuity=Curvature AverageCurves=Yes PreserveOtherEnd=Position Pick1=0,0,0 Pick2=4,1,0").unwrap();
        let first = document
            .object(source_id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        let second = document
            .object(reference_id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert_eq!(
            curve_end_continuity(first, false, second, false, document.tolerance())
                .unwrap()
                .level,
            CurveContinuityLevel::CurvatureOrHigher
        );
        assert_eq!(first.end_point().unwrap().to_array(), [3.0, 0.0, 0.0]);
        document.undo().unwrap();
        assert!(matches!(
            document.object(source_id).unwrap().geometry(),
            Geometry::Line(_)
        ));
        assert!(matches!(
            document.object(reference_id).unwrap().geometry(),
            Geometry::Arc(_)
        ));
    }

    #[test]
    fn average_position_trims_toward_midpoint_and_undo_restores_both() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
        registry.execute(&mut document, "Line 4,1,0 4,3,0").unwrap();
        let ids = document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids.clone(), SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document,
            "Match Continuity=Position AverageCurves=Yes PreserveOtherEnd=Position Pick1=0,0,0 Pick2=4,1,0"
        ).unwrap();
        let Geometry::NurbsCurve(first) = document.object(ids[0]).unwrap().geometry() else {
            panic!("first curve is NURBS");
        };
        assert_eq!(
            first.control_points()[0].point().to_array(),
            [2.0, 0.5, 0.0]
        );
        assert_eq!(
            first.control_points()[1].point().to_array(),
            [3.0, 0.0, 0.0]
        );
        let second = document
            .object(ids[1])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert_eq!(second.start_point().unwrap().to_array(), [2.0, 0.5, 0.0]);
        document.undo().unwrap();
        for id in ids {
            assert!(matches!(
                document.object(id).unwrap().geometry(),
                Geometry::Line(_)
            ));
        }
    }
}

//! Read-only endpoint continuity measurement for two selected curves.

use super::*;
use viboceros_geometry::{CurveContinuityLevel, curve_end_continuity};

const USAGE: &str = "GCon [Pick1=x,y,z] [Pick2=x,y,z]";

pub(super) struct GeometricContinuityCommand;

impl Command for GeometricContinuityCommand {
    fn name(&self) -> &'static str {
        "GCon"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse_picks(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "GCon",
            filter: ObjectSelectionFilter::Curves,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let picks = parse_picks(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| object.geometry().curve_ref().map(|curve| curve.to_owned()))
            .collect::<Option<Vec<_>>>()
            .ok_or(CommandError::GConRequiresTwoOpenCurves)?;
        let [first, second] = selected.as_slice() else {
            return Err(CommandError::GConRequiresTwoOpenCurves);
        };
        if first.as_ref().is_closed()? || second.as_ref().is_closed()? {
            return Err(CommandError::GConRequiresTwoOpenCurves);
        }
        let (nearest_first, nearest_second) = super::fillet::nearest_ends(first, second)?;
        let first_end = picked_end(
            first.as_ref(),
            picks[0].unwrap_or(nearest_first),
            document.tolerance(),
        )?;
        let second_end = picked_end(
            second.as_ref(),
            picks[1].unwrap_or(nearest_second),
            document.tolerance(),
        )?;
        let report = curve_end_continuity(
            first.as_ref(),
            first_end,
            second.as_ref(),
            second_end,
            document.tolerance(),
        )?;
        let level = match report.level {
            CurveContinuityLevel::Disconnected => "disconnected",
            CurveContinuityLevel::Position => "G0",
            CurveContinuityLevel::Tangency => "G1",
            CurveContinuityLevel::CurvatureOrHigher => "G2+",
        };
        Ok(format!(
            "GCon: {level}; gap={:.12}; tangent_angle={:.12}deg; curvature_delta={:.12}",
            report.gap,
            report.tangent_angle_radians.to_degrees(),
            report.curvature_vector_difference,
        ))
    }
}

fn parse_picks(arguments: &[&str]) -> Result<[Option<Point3>; 2], CommandError> {
    let mut picks = [None, None];
    for argument in arguments {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let index = if option_name_eq(name, "Pick1") {
            0
        } else if option_name_eq(name, "Pick2") {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if picks[index].is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let (point, consumed) = parse_point(&[value])?;
        if consumed != 1 {
            return Err(CommandError::Usage(USAGE));
        }
        picks[index] = Some(point);
    }
    Ok(picks)
}

fn picked_end(
    curve: CurveRef<'_>,
    pick: Point3,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    let start = curve.start_point()?;
    let end = curve.end_point()?;
    let start_distance = pick.distance_to(start)?;
    let end_distance = pick.distance_to(end)?;
    if (start_distance - end_distance).abs() <= tolerance.absolute() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(end_distance < start_distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start: [Real; 3], end: [Real; 3]) -> Geometry {
        Geometry::Line(
            LineSegment::try_new(
                Point3::try_from(start).unwrap(),
                Point3::try_from(end).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        )
    }

    #[test]
    fn reports_g2_for_collinear_lines_without_mutating_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        let first = document
            .add_geometry(line([0., 0., 0.], [1., 0., 0.]))
            .unwrap();
        let second = document
            .add_geometry(line([1., 0., 0.], [2., 0., 0.]))
            .unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        let before = document.object(first).unwrap().geometry().clone();
        assert_eq!(
            registry.execute(&mut document, "GCon").unwrap(),
            "GCon: G2+; gap=0.000000000000; tangent_angle=0.000000000000deg; curvature_delta=0.000000000000"
        );
        assert_eq!(document.object(first).unwrap().geometry(), &before);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_ne!(document.undo_label(), Some("GCon"));
    }

    #[test]
    fn picks_other_ends_and_reports_disconnected_or_g0() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::new(Tolerance::DEFAULT);
        let first = document
            .add_geometry(line([0., 0., 0.], [1., 0., 0.]))
            .unwrap();
        let second = document
            .add_geometry(line([1., 0., 0.], [1., 1., 0.]))
            .unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        assert!(
            registry
                .execute(&mut document, "GCon")
                .unwrap()
                .contains("G0")
        );
        assert!(
            registry
                .execute(&mut document, "GCon Pick1=0,0,0 Pick2=1,1,0")
                .unwrap()
                .contains("disconnected")
        );
        assert!(
            registry
                .execute(&mut document, "GCon Pick1=0,0,0 Pick1=1,0,0")
                .is_err()
        );
    }
}

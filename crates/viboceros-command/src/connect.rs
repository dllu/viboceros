//! Extends or trims selected curve ends until they meet.

use super::*;
use viboceros_geometry::{
    CurveArcExtensionStyle, CurveOtherExtensionStyle, try_connect_curves_joined_with_styles,
    try_connect_curves_parts_with_styles,
};

const USAGE: &str = "Connect [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [ExtendArcsBy=Arc|Line] [ExtendOtherCurvesBy=Line|Smooth]";

pub(super) struct ConnectCommand;

struct ConnectOptions {
    picks: [Option<Point3>; 2],
    join: bool,
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
}

impl Command for ConnectCommand {
    fn name(&self) -> &'static str {
        "Connect"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let ConnectOptions {
            picks,
            join,
            arc_extension,
            other_extension,
        } = parse(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| {
                Ok((
                    object.id(),
                    object
                        .geometry()
                        .curve_ref()
                        .ok_or(CommandError::ConnectRequiresTwoCurves)?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let [(first_id, first), (second_id, second)] = selected.as_slice() else {
            return Err(CommandError::ConnectRequiresTwoCurves);
        };
        let (default_first, default_second) = super::fillet::nearest_ends(first, second)?;
        let picks = [
            picks[0].unwrap_or(default_first),
            picks[1].unwrap_or(default_second),
        ];
        let outputs = if join {
            let joined = try_connect_curves_joined_with_styles(
                first,
                picks[0],
                second,
                picks[1],
                arc_extension,
                other_extension,
                document.tolerance(),
            )?;
            document
                .copy_object_pieces_into_source_groups([(*first_id, Geometry::PolyCurve(joined))])?
        } else {
            let parts = try_connect_curves_parts_with_styles(
                first,
                picks[0],
                second,
                picks[1],
                arc_extension,
                other_extension,
                document.tolerance(),
            )?;
            document.copy_object_pieces_into_source_groups(
                [*first_id, *second_id]
                    .into_iter()
                    .zip(parts.into_iter().map(Geometry::from)),
            )?
        };
        document.delete_objects([*first_id, *second_id])?;
        document.select_objects_direct(outputs, SelectionMode::Replace)?;
        Ok("Connected two curves".to_string())
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

fn parse(arguments: &[&str]) -> Result<ConnectOptions, CommandError> {
    let mut picks = [None, None];
    let mut join = None;
    let mut arc_extension = None;
    let mut other_extension = None;
    for argument in arguments {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "ExtendOtherCurvesBy") {
            let style = if value.eq_ignore_ascii_case("Line") {
                CurveOtherExtensionStyle::Line
            } else if value.eq_ignore_ascii_case("Smooth") {
                CurveOtherExtensionStyle::Smooth
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if other_extension.replace(style).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        if option_name_eq(name, "ExtendArcsBy") {
            let style = if value.eq_ignore_ascii_case("Arc") {
                CurveArcExtensionStyle::Arc
            } else if value.eq_ignore_ascii_case("Line") {
                CurveArcExtensionStyle::Line
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if arc_extension.replace(style).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        if option_name_eq(name, "Join") {
            if join
                .replace(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                .is_some()
            {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
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
        let (pick, consumed) = parse_point(&[value])?;
        if consumed != 1 {
            return Err(CommandError::Usage(USAGE));
        }
        picks[index] = Some(pick);
    }
    Ok(ConnectOptions {
        picks,
        join: join.unwrap_or(false),
        arc_extension: arc_extension.unwrap_or(CurveArcExtensionStyle::Arc),
        other_extension: other_extension.unwrap_or(CurveOtherExtensionStyle::Line),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CircularArc3, CurveSegment3, LineSegment, NurbsCurve};

    #[test]
    fn connects_extended_lines_with_or_without_join_and_undo() {
        let registry = CommandRegistry::with_builtins();
        for (command, expected_count) in [("Connect", 2), ("Connect Join=Yes", 1)] {
            let mut document = Document::default();
            registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
            registry.execute(&mut document, "Line 4,1,0 4,4,0").unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
            let before = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, command).unwrap();
            assert_eq!(document.objects().count(), expected_count);
            if expected_count == 2 {
                assert!(
                    document
                        .objects()
                        .all(|object| matches!(object.geometry(), Geometry::Line(_)))
                );
            } else {
                assert!(matches!(
                    document.objects().next().unwrap().geometry(),
                    Geometry::PolyCurve(_)
                ));
            }
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }

    #[test]
    fn already_meeting_arc_and_line_keep_native_geometry() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let arc = CircularArc3::try_from_three_points(
            p(1., 0.),
            p(diagonal, diagonal),
            p(0., 1.),
            document.tolerance(),
        )
        .unwrap();
        let line = LineSegment::try_new(p(0., 1.), p(-2., 1.), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let second = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Connect").unwrap();
        assert_eq!(document.objects().count(), 2);
        assert!(
            document
                .objects()
                .any(|object| matches!(object.geometry(), Geometry::Arc(_)))
        );
        assert!(
            document
                .objects()
                .any(|object| matches!(object.geometry(), Geometry::Line(_)))
        );
        assert!(document.object(first).is_none() && document.object(second).is_none());
    }

    #[test]
    fn connects_nurbs_endpoint_with_line_or_joined_smooth_extension() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let nurbs = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., 0.), p(2., 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let line = LineSegment::try_new(p(3., 3.), p(3., 4.), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::NurbsCurve(nurbs)).unwrap();
        let second = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "Connect Join=Yes ExtendOtherCurvesBy=Line")
            .unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined NURBS connection");
        };
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::NurbsCurve(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_)
            ]
        ));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        registry
            .execute(&mut document, "Connect ExtendOtherCurvesBy=Smooth Join=Yes")
            .unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined smooth connection");
        };
        assert_eq!(joined.segments().len(), 2);
    }

    #[test]
    fn smooth_option_extends_nurbs_shape_and_preserves_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let nurbs = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., 0.), p(2., 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let line = LineSegment::try_new(p(3., 3.), p(3., 4.), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::NurbsCurve(nurbs)).unwrap();
        let second = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "Connect ExtendOtherCurvesBy=Smooth")
            .unwrap();
        let curves = document.objects().collect::<Vec<_>>();
        assert_eq!(curves.len(), 2);
        let meeting = curves[0]
            .geometry()
            .curve_ref()
            .unwrap()
            .end_point()
            .unwrap();
        assert!(meeting.distance_to(p(3., 2.25)).unwrap() < 1e-10);
        assert!(
            curves[1]
                .geometry()
                .curve_ref()
                .unwrap()
                .start_point()
                .unwrap()
                .distance_to(meeting)
                .unwrap()
                < 1e-10
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn smooth_option_connects_two_curved_nurbs_ends() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let first = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(1., 0.), p(2., 1.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let second = NurbsCurve::try_new(
            2,
            vec![p(3., 3.), p(3., 4.), p(3.2, 5.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let first_id = document.add_geometry(Geometry::NurbsCurve(first)).unwrap();
        let second_id = document.add_geometry(Geometry::NurbsCurve(second)).unwrap();
        document
            .select_objects_direct([first_id, second_id], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "Connect ExtendOtherCurvesBy=Smooth")
            .unwrap();
        let curves = document.objects().collect::<Vec<_>>();
        assert_eq!(curves.len(), 2);
        let meeting = p(3.02533554003123, 2.28816378244402);
        assert!(
            curves[0]
                .geometry()
                .curve_ref()
                .unwrap()
                .end_point()
                .unwrap()
                .distance_to(meeting)
                .unwrap()
                < 1e-10
        );
        assert!(
            curves[1]
                .geometry()
                .curve_ref()
                .unwrap()
                .start_point()
                .unwrap()
                .distance_to(meeting)
                .unwrap()
                < 1e-10
        );
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn extend_arcs_by_line_uses_the_endpoint_tangent() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let arc = CircularArc3::try_from_three_points(
            p(1., 0.),
            p(diagonal, diagonal),
            p(0., 1.),
            document.tolerance(),
        )
        .unwrap();
        let line = LineSegment::try_new(p(-1., 2.), p(-1., 3.), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let second = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "Connect Join=Yes ExtendArcsBy=Line")
            .unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined arc connection")
        };
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_)
            ]
        ));
    }

    #[test]
    fn default_arc_extension_connects_two_native_arcs() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let first = CircularArc3::try_from_three_points(
            p(1., 0.),
            p(diagonal, diagonal),
            p(0., 1.),
            document.tolerance(),
        )
        .unwrap();
        let second = CircularArc3::try_from_three_points(
            p(-3., 0.),
            p(-2. - diagonal, -diagonal),
            p(-2., -1.),
            document.tolerance(),
        )
        .unwrap();
        let first_id = document.add_geometry(Geometry::Arc(first)).unwrap();
        let second_id = document.add_geometry(Geometry::Arc(second)).unwrap();
        document
            .select_objects_direct([first_id, second_id], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Connect Join=Yes").unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined arc connection")
        };
        assert!(matches!(
            joined.segments(),
            [CurveSegment3::Arc(_), CurveSegment3::Arc(_)]
        ));
    }

    #[test]
    fn default_arc_style_trims_arc_to_line() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let arc = CircularArc3::try_from_three_points(
            p(1., 0.),
            p(diagonal, diagonal),
            p(0., 1.),
            document.tolerance(),
        )
        .unwrap();
        let line = LineSegment::try_new(p(0.5, 2.), p(0.5, 3.), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let second = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Connect").unwrap();
        let trimmed = document
            .objects()
            .find_map(|object| match object.geometry() {
                Geometry::Arc(arc) => Some(*arc),
                _ => None,
            })
            .expect("native trimmed arc");
        assert!((trimmed.sweep_radians() - std::f64::consts::PI / 3.0).abs() < 1e-12);
    }
}

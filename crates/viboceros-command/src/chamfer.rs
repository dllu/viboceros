//! Bevels selected straight curve ends by two distances.

use super::*;
use viboceros_geometry::{
    CurveArcExtensionStyle, CurveChamferExtensionStyles, CurveOtherExtensionStyle,
    try_chamfer_curves_joined_with_styles, try_chamfer_curves_parts_with_styles,
};

const USAGE: &str = "Chamfer distance1 distance2 | Chamfer Distances=distance1,distance2 [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [Trim=Yes|No] [ExtendArcsBy=Arc|Line] [ExtendOtherCurvesBy=Line|Smooth]";

pub(super) struct ChamferCommand;

struct ChamferOptions {
    distances: [Real; 2],
    picks: [Option<Point3>; 2],
    join: bool,
    trim: bool,
    arc_extension: CurveArcExtensionStyle,
    other_extension: CurveOtherExtensionStyle,
}

impl Command for ChamferCommand {
    fn name(&self) -> &'static str {
        "Chamfer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let ChamferOptions {
            distances,
            picks,
            join,
            trim,
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
                        .ok_or(CommandError::ChamferRequiresTwoCurves)?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let [(first_id, first), (second_id, second)] = selected.as_slice() else {
            return Err(CommandError::ChamferRequiresTwoCurves);
        };
        let (default_first, default_second) = super::fillet::nearest_ends(first, second)?;
        let picks = [
            picks[0].unwrap_or(default_first),
            picks[1].unwrap_or(default_second),
        ];
        let outputs = if join && trim {
            let joined = try_chamfer_curves_joined_with_styles(
                first,
                picks[0],
                second,
                picks[1],
                distances,
                CurveChamferExtensionStyles {
                    arc: arc_extension,
                    other: other_extension,
                },
                document.tolerance(),
            )?;
            let outputs = document.copy_object_pieces_into_source_groups([(
                *first_id,
                Geometry::PolyCurve(joined),
            )])?;
            document.delete_objects([*first_id, *second_id])?;
            outputs
        } else {
            let parts = try_chamfer_curves_parts_with_styles(
                (first, picks[0]),
                (second, picks[1]),
                distances,
                trim,
                CurveChamferExtensionStyles {
                    arc: arc_extension,
                    other: other_extension,
                },
                document.tolerance(),
            )?;
            let pieces = parts.into_iter().enumerate().map(|(index, curve)| {
                (
                    if index == 1 && trim {
                        *second_id
                    } else {
                        *first_id
                    },
                    Geometry::from(curve),
                )
            });
            let outputs = document.copy_object_pieces_into_source_groups(pieces)?;
            if trim {
                document.delete_objects([*first_id, *second_id])?;
            }
            outputs
        };
        document.select_objects_direct(outputs, SelectionMode::Replace)?;
        Ok("Chamfered two curves".to_string())
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

fn parse(arguments: &[&str]) -> Result<ChamferOptions, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(USAGE));
    };
    let (distances, consumed) = if let Some((name, values)) = first.split_once('=') {
        if !option_name_eq(name, "Distances") {
            return Err(CommandError::Usage(USAGE));
        }
        let (first, second) = values.split_once(',').ok_or(CommandError::Usage(USAGE))?;
        ([parse_finite_real(first)?, parse_finite_real(second)?], 1)
    } else {
        let Some(second) = arguments.get(1) else {
            return Err(CommandError::Usage(USAGE));
        };
        ([parse_finite_real(first)?, parse_finite_real(second)?], 2)
    };
    let mut picks = [None, None];
    let mut join = None;
    let mut trim = None;
    let mut arc_extension = None;
    let mut other_extension = None;
    for argument in &arguments[consumed..] {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
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
        if option_name_eq(name, "Join") {
            if join
                .replace(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                .is_some()
            {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        if option_name_eq(name, "Trim") {
            if trim
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
    Ok(ChamferOptions {
        distances,
        picks,
        join: join.unwrap_or(true),
        trim: trim.unwrap_or(true),
        arc_extension: arc_extension.unwrap_or(CurveArcExtensionStyle::Arc),
        other_extension: other_extension.unwrap_or(CurveOtherExtensionStyle::Line),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CircularArc3, CurveSegment3, LineSegment, NurbsCurve};

    #[test]
    fn chamfer_options_and_undo() {
        let registry = CommandRegistry::with_builtins();
        for (command, count, sources_retained) in [
            ("Chamfer 0.5 1", 1, false),
            ("Chamfer Distances=0.5,1", 1, false),
            ("Chamfer 0 0", 1, false),
            ("Chamfer 0.5 1 Join=No", 3, false),
            ("Chamfer 0.5 1 Trim=No", 3, true),
        ] {
            let mut document = Document::default();
            registry.execute(&mut document, "Line 0,0,0 4,0,0").unwrap();
            registry.execute(&mut document, "Line 4,0,0 4,4,0").unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
            let originals = document
                .objects()
                .map(|object| object.id())
                .collect::<Vec<_>>();
            let before = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, command).unwrap();
            assert_eq!(document.objects().count(), count, "{command}");
            assert_eq!(
                originals.iter().all(|id| document.object(*id).is_some()),
                sources_retained
            );
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }

    #[test]
    fn chamfers_meeting_arc_and_line_without_rationalizing_the_arc() {
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
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Chamfer 0.3 0.5").unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined chamfer");
        };
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::Arc(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_)
            ]
        ));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn extends_nonmeeting_arc_before_chamfer_and_undo() {
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
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "Chamfer 0.2 0.3 ExtendArcsBy=Arc")
            .unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined chamfer")
        };
        let [
            CurveSegment3::Arc(retained),
            CurveSegment3::Line(_),
            CurveSegment3::Line(_),
        ] = joined.segments()
        else {
            panic!("arc extension, bevel, line")
        };
        assert!((retained.length().unwrap() - (std::f64::consts::PI - 0.2)).abs() < 1e-9);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

        registry
            .execute(&mut document, "Chamfer 1.2 0.3 ExtendArcsBy=Line")
            .unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined tangent-extension chamfer")
        };
        let [
            CurveSegment3::Arc(retained),
            CurveSegment3::Line(_),
            CurveSegment3::Line(_),
        ] = joined.segments()
        else {
            panic!("original arc, bevel, retained line")
        };
        assert!((retained.length().unwrap() - (std::f64::consts::FRAC_PI_2 - 0.2)).abs() < 1e-9);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn smooth_option_extends_nonmeeting_nurbs_before_chamfer() {
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
        registry
            .execute(&mut document, "Chamfer 0.2 0.3 ExtendOtherCurvesBy=Smooth")
            .unwrap();
        let Geometry::PolyCurve(joined) = document.objects().next().unwrap().geometry() else {
            panic!("joined smooth NURBS chamfer")
        };
        assert!(matches!(
            joined.segments(),
            [
                CurveSegment3::NurbsCurve(_),
                CurveSegment3::Line(_),
                CurveSegment3::Line(_)
            ]
        ));
    }
}

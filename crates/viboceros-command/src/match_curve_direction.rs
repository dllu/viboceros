//! Orient selected curves to traverse in the reference curve's direction.
use super::*;

const USAGE: &str = "MatchCrvDir [Reference=object-name-or-id] (select a reference curve first, then one or more target curves)";

pub(super) struct MatchCurveDirectionCommand;

impl Command for MatchCurveDirectionCommand {
    fn name(&self) -> &'static str {
        "MatchCrvDir"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse_reference(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "MatchCrvDir",
            filter: ObjectSelectionFilter::Curves,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let named_reference = parse_reference(arguments)?;
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let reference_id = if let Some(reference) = named_reference {
            let mut matches = document.objects().filter(|object| {
                object.attributes().name() == Some(reference)
                    || object.id().to_string().eq_ignore_ascii_case(reference)
            });
            let id = matches.next().ok_or(CommandError::Usage(USAGE))?.id();
            if matches.next().is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            id
        } else {
            *selected.first().ok_or(CommandError::NoObjectsSelected)?
        };
        let reference = document
            .object(reference_id)
            .and_then(|object| object.geometry().curve_ref())
            .ok_or(CommandError::Usage(USAGE))?;
        let targets = selected
            .into_iter()
            .filter(|id| *id != reference_id)
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut replacements = Vec::new();
        for id in targets {
            let curve = document
                .object(id)
                .and_then(|object| object.geometry().curve_ref())
                .ok_or(CommandError::Usage(USAGE))?;
            if !reference.directions_match(curve)? {
                replacements.push((
                    id,
                    Geometry::from(curve.to_owned().reversed(document.tolerance())?),
                ));
            }
        }
        let count = document.replace_object_geometries(replacements)?;
        Ok(format!("Matched {count} curve direction(s)"))
    }
}

fn parse_reference<'a>(arguments: &[&'a str]) -> Result<Option<&'a str>, CommandError> {
    match arguments {
        [] => Ok(None),
        [argument] => {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if name.eq_ignore_ascii_case("Reference") && !value.is_empty() {
                Ok(Some(value))
            } else {
                Err(CommandError::Usage(USAGE))
            }
        }
        _ => Err(CommandError::Usage(USAGE)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_selected_curves_in_selection_order_and_preserves_identity() {
        let mut document = Document::new(Tolerance::DEFAULT);
        let reference = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let target = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(10.0, 1.0, 0.0).unwrap(),
                    Point3::try_new(0.0, 1.0, 0.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([reference, target], SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        assert_eq!(
            registry.execute(&mut document, "MatchCrvDir").unwrap(),
            "Matched 1 curve direction(s)"
        );
        let Geometry::Line(line) = document.object(target).unwrap().geometry() else {
            panic!("target line retained its representation");
        };
        assert_eq!(line.start().to_array(), [0.0, 1.0, 0.0]);
        assert_eq!(line.end().to_array(), [10.0, 1.0, 0.0]);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![reference, target]
        );
        assert_eq!(
            registry.execute(&mut document, "MatchCrvDir").unwrap(),
            "Matched 0 curve direction(s)"
        );
        assert!(document.undo().unwrap().is_some());
        let Geometry::Line(line) = document.object(target).unwrap().geometry() else {
            panic!("target line retained its representation");
        };
        assert_eq!(line.start().to_array(), [10.0, 1.0, 0.0]);
    }

    #[test]
    fn named_reference_supports_target_only_selection_and_invalid_selection_is_atomic() {
        let mut document = Document::new(Tolerance::DEFAULT);
        let reference = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(document.current_layer_id()).with_name("Reference"),
            )
            .unwrap();
        let target = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(10.0, 1.0, 0.0).unwrap(),
                    Point3::try_new(0.0, 1.0, 0.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let point = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 2.0, 0.0).unwrap()))
            .unwrap();
        document
            .select_objects_direct([target, point], SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        assert!(
            registry
                .execute(&mut document, "MatchCrvDir Reference=Reference")
                .is_err()
        );
        let Geometry::Line(line) = document.object(target).unwrap().geometry() else {
            panic!("target line retained its representation");
        };
        assert_eq!(line.start().to_array(), [10.0, 1.0, 0.0]);
        document
            .select_objects_direct([target], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "MatchCrvDir Reference=Reference")
                .unwrap(),
            "Matched 1 curve direction(s)"
        );
        let Geometry::Line(line) = document.object(reference).unwrap().geometry() else {
            panic!("reference line retained its representation");
        };
        assert_eq!(line.start().to_array(), [0.0, 0.0, 0.0]);
        let Geometry::Line(line) = document.object(target).unwrap().geometry() else {
            panic!("target line retained its representation");
        };
        assert_eq!(line.start().to_array(), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn matches_closed_curves_with_different_seams() {
        let mut document = Document::new(Tolerance::DEFAULT);
        let reference = document
            .add_geometry(Geometry::Circle(
                Circle3::try_from_frame(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    5.0,
                    UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
                    UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let target = document
            .add_geometry(Geometry::Circle(
                Circle3::try_from_frame(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    5.0,
                    UnitVector3::try_new(-1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap(),
                    UnitVector3::try_new(0.0, 0.0, -1.0, Tolerance::DEFAULT).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([reference, target], SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        assert_eq!(
            registry.execute(&mut document, "MatchCrvDir").unwrap(),
            "Matched 1 curve direction(s)"
        );
        let Geometry::Circle(circle) = document.object(target).unwrap().geometry() else {
            panic!("target circle retained its representation");
        };
        assert_eq!(circle.normal().unwrap().z(), 1.0);
        assert_eq!(
            circle.point_at_angle(0.0).unwrap().to_array(),
            [-5.0, 0.0, 0.0]
        );
    }
}

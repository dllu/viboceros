//! Set selected objects' defining coordinates to a target point.

use super::*;

const USAGE: &str = "SetPt target-point [XSet=Yes|No] [YSet=Yes|No] [ZSet=Yes|No] [Alignment=World|CPlane] [Copy=Yes|No]";

pub(super) struct SetPointCommand;

impl Command for SetPointCommand {
    fn name(&self) -> &'static str {
        "SetPt"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let selected = selected_ids(document)?;
        let mut axes = [true; 3];
        let mut seen = [false; 5];
        let mut copy = false;
        let mut cplane_alignment = false;
        let mut positional = Vec::new();
        for argument in arguments {
            let Some((name, value)) = argument.split_once('=') else {
                positional.push(*argument);
                continue;
            };
            let field = if option_name_eq(name, "XSet") {
                0
            } else if option_name_eq(name, "YSet") {
                1
            } else if option_name_eq(name, "ZSet") {
                2
            } else if option_name_eq(name, "Alignment") {
                3
            } else if option_name_eq(name, "Copy") {
                4
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if seen[field] {
                return Err(CommandError::Usage(USAGE));
            }
            seen[field] = true;
            match field {
                0..=2 => axes[field] = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                3 if option_name_eq(value, "World") => cplane_alignment = false,
                3 if option_name_eq(value, "CPlane") => cplane_alignment = true,
                3 => return Err(CommandError::Usage(USAGE)),
                4 => copy = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                _ => unreachable!(),
            }
        }
        if !axes.contains(&true) {
            return Err(CommandError::Usage(USAGE));
        }
        let (target, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, USAGE)?;
        let frame = if cplane_alignment {
            context.construction_plane
        } else {
            CommandContext::default().construction_plane
        }
        .with_origin(target);
        let transform = AffineTransform3::try_frame_mapping(
            frame,
            frame,
            axes.map(|enabled| if enabled { 0.0 } else { 1.0 }),
        )?;
        let (transformed, copied) =
            apply_transform_or_copy(document, selected.as_slice(), transform, copy)?;
        Ok(format!(
            "Set coordinates of {transformed} object(s), creating {copied} copy object(s)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_selected_world_or_cplane_coordinates_and_retains_copy_selection() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "Point 4,5,6").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "SetPt 7,8,9 XSet=No YSet=No ZSet=Yes")
            .unwrap();
        let points = document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(points, [[1.0, 2.0, 9.0], [4.0, 5.0, 9.0]]);
        assert_eq!(document.undo_label(), Some("SetPt"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 2);

        let plane = viboceros_geometry::Frame3::try_from_directions(
            Point3::try_new(10.0, 20.0, 30.0).unwrap(),
            viboceros_geometry::Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            viboceros_geometry::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        registry
            .execute_in_context(
                &mut document,
                "SetPt 7,8,9 XSet=No YSet=Yes ZSet=No Alignment=CPlane Copy=Yes",
                CommandContext {
                    construction_plane: plane,
                },
            )
            .unwrap();
        assert_eq!(document.objects().len(), 4);
        let points = document
            .objects()
            .map(|object| match object.geometry() {
                Geometry::Point(point) => point.to_array(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            points,
            [
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [1.0, 2.0, 9.0],
                [4.0, 5.0, 9.0]
            ]
        );
        assert_eq!(document.selected_object_count(), 2);
    }

    #[test]
    fn rejects_bad_options_before_mutating_the_document() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        for command in [
            "SetPt 0,0,0 XSet=No YSet=No ZSet=No",
            "SetPt 0,0,0 XSet=Maybe",
            "SetPt 0,0,0 Alignment=Camera",
            "SetPt 0,0,0 XSet=Yes XSet=No",
            "SetPt 0,0,0 Copy=Maybe",
            "SetPt nan,0,0",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
            assert_eq!(document.objects().len(), 1);
            assert_eq!(document.undo_label(), Some("Point"));
        }
    }

    #[test]
    fn flattens_polyline_vertices_and_nurbs_controls_in_one_undo_step() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, "Polyline 0,0,1 1,0,2 1,1,3")
            .unwrap();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,0,1 1,0,2 2,1,3 3,1,4")
            .unwrap();
        let before = document
            .objects()
            .map(|object| object.geometry().clone())
            .collect::<Vec<_>>();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "SetPt 0,0,5 XSet=No YSet=No ZSet=Yes")
            .unwrap();
        for object in document.objects() {
            match object.geometry() {
                Geometry::Polyline(polyline) => {
                    assert!(polyline.vertices().iter().all(|point| point.z() == 5.0));
                }
                Geometry::NurbsCurve(curve) => {
                    assert!(
                        curve
                            .control_points()
                            .iter()
                            .all(|control| control.point().z() == 5.0)
                    );
                }
                _ => panic!("unexpected geometry"),
            }
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document
                .objects()
                .map(|object| object.geometry().clone())
                .collect::<Vec<_>>(),
            before
        );
    }
}

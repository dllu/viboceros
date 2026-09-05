use super::*;
use viboceros_geometry::{Sweep1, SweepBlend, SweepFrameStyle, SweepSection};

const USAGE: &str = "Sweep1 Parameters=native-rail-parameters [RailName=name] [FrameStyle=Freeform|Roadlike] [Axis=x,y,z] [GlobalShapeBlending=Yes|No] [RefitRail=Yes|No]";

pub(super) struct SweepCommand;

impl Command for SweepCommand {
    fn name(&self) -> &'static str {
        "Sweep1"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (mut rail_name, mut parameters, mut roadlike, mut axis, mut global) =
            (None, None, None, None, None);
        let mut refit = None;
        for argument in arguments {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "RailName") && rail_name.is_none() && !value.is_empty() {
                rail_name = Some(value);
            } else if option_name_eq(name, "Parameters") && parameters.is_none() {
                parameters = Some(
                    value
                        .split(',')
                        .map(parse_finite_real)
                        .collect::<Result<Vec<_>, _>>()?,
                );
            } else if option_name_eq(name, "FrameStyle") && roadlike.is_none() {
                roadlike = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "freeform" => false,
                        "roadlike" => true,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "Axis") && axis.is_none() {
                axis = Some(
                    Vector3::try_from(parse_single_option_point(value, USAGE)?.to_array())?
                        .normalized_nonzero()?,
                );
            } else if option_name_eq(name, "GlobalShapeBlending") && global.is_none() {
                global = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "RefitRail") && refit.is_none() {
                refit = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let parameters = parameters.ok_or(CommandError::Usage(USAGE))?;
        if axis.is_some() && !roadlike.unwrap_or(false) {
            return Err(CommandError::Usage(USAGE));
        }
        let selected = selected_ids(document)?;
        let rail_id = resolve_curve_along_curve_path(document, &selected, rail_name)?;
        let rail = document
            .object(rail_id)
            .unwrap()
            .geometry()
            .curve_ref()
            .ok_or(CommandError::Usage(USAGE))?;
        let profiles = selected
            .into_iter()
            .filter(|id| *id != rail_id)
            .collect::<Vec<_>>();
        if profiles.len() != parameters.len() {
            return Err(CommandError::Usage(USAGE));
        }
        let sections = profiles
            .iter()
            .zip(parameters)
            .map(|(id, parameter)| {
                let curve = document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage(USAGE))?;
                Ok(SweepSection { parameter, curve })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let style = if roadlike.unwrap_or(false) {
            SweepFrameStyle::Roadlike(axis.unwrap_or(UnitVector3::try_new(
                0.,
                0.,
                1.,
                document.tolerance(),
            )?))
        } else {
            SweepFrameStyle::Freeform
        };
        let blend = if global.unwrap_or(false) {
            SweepBlend::Global
        } else {
            SweepBlend::Local
        };
        let sweep = Sweep1::try_new(rail, &sections, style, blend, document.tolerance())?;
        let surface = if refit.unwrap_or(false) {
            sweep.to_surface()?
        } else {
            sweep.to_rail_basis_surface()?
        };
        let [u, v] = surface.sampled_kink_parameters(document.tolerance().angular())?;
        let brep = Brep::try_surface_grid(&surface, &u, &v, document.tolerance())?;
        let count = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Swept {} sections into one BRep with {count} faces",
            sections.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_and_kinked_profiles_produce_valid_shared_brep_topology() {
        let registry = CommandRegistry::with_builtins();
        for (profile, expected_faces) in [("Circle 0,0,0 1", 1), ("Polyline 0,0,0 1,0,0 1,1,0", 2)]
        {
            let mut document = Document::default();
            for command in [
                "Line 0,0,0 0,0,5",
                "SelLast",
                "SetObjectName Rail",
                profile,
                "SelLast",
                "Sweep1 RailName=Rail Parameters=0",
            ] {
                registry.execute(&mut document, command).unwrap();
            }
            let object = document
                .objects()
                .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
                .unwrap();
            let Geometry::Brep(brep) = object.geometry() else {
                unreachable!()
            };
            assert_eq!(brep.faces().len(), expected_faces);
            assert!(!document.is_selected(object.id()));
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().count(), 2);
        }
    }

    #[test]
    fn multiple_selected_sections_obey_refit_and_blend_options() {
        let registry = CommandRegistry::with_builtins();
        for (blend, width) in [("No", 1.84375), ("Yes", 1.75)] {
            for style in ["FrameStyle=Freeform", "FrameStyle=Roadlike Axis=1,0,0"] {
                let mut document = Document::default();
                for command in [
                    "Line 0,0,0 0,0,5",
                    "SelLast",
                    "SetObjectName Rail",
                    "Line 0,0,0 2,0,0",
                    "SelLast",
                    "Line 0,0,5 1,0,5",
                    "SelLast DeselectOthersBeforeSelect=No",
                ] {
                    registry.execute(&mut document, command).unwrap();
                }
                let before = document.objects().cloned().collect::<Vec<_>>();
                let selected = document
                    .selected_objects()
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                registry
                    .execute(
                        &mut document,
                        "Sweep1 RailName=Rail Parameters=0,5 RefitRail=No",
                    )
                    .unwrap();
                let output = document
                    .objects()
                    .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
                    .unwrap();
                let Geometry::Brep(brep) = output.geometry() else {
                    unreachable!()
                };
                let point = brep.faces()[0].surface().evaluate(1.25, 1.).unwrap();
                assert_eq!(brep.faces()[0].surface().degree_u(), 1);
                assert!((point.x() - 1.75).abs() < 1e-12);
                registry.execute(&mut document, "Undo").unwrap();
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                registry.execute(&mut document, &format!(
                    "Sweep1 RailName=Rail Parameters=0,5 RefitRail=Yes GlobalShapeBlending={blend} {style}"
                )).unwrap();
                let object = document
                    .objects()
                    .find(|o| matches!(o.geometry(), Geometry::Brep(_)))
                    .unwrap();
                let Geometry::Brep(brep) = object.geometry() else {
                    unreachable!()
                };
                let point = brep.faces()[0].surface().evaluate(1.25, 1.).unwrap();
                assert!((point.x() - width).abs() < 1e-12);
                assert!(point.y().abs() < 1e-12);
                assert!((point.z() - 1.25).abs() < 1e-12);
                assert_eq!(
                    document
                        .selected_objects()
                        .map(|o| o.id())
                        .collect::<Vec<_>>(),
                    selected
                );
                registry.execute(&mut document, "Undo").unwrap();
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            }
        }
    }

    #[test]
    fn selected_profiles_and_named_rail_create_one_undoable_brep() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for command in [
            "Line 0,0,0 0,0,5",
            "SelLast",
            "SetObjectName Rail",
            "Line 0,0,0 2,0,0",
            "SelLast",
        ] {
            registry.execute(&mut document, command).unwrap();
        }
        let before = document.objects().cloned().collect::<Vec<_>>();
        let selected = document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        registry
            .execute(&mut document, "Sweep1 RailName=Rail Parameters=0")
            .unwrap();
        assert_eq!(document.objects().count(), 3);
        assert_eq!(
            document
                .selected_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>(),
            selected
        );
        let output = document
            .objects()
            .find(|o| !before.iter().any(|b| b.id() == o.id()))
            .unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("sweep must produce a BRep");
        };
        assert_eq!(brep.faces().len(), 1);
        assert_eq!(output.attributes().name(), None);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        for invalid in [
            "Sweep1 RailName=Rail",
            "Sweep1 RailName=Rail Parameters=0,5",
            "Sweep1 RailName=Rail Parameters=5",
            "Sweep1 RailName=Rail Parameters=0 Axis=0,0,1",
            "Sweep1 RailName=Rail Parameters=0 FrameStyle=Roadlike Axis=0,0,1",
            "Sweep1 RailName=Rail Parameters=0 RefitRail=Maybe",
            "Sweep1 RailName=Rail Parameters=0 Parameters=0",
            "Sweep1 RailName=Rail Parameters=0 RefitRail=Yes RefitRail=No",
            "Sweep1 RailName=Rail Parameters=0 Unknown=Yes",
        ] {
            assert!(
                registry.execute(&mut document, invalid).is_err(),
                "{invalid}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
}

//! Planar-region command policy with interchangeable geometry adapters.
use super::*;
mod chains;
mod portable;
#[cfg(not(feature = "native-smlib"))]
use portable as backend;
#[cfg(feature = "native-smlib")]
mod native;
#[cfg(feature = "native-smlib")]
use native as backend;

const PLANAR_SURFACE_USAGE: &str = "PlanarSrf [DeleteInput=Yes|No]";

fn delete_option(arguments: &[&str]) -> Result<bool, CommandError> {
    let mut delete_input = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(PLANAR_SURFACE_USAGE));
        };
        if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input =
                Some(parse_yes_no(value).ok_or(CommandError::Usage(PLANAR_SURFACE_USAGE))?);
        } else {
            return Err(CommandError::Usage(PLANAR_SURFACE_USAGE));
        }
    }

    Ok(delete_input.unwrap_or(false))
}

pub(super) struct PlanarSurfaceCommand;

impl Command for PlanarSurfaceCommand {
    fn name(&self) -> &'static str {
        "PlanarSrf"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["PlanarSurface"]
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: delete_option(arguments)?,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let delete_input = delete_option(arguments)?;
        let selected = selected_ids(document)?;
        let curves = selected
            .into_iter()
            .filter_map(|id| {
                document
                    .object(id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()
                    .map(|curve| curve.map(|curve| (id, curve)))
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let open = curves.iter().try_fold(false, |open, (_, c)| {
            Ok::<_, GeometryError>(open || !c.is_closed()?)
        })?;
        let (faces, consumed) = if open {
            let prepared = chains::prepare(curves, document.tolerance(), delete_input)?;
            prepared.stage(document.tolerance())?
        } else {
            stage_curves(curves, document.tolerance(), false)?
        };

        let face_count = faces.len();
        for face in faces {
            document.add_geometry(Geometry::Brep(face))?;
        }
        if delete_input {
            for id in consumed {
                document.delete_object(id)?;
            }
        }
        Ok(format!(
            "Created {face_count} exact trimmed planar surface(s){}",
            if delete_input {
                ", deleting the input curves"
            } else {
                ""
            }
        ))
    }
}

/// Shared pure construction for planar surfaces and editable planar meshes.
pub(super) fn stage_curves(
    curves: Vec<(ObjectId, NurbsCurve)>,
    tolerance: Tolerance,
    independent_contacts: bool,
) -> Result<(Vec<Brep>, Vec<ObjectId>), CommandError> {
    let (ids, profiles): (Vec<_>, Vec<_>) = curves.into_iter().unzip();
    let (faces, used) = stage_profiles(profiles, tolerance, independent_contacts)?;
    Ok((faces, used.into_iter().map(|i| ids[i]).collect()))
}
fn stage_profiles(
    profiles: Vec<NurbsCurve>,
    tolerance: Tolerance,
    independent_contacts: bool,
) -> Result<(Vec<Brep>, Vec<usize>), CommandError> {
    backend::stage(
        profiles.into_iter().enumerate().collect(),
        tolerance,
        independent_contacts,
    )
}

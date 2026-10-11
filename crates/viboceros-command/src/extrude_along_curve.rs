//! Fixed-orientation curve-path extrusion and transactional document policy.
use super::*;

#[cfg(feature = "native-smlib")]
mod native;

const EXTRUDE_CURVE_ALONG_CURVE_USAGE: &str = "ExtrudeCrvAlongCrv [PathName=name] [DeleteInput=Yes|No] [Output=Surface] [Solid=Yes|No] [SplitAtTangents=Yes|No]";

pub(super) struct ExtrudeCurveAlongCurveCommand;

impl Command for ExtrudeCurveAlongCurveCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvAlongCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (path_name, delete_input, solid) = parse_curve_along_curve_options(arguments)?;
        let selected = selected_ids(document)?;
        let path_id = resolve_curve_along_curve_path(document, &selected, path_name.as_deref())?;
        let path = document
            .object(path_id)
            .expect("resolved extrusion paths exist")
            .geometry()
            .nurbs_curve_representation()?
            .ok_or(CommandError::CurveAlongCurvePathNotCurve)?;
        let profile_ids = selected
            .iter()
            .copied()
            .filter(|id| *id != path_id)
            .collect::<Vec<_>>();
        if profile_ids.is_empty() {
            return Err(CommandError::CurveAlongCurveProfilesRequired);
        }

        let path_is_open = !path.is_closed()?;
        #[cfg(feature = "native-smlib")]
        let native_path = viboceros_smlib::KernelCurve::from_nurbs_for_modeling(&path)?;
        let mut extrusions = Vec::new();
        for id in &profile_ids {
            let Some(profile) = document
                .object(*id)
                .expect("selected objects exist")
                .geometry()
                .nurbs_curve_representation()?
            else {
                continue;
            };
            #[cfg(not(feature = "native-smlib"))]
            let geometry = if solid
                && path_is_open
                && profile.is_closed()?
                && profile.is_planar(document.tolerance())?
            {
                Geometry::Brep(Brep::try_extruded_curve_along_curve(
                    &profile,
                    &path,
                    document.tolerance(),
                )?)
            } else {
                Geometry::NurbsSurface(NurbsSurface::try_extruded_curve_along_curve(
                    &profile, &path,
                )?)
            };
            #[cfg(feature = "native-smlib")]
            let geometry = native::extrude(
                &profile,
                &native_path,
                solid && path_is_open,
                document.tolerance(),
            )?;
            extrusions.push((*id, geometry));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoCurveAlongCurveProfiles);
        }

        let output_count = extrusions.len();
        let solid_count = extrusions
            .iter()
            .filter(|(_, geometry)| matches!(geometry, Geometry::Brep(_)))
            .count();
        for (_, geometry) in &extrusions {
            document.add_geometry(geometry.clone())?;
        }
        if delete_input {
            for (id, _) in &extrusions {
                document.delete_object(*id)?;
            }
        }
        let retained_selection = selected
            .into_iter()
            .filter(|id| *id != path_id && document.object(*id).is_some())
            .collect::<Vec<_>>();
        document.select_objects_direct(retained_selection, SelectionMode::Replace)?;
        let surface_count = output_count - solid_count;
        Ok(format!(
            "Extruded {output_count} curve object(s) along the fixed-orientation path into {solid_count} exact capped B-rep solid(s) and {surface_count} exact NURBS surface(s){}",
            if delete_input {
                ", deleting the input profiles"
            } else {
                ""
            }
        ))
    }
}

fn parse_curve_along_curve_options(
    arguments: &[&str],
) -> Result<(Option<String>, bool, bool), CommandError> {
    let mut path_name = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid = None;
    let mut split_at_tangents_seen = false;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE));
        };
        if option_name_eq(name, "PathName") && path_name.is_none() && !value.is_empty() {
            path_name = Some(value.to_owned());
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = Some(
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?,
            );
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveAlongCurveOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && solid.is_none() {
            solid = Some(
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?,
            );
        } else if option_name_eq(name, "SplitAtTangents") && !split_at_tangents_seen {
            parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?;
            split_at_tangents_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE));
        }
    }
    Ok((
        path_name,
        delete_input.unwrap_or(false),
        solid.unwrap_or(false),
    ))
}

pub(super) fn resolve_curve_along_curve_path(
    document: &Document,
    selected: &[ObjectId],
    path_name: Option<&str>,
) -> Result<ObjectId, CommandError> {
    if let Some(name) = path_name {
        let matches = document
            .objects()
            .filter(|object| {
                object
                    .attributes()
                    .name()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
            .map(|object| object.id())
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(CommandError::CurveAlongCurvePathNotFound(name.to_owned())),
            [id] => Ok(*id),
            _ => Err(CommandError::AmbiguousCurveAlongCurvePath(name.to_owned())),
        }
    } else {
        selected
            .last()
            .copied()
            .ok_or(CommandError::CurveAlongCurvePathRequired)
    }
}

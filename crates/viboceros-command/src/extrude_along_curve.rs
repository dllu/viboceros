//! Fixed-orientation curve-path extrusion and transactional document policy.
use super::*;
mod options;
mod topology;

#[cfg(feature = "native-smlib")]
mod native;

const EXTRUDE_CURVE_ALONG_CURVE_USAGE: &str = "ExtrudeCrvAlongCrv [PathName=name] [DeleteInput=Yes|No] [Output=Surface] [Solid=Yes|No] [SplitAtTangents=Yes|No]";

pub(super) struct ExtrudeCurveAlongCurveCommand(pub crease_splitting::Preference);

impl Command for ExtrudeCurveAlongCurveCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvAlongCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (path_name, delete_input, solid, split) = options::parse(arguments)?;
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

        let crease = self.0.get().0;
        let path_cuts = topology::cuts(&path, crease, split)?;
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
            let wall = NurbsSurface::try_extruded_curve_along_curve(&profile, &path)?;
            #[cfg(feature = "native-smlib")]
            let wall = native::surface(&profile, &native_path)?;
            let profile_cuts = topology::cuts(&profile, crease, split)?;
            let geometry = topology::finish(
                wall,
                &profile,
                &path,
                [&profile_cuts, &path_cuts],
                solid,
                document.tolerance(),
            )?;
            extrusions.push((*id, geometry));
        }
        if extrusions.is_empty() {
            return Err(CommandError::NoCurveAlongCurveProfiles);
        }

        let output_count = extrusions.len();
        let input_ids = extrusions.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        for (_, geometry) in extrusions {
            document.add_geometry(geometry)?;
        }
        if delete_input {
            for id in input_ids {
                document.delete_object(id)?;
            }
        }
        let retained_selection = selected
            .into_iter()
            .filter(|id| *id != path_id && document.object(*id).is_some())
            .collect::<Vec<_>>();
        document.select_objects_direct(retained_selection, SelectionMode::Replace)?;
        Ok(format!(
            "Extruded {output_count} curve object(s) along the path{}",
            if delete_input {
                ", deleting the input profiles"
            } else {
                ""
            }
        ))
    }
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

//! Planar-region command policy with interchangeable geometry adapters.
use super::*;
#[cfg(not(feature = "native-smlib"))]
mod portable;
#[cfg(not(feature = "native-smlib"))]
use portable as backend;
#[cfg(feature = "native-smlib")]
mod native;
#[cfg(feature = "native-smlib")]
use native as backend;

const PLANAR_SURFACE_USAGE: &str = "PlanarSrf [DeleteInput=Yes|No]";

pub(super) struct PlanarSurfaceCommand;

impl Command for PlanarSurfaceCommand {
    fn name(&self) -> &'static str {
        "PlanarSrf"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["PlanarSurface"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
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

        let selected = selected_ids(document)?;
        let (faces, consumed) = backend::stage(document, selected)?;

        let face_count = faces.len();
        for face in faces {
            document.add_geometry(Geometry::Brep(face))?;
        }
        let delete_input = delete_input.unwrap_or(false);
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

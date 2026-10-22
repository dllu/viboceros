use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToSubDOptions {
    pub mesh_location: bool,
    pub mesh_corners: bool,
    pub mesh_creases: bool,
    pub surface_method: viboceros_geometry::SubDFromSurfaceMethod,
    pub surface_corners: bool,
    pub delete_input: bool,
}
impl ToSubDOptions {
    pub fn from_prompt(prompt: &ObjectSelectionPrompt) -> Result<Self, CommandError> {
        if prompt.command != "ToSubD" {
            return Err(CommandError::Usage(USAGE));
        }
        let line = prompt.command_line();
        let args = line.split_whitespace().skip(1).collect::<Vec<_>>();
        Ok(parse(&args, Self::default())?.options)
    }
}
impl Default for ToSubDOptions {
    fn default() -> Self {
        Self {
            mesh_location: false,
            mesh_corners: false,
            mesh_creases: false,
            surface_method: viboceros_geometry::SubDFromSurfaceMethod::Location,
            surface_corners: false,
            delete_input: false,
        }
    }
}
pub(super) struct Input {
    pub options: ToSubDOptions,
    pub object: Option<ObjectId>,
    pub faces: Option<SurfaceFaceIndices>,
}
pub(super) fn parse(args: &[&str], mut options: ToSubDOptions) -> Result<Input, CommandError> {
    let (mut object, mut faces) = (None, None);
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let (name, value, n) = orient_option(args, i, USAGE)?;
        let normalized = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if !seen.insert(normalized.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match normalized.as_str() {
            "usemesh" => options.mesh_location = location(value)?,
            "usesurface" => {
                options.surface_method = if location(value)? {
                    viboceros_geometry::SubDFromSurfaceMethod::Location
                } else {
                    viboceros_geometry::SubDFromSurfaceMethod::ControlPoints
                }
            }
            "meshcorners" => {
                options.mesh_corners = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "meshcreases" => {
                options.mesh_creases = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "surfacecorners" => {
                options.surface_corners = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "deleteinput" => {
                options.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "object" => object = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
            "faces" => faces = Some(parse_surface_face_indices(value, USAGE)?),
            _ => return Err(CommandError::Usage(USAGE)),
        }
        i += n;
    }
    if object.is_some() && faces.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Input {
        options,
        object,
        faces,
    })
}
fn location(value: &str) -> Result<bool, CommandError> {
    if option_name_eq(value, "Location") {
        Ok(true)
    } else if option_name_eq(value, "ControlPoints") {
        Ok(false)
    } else {
        Err(CommandError::Usage(USAGE))
    }
}
pub(super) fn prompt(options: ToSubDOptions) -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command: "ToSubD",
        filter: ObjectSelectionFilter::ToSubD,
        workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        menus: vec![],
        options: vec![
            BooleanSelectionOption {
                name: "MeshCreases",
                value: options.mesh_creases,
                aliases: &[],
            },
            BooleanSelectionOption {
                name: "MeshCorners",
                value: options.mesh_corners,
                aliases: &[],
            },
            BooleanSelectionOption {
                name: "SurfaceCorners",
                value: options.surface_corners,
                aliases: &[],
            },
            BooleanSelectionOption {
                name: "DeleteInput",
                value: options.delete_input,
                aliases: &[],
            },
        ],
        choices: vec![
            ChoiceSelectionOption {
                name: "UseMesh",
                value: if options.mesh_location {
                    "Location"
                } else {
                    "ControlPoints"
                },
                choices: &["ControlPoints", "Location"],
                toggle: None,
            },
            ChoiceSelectionOption {
                name: "UseSurface",
                value: if options.surface_method
                    == viboceros_geometry::SubDFromSurfaceMethod::Location
                {
                    "Location"
                } else {
                    "ControlPoints"
                },
                choices: &["ControlPoints", "Location"],
                toggle: None,
            },
        ],
    }
}

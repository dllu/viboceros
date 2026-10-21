//! Endpoint-network meshing shared by scripted commands and staged selection.
use super::*;

pub const USAGE: &str = "MeshFromLines [MaxSidesPerFace=n] [DeleteInput=Yes|No] [OutputLayer=Current|layer-id] [Sources=id,...]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub max_sides: usize,
    pub delete_input: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            max_sides: 4,
            delete_input: false,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub preferences: Preferences,
    pub output_layer: Option<String>,
    pub sources: Option<Vec<ObjectId>>,
}
pub fn parse(args: &[&str], preferences: Preferences) -> Result<Options, CommandError> {
    let mut options = Options {
        preferences,
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    for arg in args {
        let arg = arg.trim_start_matches('_');
        let (key, value) = if arg.eq_ignore_ascii_case("CurrentLayer") {
            ("OutputLayer", "Current")
        } else {
            arg.split_once('=').ok_or(CommandError::Usage(USAGE))?
        };
        if !seen.insert(key.to_ascii_lowercase()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.to_ascii_lowercase().as_str() {
            "maxsidesperface" => {
                options.preferences.max_sides = value
                    .parse()
                    .ok()
                    .filter(|n| *n >= 3)
                    .ok_or(CommandError::Usage(USAGE))?
            }
            "deleteinput" => {
                options.preferences.delete_input =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "outputlayer" => {
                options.output_layer = if value.eq_ignore_ascii_case("Current") {
                    None
                } else {
                    if value.is_empty() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    Some(value.to_owned())
                }
            }
            "sources" => {
                let ids = value
                    .split(',')
                    .take(TriangleMesh::MAX_LINE_NETWORK_LINES + 1)
                    .map(|s| s.parse().map_err(|_| CommandError::Usage(USAGE)))
                    .collect::<Result<Vec<_>, _>>()?;
                if ids.is_empty()
                    || ids.len() > TriangleMesh::MAX_LINE_NETWORK_LINES
                    || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
                {
                    return Err(CommandError::Usage(USAGE));
                }
                options.sources = Some(ids);
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    Ok(options)
}
pub(super) struct MeshFromLinesCommand(pub std::sync::Arc<remembered::Remembered<Preferences>>);
impl Command for MeshFromLinesCommand {
    fn name(&self) -> &'static str {
        "MeshFromLines"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(args, self.0.get())?;
        if options.sources.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::LineCurves,
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: options.preferences.delete_input,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = parse(args, self.0.get())?;
        let ids = options.sources.clone().unwrap_or_else(|| {
            document
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::LineCurves.accepts_object(o))
                .take(TriangleMesh::MAX_LINE_NETWORK_LINES + 1)
                .map(|o| o.id())
                .collect()
        });
        if ids.len() < 3 {
            return Err(CommandError::Usage(USAGE));
        }
        if ids.len() > TriangleMesh::MAX_LINE_NETWORK_LINES {
            return Err(GeometryError::MeshLineNetworkLimit.into());
        }
        let layer = if let Some(target) = &options.output_layer {
            document
                .layers()
                .find(|layer| {
                    layer.name().eq_ignore_ascii_case(target)
                        || layer.id().to_string().eq_ignore_ascii_case(target)
                })
                .map(|layer| layer.id())
                .ok_or(CommandError::Usage(USAGE))?
        } else {
            document.current_layer_id()
        };
        if document.layer(layer).is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        let lines = ids
            .iter()
            .map(|id| {
                let object = document
                    .object(*id)
                    .filter(|o| {
                        document.is_object_selectable(o.id())
                            && ObjectSelectionFilter::LineCurves.accepts_object(o)
                    })
                    .ok_or(CommandError::Usage(USAGE))?;
                let curve = object
                    .geometry()
                    .curve_ref()
                    .ok_or(CommandError::Usage(USAGE))?;
                Ok(viboceros_geometry::LineSegment::try_new(
                    curve.start_point()?,
                    curve.end_point()?,
                    viboceros_geometry::Tolerance::NUMERICAL_VALIDATION,
                )?)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let mesh = TriangleMesh::try_from_lines(
            &lines,
            options.preferences.max_sides,
            document.tolerance(),
        )?
        .map(|mesh| {
            mesh.unwelded_vertices_with_float_normals(std::f64::consts::PI / 3.)
                .map(|value| value.0)
        })
        .transpose()?;
        document.select_objects_direct(ids.iter().copied(), SelectionMode::Remove)?;
        let message = if let Some(mesh) = mesh {
            let count = mesh.faces().len();
            document.add_geometry_with_attributes(
                Geometry::Mesh(mesh),
                viboceros_document::ObjectAttributes::on_layer(layer),
            )?;
            if options.preferences.delete_input {
                document.delete_objects(ids)?;
            }
            format!("Created mesh from lines with {count} faces")
        } else {
            "No closed line regions within MaxSidesPerFace".into()
        };
        self.0.set(options.preferences);
        Ok(message)
    }
}
impl CommandRegistry {
    pub fn mesh_from_lines_options(&self, args: &[&str]) -> Result<Options, CommandError> {
        parse(args, self.mesh_from_lines_preferences.get())
    }
    pub fn accept_mesh_from_lines_options(&self, options: &Options) {
        self.mesh_from_lines_preferences.set(options.preferences);
    }
}

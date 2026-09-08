//! Exact object representation conversion, distinct from span decomposition.
use super::*;

#[cfg(test)]
mod tests;

const USAGE: &str = "ToNURBS [DeleteInputObjects=Yes|No] [MeshOptions TrimTriangularFaces=Yes|No]";
const MAX_OUTPUT_CONTROLS: usize = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Options {
    delete_input: bool,
    trim_triangles: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            delete_input: false,
            trim_triangles: true,
        }
    }
}

#[derive(Default)]
pub(super) struct ToNurbsCommand {
    options: remembered::Remembered<Options>,
}

impl Command for ToNurbsCommand {
    fn name(&self) -> &'static str {
        "ToNURBS"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, mesh) = parse(arguments, self.options.get())?;
        Ok(Some(prompt(options, mesh)))
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, mesh_option) = parse(arguments, self.options.get())?;
        let mesh = document
            .selected_objects()
            .any(|o| matches!(o.geometry(), Geometry::Mesh(_)));
        if mesh_option && !mesh {
            return Err(CommandError::Usage(USAGE));
        }
        let convertible = document.selected_objects().any(|o| {
            matches!(
                o.geometry(),
                Geometry::Line(_)
                    | Geometry::Circle(_)
                    | Geometry::Arc(_)
                    | Geometry::Polyline(_)
                    | Geometry::PolyCurve(_)
                    | Geometry::Mesh(_)
            )
        });
        Ok(convertible.then(|| prompt(options, mesh)))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.convert(document, arguments, false)
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        // ToNURBS commits choices only after a real conversion, not on prompt
        // edits or cancellation. Still validate the command-owned option syntax.
        parse(arguments, self.options.get()).map(|_| ())
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        let message = self.convert(document, arguments, true)?;
        document.clear_selection();
        Ok(message)
    }
}

impl ToNurbsCommand {
    fn convert(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let (options, mesh_option) = parse(arguments, self.options.get())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if mesh_option
            && !document
                .selected_objects()
                .any(|o| matches!(o.geometry(), Geometry::Mesh(_)))
        {
            return Err(CommandError::Usage(USAGE));
        }
        let mut eligible = 0;
        let mut controls = 0usize;
        let mut conversions = Vec::new();
        for object in document.objects().filter(|o| document.is_selected(o.id())) {
            let geometry = match object.geometry() {
                // Rhino stores an ellipse as a NURBS curve already. The compact
                // native analytic representation must not turn this into a copy.
                Geometry::Ellipse(_)
                | Geometry::NurbsCurve(_)
                | Geometry::NurbsSurface(_)
                | Geometry::Brep(_) => {
                    eligible += 1;
                    continue;
                }
                Geometry::Mesh(mesh) => {
                    charge(&mut controls, mesh.face_count().saturating_mul(4))?;
                    // ToNURBS retains disconnected pieces in one B-rep object;
                    // this intentionally differs from MeshToNURB's splitting.
                    Geometry::Brep(Brep::try_from_mesh(
                        mesh,
                        options.trim_triangles,
                        document.tolerance(),
                    )?)
                }
                source => {
                    let Some(curve) = source.converted_to_nurbs_curve()? else {
                        continue;
                    };
                    charge(&mut controls, curve.control_points().len())?;
                    Geometry::NurbsCurve(curve)
                }
            };
            eligible += 1;
            conversions.push((object.id(), geometry));
        }
        if eligible == 0 {
            return Err(CommandError::UnsupportedToNurbsGeometry);
        }
        let converted = conversions.len();
        if postselected {
            let ranks = document
                .selected_object_ids()
                .enumerate()
                .map(|(i, id)| (id, i))
                .collect::<BTreeMap<_, _>>();
            conversions.sort_unstable_by_key(|(id, _)| ranks[id]);
        }
        if options.delete_input {
            let ids = conversions.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            document.replace_object_geometries(conversions)?;
            if postselected {
                document.move_objects_to_end_in_order(ids)?;
            } else {
                document.move_objects_to_end(ids)?;
            }
        } else if postselected {
            document.copy_object_geometries_into_source_groups_in_order(conversions)?;
        } else {
            document.copy_object_geometries_into_source_groups(conversions)?;
        }
        // Rhino's ToNURBS no-op does not accept new choices, unlike
        // ConvertToSingleSpans. Only a real conversion updates option memory.
        if converted > 0 {
            self.options.set(options);
        }
        Ok(format!(
            "Converted {converted} object(s) to exact NURBS geometry; {} unchanged{}",
            eligible - converted,
            if options.delete_input {
                "; inputs replaced"
            } else {
                "; inputs retained"
            }
        ))
    }
}

fn prompt(options: Options, mesh: bool) -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command: "ToNURBS",
        filter: ObjectSelectionFilter::ToNurbs,
        workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        choices: vec![],
        options: vec![BooleanSelectionOption {
            name: "DeleteInputObjects",
            value: options.delete_input,
            aliases: &["DeleteInput"],
        }],
        menus: if mesh {
            vec![BooleanSelectionMenu {
                name: "MeshOptions",
                options: vec![BooleanSelectionOption {
                    name: "TrimTriangularFaces",
                    value: options.trim_triangles,
                    aliases: &[],
                }],
            }]
        } else {
            vec![]
        },
    }
}

fn charge(total: &mut usize, count: usize) -> Result<(), CommandError> {
    *total = total.saturating_add(count);
    if *total > MAX_OUTPUT_CONTROLS {
        return Err(CommandError::TooManyNurbsConversionControls {
            maximum: MAX_OUTPUT_CONTROLS,
        });
    }
    Ok(())
}

fn parse(arguments: &[&str], mut options: Options) -> Result<(Options, bool), CommandError> {
    let (mut delete_seen, mut trim_seen, mut mesh_seen, mut i) = (false, false, false, 0);
    while i < arguments.len() {
        if option_name_eq(arguments[i], "MeshOptions") && !mesh_seen {
            mesh_seen = true;
            i += 1;
            continue;
        }
        let (name, value, consumed) = orient_option(arguments, i, USAGE)?;
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        if (option_name_eq(name, "DeleteInputObjects") || option_name_eq(name, "DeleteInput"))
            && !delete_seen
        {
            options.delete_input = value;
            delete_seen = true;
        } else if option_name_eq(name, "TrimTriangularFaces") && !trim_seen {
            options.trim_triangles = value;
            trim_seen = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    Ok((options, mesh_seen || trim_seen))
}

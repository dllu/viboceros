//! Mesh face conversion with Rhino's selection, option, and component policies.
use super::*;

#[cfg(test)]
mod tests;

const USAGE: &str = "MeshToNURB [TrimTriangularFaces=Yes|No] [UseNgons=Yes|No]";
const MAX_OUTPUT_CONTROLS: usize = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Options {
    trim_triangular_faces: bool,
    use_ngons: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            trim_triangular_faces: true,
            use_ngons: true,
        }
    }
}

#[derive(Default)]
pub(super) struct MeshToNurbCommand {
    options: remembered::Remembered<Options>,
}

impl Command for MeshToNurbCommand {
    fn name(&self) -> &'static str {
        "MeshToNURB"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(arguments, self.options.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            options: vec![
                BooleanSelectionOption {
                    name: "TrimTriangularFaces",
                    value: options.trim_triangular_faces,
                },
                BooleanSelectionOption {
                    name: "UseNgons",
                    value: options.use_ngons,
                },
            ],
        }))
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.options.set(parse(arguments, self.options.get())?);
        Ok(())
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.accept_object_selection_options(arguments)?;
        let result = self.convert(document, arguments, true)?;
        document.clear_selection();
        Ok(result)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.convert(document, arguments, false)
    }
}

impl MeshToNurbCommand {
    fn convert(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let options = parse(arguments, self.options.get())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut total_face_count = 0usize;
        let mut outputs = Vec::new();
        // Preselection follows document order; prompted picks follow action
        // order. Build a rank map instead of repeated linear ID lookups.
        let mut sources = document
            .objects()
            .filter(|o| document.is_selected(o.id()))
            .collect::<Vec<_>>();
        if postselected {
            let rank = document
                .selected_object_ids()
                .enumerate()
                .map(|(i, id)| (id, i))
                .collect::<BTreeMap<_, _>>();
            sources.sort_unstable_by_key(|o| rank[&o.id()]);
        }
        // Borrow source meshes; component extraction already owns its pieces.
        for object in sources {
            let Geometry::Mesh(mesh) = object.geometry() else {
                continue;
            };
            total_face_count = total_face_count.saturating_add(mesh.face_count());
            if total_face_count > MAX_OUTPUT_CONTROLS / 4 {
                return Err(CommandError::TooManyMeshNurbsControls {
                    maximum: MAX_OUTPUT_CONTROLS,
                });
            }
            for piece in mesh.disjoint_pieces() {
                outputs.push((
                    Geometry::Brep(Brep::try_from_mesh(
                        &piece,
                        options.trim_triangular_faces,
                        document.tolerance(),
                    )?),
                    object.attributes().clone(),
                ));
            }
        }
        if outputs.is_empty() {
            return Err(CommandError::UnsupportedMeshToNurbGeometry);
        }
        let output_count = outputs.len();
        // All geometry is staged before mutation. Fresh outputs intentionally
        // inherit attributes but not their source's group memberships.
        for (geometry, attributes) in outputs {
            document.add_geometry_with_attributes(geometry, attributes)?;
        }
        self.options.set(options);
        Ok(format!(
            "Created {output_count} NURBS B-rep(s) from {total_face_count} mesh face(s); source mesh(es) retained"
        ))
    }
}

fn parse(arguments: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let (mut trim_seen, mut ngons_seen, mut i) = (false, false, 0);
    while i < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, i, USAGE)?;
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "TrimTriangularFaces") && !trim_seen {
            options.trim_triangular_faces = value;
            trim_seen = true;
        } else if option_name_eq(name, "UseNgons") && !ngons_seen {
            // Triangles and quads have no n-gon regions; both choices are
            // equivalent for the currently representable input geometry.
            options.use_ngons = value;
            ngons_seen = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    Ok(options)
}

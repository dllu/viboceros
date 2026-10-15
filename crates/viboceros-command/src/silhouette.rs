//! Parallel-view silhouettes, staged completely before document admission.
use super::*;

pub const USAGE: &str = "Silhouette [InternalEdges=Yes|No] [Direction=x,y,z]";
pub(super) struct SilhouetteCommand;

fn options(arguments: &[&str]) -> Result<(bool, Option<Vector3>), CommandError> {
    let (mut internal, mut direction) = (false, None);
    let mut seen = BTreeSet::new();
    for word in arguments {
        let (key, value) = word
            .trim_start_matches('_')
            .split_once('=')
            .ok_or(CommandError::Usage(USAGE))?;
        let key = key.to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.as_str() {
            "internaledges" => internal = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
            "direction" => {
                let (point, count) = parse_point(&[value])?;
                if count != 1 {
                    return Err(CommandError::Usage(USAGE));
                }
                direction = Some(
                    Vector3::try_from(point.to_array())?
                        .normalized_nonzero()?
                        .as_vector(),
                );
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    Ok((internal, direction))
}

impl Command for SilhouetteCommand {
    fn name(&self) -> &'static str {
        "Silhouette"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (internal, direction) = options(args)?;
        if direction.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::EdgeAnalysis,
            options: vec![BooleanSelectionOption {
                name: "InternalEdges",
                value: internal,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (internal, direction) = options(args)?;
        let direction = direction.unwrap_or(context.construction_plane.z_axis().as_vector());
        let inputs = document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::EdgeAnalysis.accepts_object(o))
            .collect::<Vec<_>>();
        if inputs.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut staged = Vec::new();
        for source in inputs {
            staged.extend(produce(
                source.geometry(),
                direction,
                internal,
                document.tolerance(),
            )?);
            if staged.len() > 100_000 {
                return Err(CommandError::Usage(
                    "Silhouette exceeds 100000 output curves",
                ));
            }
        }
        if staged.is_empty() {
            return Ok("Created 0 silhouette curves".into());
        }
        let ids = staged
            .into_iter()
            .map(|curve| document.add_geometry(Geometry::NurbsCurve(curve)))
            .collect::<Result<Vec<_>, _>>()?;
        let count = ids.len();
        document.select_command_results(ids)?;
        Ok(format!("Created {count} silhouette curves"))
    }
}

fn produce(
    geometry: &Geometry,
    direction: Vector3,
    internal: bool,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        let brep = match geometry {
            Geometry::Brep(brep) => brep.clone(),
            Geometry::NurbsSurface(surface) => Brep::try_surface_face(surface.clone(), tolerance)?,
            Geometry::Mesh(mesh) => {
                // Preserve the mesh's actual triangle image, including warped quads.
                let triangles = TriangleMesh::try_new(
                    mesh.vertices().to_vec(),
                    mesh.triangles().to_vec(),
                    tolerance,
                )?;
                Brep::try_from_mesh(&triangles, true, tolerance)?
            }
            _ => return Err(CommandError::Usage(USAGE)),
        };
        Ok(
            viboceros_smlib::Shape::from_brep(&brep, tolerance)?.parallel_silhouette_curves(
                direction,
                tolerance.absolute(),
                0.05,
                internal,
            )?,
        )
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (geometry, direction, internal, tolerance);
        Err(CommandError::SilhouetteRequiresNativeKernel)
    }
}

#[cfg(test)]
mod tests;

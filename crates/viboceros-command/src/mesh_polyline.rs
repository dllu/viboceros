//! Closed-polyline mesh construction with ordinary source selection.
use super::*;

const USAGE: &str = "MeshPolyline [Sources=id,...]";
pub(super) struct MeshPolylineCommand;

fn sources(args: &[&str]) -> Result<Option<Vec<ObjectId>>, CommandError> {
    if args.is_empty() {
        return Ok(None);
    }
    let [argument] = args else {
        return Err(CommandError::Usage(USAGE));
    };
    let (key, value) = argument
        .trim_start_matches('_')
        .split_once('=')
        .ok_or(CommandError::Usage(USAGE))?;
    if !key.eq_ignore_ascii_case("Sources") {
        return Err(CommandError::Usage(USAGE));
    }
    let ids = value
        .split(',')
        .take(viboceros_geometry::MAX_MESH_PATCH_POINTS + 1)
        .map(|s| s.parse().map_err(|_| CommandError::Usage(USAGE)))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty()
        || ids.len() > viboceros_geometry::MAX_MESH_PATCH_POINTS
        || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Some(ids))
}

impl Command for MeshPolylineCommand {
    fn name(&self) -> &'static str {
        "MeshPolyline"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(sources(args)?.is_none().then_some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::ClosedPolylines,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let ids = sources(args)?.unwrap_or_else(|| {
            document
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::ClosedPolylines.accepts_object(o))
                .take(viboceros_geometry::MAX_MESH_PATCH_POINTS + 1)
                .map(|o| o.id())
                .collect()
        });
        if ids.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut count = 0usize;
        let mut meshes = Vec::new();
        for id in &ids {
            let object = document
                .object(*id)
                .filter(|o| {
                    document.is_object_selectable(*id)
                        && ObjectSelectionFilter::ClosedPolylines.accepts_object(o)
                })
                .ok_or(CommandError::Usage(USAGE))?;
            let vertices = object
                .geometry()
                .curve_ref()
                .ok_or(CommandError::Usage(USAGE))?
                .polyline_vertices()?
                .ok_or(CommandError::Usage(USAGE))?;
            count = count
                .checked_add(vertices.len())
                .filter(|n| *n <= viboceros_geometry::MAX_MESH_PATCH_POINTS)
                .ok_or(GeometryError::MeshPatchLimit)?;
            let polyline = Polyline3::try_new(vertices, Tolerance::NUMERICAL_VALIDATION)?;
            if let Some(mesh) = TriangleMesh::try_from_closed_polyline(&polyline)? {
                meshes.push((mesh, object.attributes().clone()));
            }
        }
        // Complete construction before changing selection or the document.
        document.clear_selection();
        let count = meshes.len();
        for (mesh, attributes) in meshes {
            document.add_geometry_with_attributes(Geometry::Mesh(mesh), attributes)?;
        }
        Ok(format!("Created {count} meshes from closed polylines"))
    }
}

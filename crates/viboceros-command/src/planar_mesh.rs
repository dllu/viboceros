//! Planar region meshing reuses the curve joiner and planar surface adapters.
use super::*;
use viboceros_geometry::{CurveJoinOptions, CurveJoinStyle};

const USAGE: &str = "PlanarMesh [Sources=id,...]";
const MAX_PROFILES: usize = 128;
pub(super) struct PlanarMeshCommand;

fn sources(args: &[&str]) -> Result<Option<Vec<ObjectId>>, CommandError> {
    if args.is_empty() {
        return Ok(None);
    }
    let [arg] = args else {
        return Err(CommandError::Usage(USAGE));
    };
    let (key, value) = arg
        .trim_start_matches('_')
        .split_once('=')
        .ok_or(CommandError::Usage(USAGE))?;
    if !key.eq_ignore_ascii_case("Sources") {
        return Err(CommandError::Usage(USAGE));
    }
    let ids = value
        .split(',')
        .take(MAX_PROFILES + 1)
        .map(|s| s.parse().map_err(|_| CommandError::Usage(USAGE)))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty()
        || ids.len() > MAX_PROFILES
        || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Some(ids))
}
impl Command for PlanarMeshCommand {
    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        postselected: bool,
    ) {
        if postselected && matches!(error, CommandError::NoPlanarSurfaceBoundaries) {
            document.clear_selection();
        }
    }
    fn name(&self) -> &'static str {
        "PlanarMesh"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(sources(args)?.is_none().then_some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let ids = sources(args)?.unwrap_or_else(|| {
            document
                .selected_objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .take(MAX_PROFILES + 1)
                .map(|o| o.id())
                .collect()
        });
        if ids.is_empty() || ids.len() > MAX_PROFILES {
            return Err(CommandError::Usage(USAGE));
        }
        let curves = ids
            .iter()
            .map(|id| {
                Ok(document
                    .object(*id)
                    .filter(|_| document.is_object_selectable(*id))
                    .ok_or(CommandError::Usage(USAGE))?
                    .geometry()
                    .curve_ref()
                    .ok_or(CommandError::Usage(USAGE))?
                    .to_owned())
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let joined = viboceros_geometry::join_curves(
            &curves,
            CurveJoinOptions {
                tolerance: document.tolerance().absolute(),
                preserve_direction: false,
                style: CurveJoinStyle::Batch,
            },
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let profiles = joined
            .into_iter()
            .map(|chain| {
                let id = ids[chain.source_indices()[0]];
                Ok((id, chain.into_curve().as_ref().to_nurbs()?))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        profiles.iter().try_fold(0usize, |total, (_, curve)| {
            total
                .checked_add(curve.control_points().len())
                .filter(|n| *n <= viboceros_geometry::MAX_MESH_PATCH_POINTS)
                .ok_or(GeometryError::MeshPatchLimit)
        })?;
        let (regions, _) = planar_surface::stage_curves(profiles, document.tolerance(), true)?;
        let meshes = regions
            .iter()
            .map(|region| region.planar_polygon_mesh(document.tolerance()))
            .collect::<Result<Vec<_>, _>>()?;
        document.clear_selection();
        let count = meshes.len();
        for mesh in meshes {
            document.add_geometry(Geometry::Mesh(mesh))?;
        }
        Ok(format!(
            "Created {count} planar meshes; input curves retained"
        ))
    }
}

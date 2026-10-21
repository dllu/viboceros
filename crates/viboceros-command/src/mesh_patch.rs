//! MeshPatch input policy; construction is shared by scripts and staged picking.
use super::*;
use viboceros_geometry::{MAX_MESH_PATCH_POINTS, MeshPatchOptions};
#[cfg(test)]
mod tests;

pub const USAGE: &str = "MeshPatch [AngleTolerance=degrees] [TrimBack=Yes|No] [MainSegmentCount=n] [Sources=id,...] [Holes=id,...] [StartingSurface=id Face=n] [OuterBoundary=id]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub angle: Real,
    pub trim_back: bool,
    pub divisions: usize,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            angle: 15.,
            trim_back: true,
            divisions: 10,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub preferences: Preferences,
    pub sources: Option<Vec<ObjectId>>,
    pub holes: Vec<ObjectId>,
    pub surface: Option<ObjectId>,
    pub face: Option<usize>,
    pub outer: Option<ObjectId>,
}
pub fn parse(args: &[&str], preferences: Preferences) -> Result<Options, CommandError> {
    let mut result = Options {
        preferences,
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    for arg in args {
        let (key, value) = arg
            .trim_start_matches('_')
            .split_once('=')
            .ok_or(CommandError::Usage(USAGE))?;
        if !seen.insert(key.to_ascii_lowercase()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.to_ascii_lowercase().as_str() {
            "angletolerance" => {
                let angle = parse_finite_real(value)?;
                if !(0. ..180.).contains(&angle) || angle == 0. {
                    return Err(CommandError::Usage(USAGE));
                }
                result.preferences.angle = angle;
            }
            "trimback" => {
                result.preferences.trim_back =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "mainsegmentcount" => {
                result.preferences.divisions = value
                    .parse()
                    .ok()
                    .filter(|n| (1..=MAX_MESH_PATCH_POINTS / 4).contains(n))
                    .ok_or(CommandError::Usage(USAGE))?
            }
            "sources" => result.sources = Some(ids(value)?),
            "holes" => result.holes = ids(value)?,
            "startingsurface" => {
                result.surface = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
            }
            "face" => result.face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
            "outerboundary" => {
                result.outer = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if result.surface.is_none() && result.face.is_some() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(result)
}
fn ids(value: &str) -> Result<Vec<ObjectId>, CommandError> {
    let ids = value
        .split(',')
        .take(MAX_MESH_PATCH_POINTS + 1)
        .map(|v| v.parse().map_err(|_| CommandError::Usage(USAGE)))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty()
        || ids.len() > MAX_MESH_PATCH_POINTS
        || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(ids)
}
fn input(doc: &Document, id: ObjectId) -> Result<&Geometry, CommandError> {
    if !doc.is_object_selectable(id) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(doc
        .object(id)
        .ok_or(DocumentError::ObjectNotFound(id))?
        .geometry())
}
fn polyline(doc: &Document, id: ObjectId, angle: Real) -> Result<Polyline3, CommandError> {
    input(doc, id)?
        .curve_ref()
        .ok_or(CommandError::Usage(USAGE))?
        .try_polyline_by_angle(angle.to_radians(), MAX_MESH_PATCH_POINTS)
        .map_err(Into::into)
}
fn charge(count: &mut usize, amount: usize) -> Result<(), CommandError> {
    *count = count
        .checked_add(amount)
        .filter(|n| *n <= MAX_MESH_PATCH_POINTS)
        .ok_or(GeometryError::MeshPatchLimit)?;
    Ok(())
}
pub(super) struct MeshPatchCommand(pub std::sync::Arc<remembered::Remembered<Preferences>>);
impl Command for MeshPatchCommand {
    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(error, CommandError::Geometry(GeometryError::EmptyMeshPatch)) {
            document.clear_selection();
        }
    }
    fn name(&self) -> &'static str {
        "MeshPatch"
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
            filter: ObjectSelectionFilter::MeshPatchSources,
            options: vec![BooleanSelectionOption {
                name: "TrimBack",
                value: options.preferences.trim_back,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = parse(args, self.0.get())?;
        let selected = options.sources.clone().unwrap_or_else(|| {
            document
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::MeshPatchSources.accepts_object(o))
                .take(MAX_MESH_PATCH_POINTS + 1)
                .map(|o| o.id())
                .collect()
        });
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if selected.len() > MAX_MESH_PATCH_POINTS {
            return Err(GeometryError::MeshPatchLimit.into());
        }
        let mut points = Vec::new();
        let mut guides = Vec::new();
        let mut holes = Vec::new();
        let mut count = 0usize;
        let hole_ids = options.holes.iter().copied().collect::<BTreeSet<_>>();
        for id in &selected {
            if hole_ids.contains(id) || options.surface == Some(*id) || options.outer == Some(*id) {
                continue;
            }
            match input(document, *id)? {
                Geometry::Point(p) => {
                    charge(&mut count, 1)?;
                    points.push(*p)
                }
                Geometry::PointCloud(c) => {
                    charge(&mut count, c.points().len())?;
                    points.extend_from_slice(c.points())
                }
                g if g.curve_ref().is_some() => {
                    let curve = polyline(document, *id, options.preferences.angle)?;
                    charge(&mut count, curve.vertices().len())?;
                    guides.push(curve);
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        for id in &options.holes {
            let curve = polyline(document, *id, options.preferences.angle)?;
            charge(&mut count, curve.vertices().len())?;
            holes.push(curve);
        }
        let outer = options
            .outer
            .map(|id| polyline(document, id, options.preferences.angle))
            .transpose()?;
        if let Some(curve) = &outer {
            charge(&mut count, curve.vertices().len())?;
        }
        let mut reference_args = Vec::new();
        if let Some(id) = options.surface {
            reference_args.push(format!("Surface={id}"));
        }
        if let Some(face) = options.face {
            reference_args.push(format!("Face={face}"));
        }
        let reference = options
            .surface
            .map(|_| {
                uv_reference::resolve(
                    document,
                    &reference_args
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    USAGE,
                )
            })
            .transpose()?;
        let mesh = TriangleMesh::try_patch(
            &points,
            &guides,
            &holes,
            MeshPatchOptions {
                trim_back: options.preferences.trim_back,
                divisions: options.preferences.divisions,
                starting_surface: reference.as_ref().map(|r| r.surface),
                outer_boundary: outer.as_ref(),
            },
            document.tolerance(),
        )?;
        let faces = mesh.faces().len();
        let vertices = mesh.vertices().len();
        document.add_geometry(Geometry::Mesh(mesh))?;
        document.clear_selection();
        self.0.set(options.preferences);
        Ok(format!(
            "Created mesh patch with {vertices} vertices and {faces} triangles"
        ))
    }
}
impl CommandRegistry {
    pub fn mesh_patch_options(&self, args: &[&str]) -> Result<Options, CommandError> {
        parse(args, self.mesh_patch_preferences.get())
    }
    pub fn accept_mesh_patch_options(&self, options: &Options) {
        self.mesh_patch_preferences.set(options.preferences);
    }
}

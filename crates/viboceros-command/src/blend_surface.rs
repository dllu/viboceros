//! Surface-edge blends, with readonly preparation and atomic acceptance.
use super::*;
use viboceros_geometry::{
    SurfaceBlendContinuity, SurfaceBlendDiagnostics, SurfaceBlendOptions, SurfaceExtensionEdge,
    try_blend_nurbs_surfaces,
};

#[cfg(test)]
mod tests;
const USAGE: &str = "BlendSrf [object-id edge-index object-id edge-index | Sources=id,id] [Edge1=West|South|East|North] [Edge2=West|South|East|North] [Continuity1=G0|G1|G2] [Continuity2=G0|G1|G2] [Bulge1=positive] [Bulge2=positive] [Reverse2=Auto|Yes|No] [Trim1=0..1] [Trim2=0..1] [Join=Yes|No] [Preview=Yes|No]";
pub(super) struct BlendSurfaceCommand;

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub sources: Option<[ObjectId; 2]>,
    pub edges: [SurfaceExtensionEdge; 2],
    pub edge_indices: Option<[usize; 2]>,
    pub blend: SurfaceBlendOptions,
    pub trims: [Real; 2],
    pub join: bool,
    pub preview: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            sources: None,
            edges: [SurfaceExtensionEdge::East, SurfaceExtensionEdge::West],
            edge_indices: None,
            blend: SurfaceBlendOptions::default(),
            trims: [0.; 2],
            join: false,
            preview: false,
        }
    }
}
impl Options {
    pub fn updated(&self, arguments: &[&str]) -> Result<Self, CommandError> {
        parse_with_defaults(arguments, self.clone())
    }
}
pub fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    parse_with_defaults(arguments, Options::default())
}
fn parse_with_defaults(arguments: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let mut seen = BTreeSet::new();
    let mut positional = Vec::new();
    for argument in arguments {
        let Some((key, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        let key = key.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_');
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.as_str() {
            "sources" => {
                let ids = value
                    .split(',')
                    .map(|id| {
                        id.parse::<ObjectId>()
                            .map_err(|_| CommandError::Usage(USAGE))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                options.sources = Some(ids.try_into().map_err(|_| CommandError::Usage(USAGE))?);
            }
            "edge1" | "edge2" => {
                let i = usize::from(key.ends_with('2'));
                options.edges[i] = match value.to_ascii_lowercase().as_str() {
                    "west" => SurfaceExtensionEdge::West,
                    "south" => SurfaceExtensionEdge::South,
                    "east" => SurfaceExtensionEdge::East,
                    "north" => SurfaceExtensionEdge::North,
                    _ => return Err(CommandError::Usage(USAGE)),
                };
            }
            "continuity1" | "continuity2" => {
                options.blend.continuity[usize::from(key.ends_with('2'))] =
                    match value.to_ascii_lowercase().as_str() {
                        "g0" | "position" => SurfaceBlendContinuity::Position,
                        "g1" | "tangency" => SurfaceBlendContinuity::Tangency,
                        "g2" | "curvature" => SurfaceBlendContinuity::Curvature,
                        _ => return Err(CommandError::Usage(USAGE)),
                    };
            }
            "bulge1" | "bulge2" => {
                let number = parse_finite_real(value)?;
                if number <= 0. {
                    return Err(CommandError::Usage(USAGE));
                }
                options.blend.bulges[usize::from(key.ends_with('2'))] = number;
            }
            "trim1" | "trim2" => {
                let number = parse_finite_real(value)?;
                if !(0.0..1.0).contains(&number) {
                    return Err(CommandError::Usage(USAGE));
                }
                options.trims[usize::from(key.ends_with('2'))] = number;
            }
            "reverse2" => {
                options.blend.reverse_second = if value.eq_ignore_ascii_case("Auto") {
                    None
                } else {
                    Some(boolean(value)?)
                }
            }
            "join" => options.join = boolean(value)?,
            "preview" => options.preview = boolean(value)?,
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if !positional.is_empty() {
        if positional.len() != 4
            || seen.contains("sources")
            || seen.contains("edge1")
            || seen.contains("edge2")
        {
            return Err(CommandError::Usage(USAGE));
        }
        options.sources = Some([
            positional[0]
                .parse()
                .map_err(|_| CommandError::Usage(USAGE))?,
            positional[2]
                .parse()
                .map_err(|_| CommandError::Usage(USAGE))?,
        ]);
        options.edge_indices = Some([
            positional[1]
                .parse()
                .map_err(|_| CommandError::Usage(USAGE))?,
            positional[3]
                .parse()
                .map_err(|_| CommandError::Usage(USAGE))?,
        ]);
    }
    if options.sources.is_some_and(|ids| ids[0] == ids[1]) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(options)
}
fn boolean(value: &str) -> Result<bool, CommandError> {
    if value.eq_ignore_ascii_case("Yes") {
        Ok(true)
    } else if value.eq_ignore_ascii_case("No") {
        Ok(false)
    } else {
        Err(CommandError::Usage(USAGE))
    }
}
fn unsupported(context: &'static str) -> CommandError {
    GeometryError::InvalidSurfaceBlend { context }.into()
}

#[derive(Clone, Debug)]
pub struct Prepared {
    pub brep: Brep,
    pub diagnostics: SurfaceBlendDiagnostics,
    pub reversed_second: bool,
    pub continuity: [SurfaceBlendContinuity; 2],
    ids: [ObjectId; 2],
    sources: [viboceros_document::GeometrySnapshot; 2],
    trimmed: [Option<Brep>; 2],
    tolerance: Tolerance,
    join: bool,
}
impl Prepared {
    pub fn message(&self) -> String {
        format!(
            "BlendSrf G{}/G{}: {} boundary checks; position error {:.3e}, tangent plane angle {:.3e} degrees, first/second derivative relative error {:.3e}/{:.3e}; singular samples {}; Reverse2={}",
            self.continuity[0].order(),
            self.continuity[1].order(),
            self.diagnostics.samples,
            self.diagnostics.maximum_position_error,
            self.diagnostics.maximum_tangent_plane_angle.to_degrees(),
            self.diagnostics.maximum_first_derivative_relative_error,
            self.diagnostics.maximum_second_derivative_relative_error,
            self.diagnostics.singular_boundary_samples,
            if self.reversed_second { "Yes" } else { "No" }
        )
    }
    pub fn validate_source(&self, document: &Document) -> Result<(), CommandError> {
        if self.tolerance != document.tolerance()
            || self.ids.iter().enumerate().any(|(i, id)| {
                !document.is_object_selectable(*id)
                    || document
                        .object(*id)
                        .is_none_or(|o| o.geometry_snapshot() != &self.sources[i])
            })
        {
            return Err(unsupported(
                "sources or tolerance changed; prepare the preview again",
            ));
        }
        Ok(())
    }
    /// Applies the already validated output to a document or preview copy.
    /// Commands wrap this in their standard transaction; API clients can use commit.
    pub fn apply(&self, document: &mut Document) -> Result<ObjectId, CommandError> {
        self.validate_source(document)?;
        let id = if self.join {
            document
                .replace_object_geometries([(self.ids[0], Geometry::Brep(self.brep.clone()))])?;
            document.delete_object(self.ids[1])?;
            self.ids[0]
        } else {
            for i in 0..2 {
                if let Some(brep) = &self.trimmed[i] {
                    document
                        .replace_object_geometries([(self.ids[i], Geometry::Brep(brep.clone()))])?;
                }
            }
            document.add_geometry(Geometry::Brep(self.brep.clone()))?
        };
        document.select_objects_direct([id], SelectionMode::Replace)?;
        Ok(id)
    }
    pub fn commit(&self, document: &mut Document) -> Result<ObjectId, CommandError> {
        run_command_transaction(document, "BlendSrf", |document| self.apply(document))
    }
}

pub fn prepare(
    document: &Document,
    ids: [ObjectId; 2],
    options: &Options,
) -> Result<Prepared, CommandError> {
    if ids[0] == ids[1] {
        return Err(CommandError::Usage(USAGE));
    }
    let mut sources = Vec::new();
    let mut surfaces = Vec::new();
    let mut breps = Vec::new();
    let mut trimmed = [None, None];
    let mut edges = options.edges;
    for (i, id) in ids.into_iter().enumerate() {
        let object = document
            .object(id)
            .filter(|_| document.is_object_selectable(id))
            .ok_or(CommandError::Usage(USAGE))?;
        sources.push(object.geometry_snapshot().clone());
        let brep = match object.geometry() {
            Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), document.tolerance())?,
            Geometry::Brep(b) if b.faces().len() == 1 => b.clone(),
            _ => {
                return Err(unsupported(
                    "select two single-face surfaces; ExtractSrf can detach a polysurface face",
                ));
            }
        };
        let face = &brep.faces()[0];
        let bounds = face
            .rectangular_trim_bounds(document.tolerance())?
            .ok_or_else(|| {
                unsupported("only complete rectangular/isoparametric boundaries are supported")
            })?;
        if let Some(indices) = options.edge_indices {
            edges[i] = classify_edge(&brep, indices[i], bounds)?;
        }
        let mut ranges = [bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1]];
        let axis = usize::from(matches!(
            edges[i],
            SurfaceExtensionEdge::South | SurfaceExtensionEdge::North
        ));
        let start = *ranges[axis].start();
        let end = *ranges[axis].end();
        if matches!(
            edges[i],
            SurfaceExtensionEdge::West | SurfaceExtensionEdge::South
        ) {
            ranges[axis] = (start + (end - start) * options.trims[i])..=end;
        } else {
            ranges[axis] = start..=(end - (end - start) * options.trims[i]);
        }
        let surface = face
            .surface()
            .try_trimmed(ranges[0].clone(), ranges[1].clone())?;
        let parent = if options.trims[i] > 0. {
            let b = Brep::try_rectangular_surface_face_with_orientation(
                surface.clone(),
                surface.domain_u(),
                surface.domain_v(),
                face.is_reversed(),
                document.tolerance(),
            )?;
            trimmed[i] = Some(b.clone());
            b
        } else {
            brep
        };
        surfaces.push(surface);
        breps.push(parent);
    }
    let result = try_blend_nurbs_surfaces(
        &surfaces[0],
        edges[0],
        &surfaces[1],
        edges[1],
        options.blend,
        document.tolerance(),
    )?;
    let blend = Brep::try_surface_face(result.surface, document.tolerance())?;
    let output = if options.join {
        let mut components = viboceros_geometry::join_breps(
            &[&breps[0], &blend, &breps[1]],
            document.tolerance().absolute(),
            document.tolerance(),
        )?;
        if components.len() != 1 || components[0].joined_edge_count < 2 {
            return Err(unsupported(
                "both blend boundaries could not be joined; sources were preserved",
            ));
        }
        components.remove(0).brep
    } else {
        blend
    };
    Ok(Prepared {
        brep: output,
        diagnostics: result.diagnostics,
        reversed_second: result.reversed_second,
        continuity: options.blend.continuity,
        ids,
        sources: sources.try_into().unwrap(),
        trimmed,
        tolerance: document.tolerance(),
        join: options.join,
    })
}

fn classify_edge(
    brep: &Brep,
    index: usize,
    bounds: [[Real; 2]; 2],
) -> Result<SurfaceExtensionEdge, CommandError> {
    if index >= brep.edges().len() {
        return Err(CommandError::Usage(USAGE));
    }
    let trims = brep.faces()[0]
        .loops()
        .iter()
        .flat_map(|l| l.trims())
        .filter(|t| t.edge() == Some(index))
        .collect::<Vec<_>>();
    let [trim] = trims.as_slice() else {
        return Err(unsupported(
            "select a naked complete isoparametric edge, not a seam",
        ));
    };
    let a = trim.curve().start_point()?;
    let b = trim.curve().end_point()?;
    let edge = if a.x() == b.x() && a.x() == bounds[0][0] {
        SurfaceExtensionEdge::West
    } else if a.x() == b.x() && a.x() == bounds[0][1] {
        SurfaceExtensionEdge::East
    } else if a.y() == b.y() && a.y() == bounds[1][0] {
        SurfaceExtensionEdge::South
    } else if a.y() == b.y() && a.y() == bounds[1][1] {
        SurfaceExtensionEdge::North
    } else {
        return Err(unsupported(
            "edge must cover a complete rectangular boundary",
        ));
    };
    Ok(edge)
}

impl Command for BlendSurfaceCommand {
    fn name(&self) -> &'static str {
        "BlendSrf"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        let ids = if let Some(ids) = options.sources {
            ids
        } else {
            selected_ids(document)?
                .try_into()
                .map_err(|_| CommandError::Usage(USAGE))?
        };
        let prepared = prepare(document, ids, &options)?;
        if !options.preview {
            prepared.apply(document)?;
        }
        Ok(format!(
            "{}; {}",
            prepared.message(),
            if options.preview {
                "preview only"
            } else {
                "created surface blend"
            }
        ))
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(arguments)?;
        Ok(options.sources.is_none().then_some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Surfaces,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

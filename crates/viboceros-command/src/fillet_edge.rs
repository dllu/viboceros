//! Stage owned circular, chamfer and G2 edge blends before document edits.
use super::*;
use viboceros_document::{EdgeBlendMode, EdgeBlendRecipe, GeometrySnapshot};
pub use viboceros_smlib::EdgeBlendSizes;
mod face_edges;
pub use face_edges::edge_blend_face_edges;
mod sizes;
use sizes::{blend_patches, blend_with_sizes, native_rail};
#[cfg(test)]
mod tests;
const USAGE: &str = "FilletEdge radius object-id edge-index[,edge-index...] ... | FilletEdge Radius=radius All=Yes [Corner=Automatic|Ball] [TrimAndJoin=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilletCorner {
    #[default]
    Automatic,
    Ball,
}

impl FilletCorner {
    pub fn token(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Ball => "Ball",
        }
    }

    fn parse(value: &str) -> Result<Self, CommandError> {
        if option_name_eq(value, "Automatic") {
            Ok(Self::Automatic)
        } else if option_name_eq(value, "Ball") {
            Ok(Self::Ball)
        } else {
            Err(CommandError::Usage(USAGE))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FilletOptions {
    pub radius: Real,
    pub corner: FilletCorner,
}

impl Default for FilletOptions {
    fn default() -> Self {
        Self {
            radius: 1.,
            corner: FilletCorner::Automatic,
        }
    }
}

impl FilletOptions {
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        for word in words {
            if let Some(value) = corner_option(word) {
                self.corner = FilletCorner::parse(value)?;
            } else {
                self.radius = parse_radius(word)?;
            }
        }
        Ok(())
    }
}

fn corner_option(word: &str) -> Option<&str> {
    word.split_once('=')
        .filter(|(name, _)| option_name_eq(name, "Corner"))
        .map(|(_, value)| value)
}

#[derive(Clone, Debug)]
pub struct EdgeBlendSelection {
    kind: EdgeBlendKind,
    sources: Vec<EdgeBlendSource>,
    outputs: Vec<(ObjectId, Geometry)>,
    trim_and_join: bool,
    tolerance: Tolerance,
}

/// Backward-compatible selected-edge fillet entry point.
pub type FilletEdgeSelection = EdgeBlendSelection;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeBlendKind {
    Fillet(FilletCorner),
    Chamfer(crate::chamfer_edge::ChamferRailType),
    Curvature(crate::blend_edge::BlendRailType),
}

impl From<EdgeBlendMode> for EdgeBlendKind {
    fn from(mode: EdgeBlendMode) -> Self {
        use crate::edge_blend_input::RailType;
        match mode {
            EdgeBlendMode::Fillet => Self::Fillet(FilletCorner::Automatic),
            EdgeBlendMode::BallFillet => Self::Fillet(FilletCorner::Ball),
            EdgeBlendMode::ChamferRollingBall => Self::Chamfer(RailType::RollingBall),
            EdgeBlendMode::ChamferBetweenRails => Self::Chamfer(RailType::DistanceBetweenRails),
            EdgeBlendMode::BlendRollingBall => Self::Curvature(RailType::RollingBall),
            EdgeBlendMode::BlendBetweenRails => Self::Curvature(RailType::DistanceBetweenRails),
        }
    }
}

impl EdgeBlendKind {
    fn mode(self) -> EdgeBlendMode {
        use crate::edge_blend_input::RailType;
        match self {
            Self::Fillet(FilletCorner::Automatic) => EdgeBlendMode::Fillet,
            Self::Fillet(FilletCorner::Ball) => EdgeBlendMode::BallFillet,
            Self::Chamfer(RailType::RollingBall) => EdgeBlendMode::ChamferRollingBall,
            Self::Chamfer(RailType::DistanceBetweenRails) => EdgeBlendMode::ChamferBetweenRails,
            Self::Curvature(RailType::RollingBall) => EdgeBlendMode::BlendRollingBall,
            Self::Curvature(RailType::DistanceBetweenRails) => EdgeBlendMode::BlendBetweenRails,
        }
    }
    fn usage(self) -> &'static str {
        match self {
            Self::Fillet(_) => USAGE,
            Self::Chamfer(_) => crate::chamfer_edge::USAGE,
            Self::Curvature(_) => crate::blend_edge::USAGE,
        }
    }
    pub fn command(self) -> &'static str {
        match self {
            Self::Fillet(_) => "FilletEdge",
            Self::Chamfer(_) => "ChamferEdge",
            Self::Curvature(_) => "BlendEdge",
        }
    }
}

type EdgeBlendSource = (ObjectId, GeometrySnapshot, Vec<(usize, EdgeBlendSizes)>);

/// Read-only input captured from the original edge indices and owned geometry.
#[derive(Clone, PartialEq)]
pub struct EdgeBlendPlan {
    kind: EdgeBlendKind,
    trim_and_join: bool,
    tolerance: Tolerance,
    sources: Vec<EdgeBlendSource>,
}

impl EdgeBlendSelection {
    pub fn prepare(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        radius: Real,
    ) -> Result<Self, CommandError> {
        Self::prepare_fillet(
            document,
            picks,
            FilletOptions {
                radius,
                ..Default::default()
            },
        )
    }
    pub fn prepare_fillet(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        options: FilletOptions,
    ) -> Result<Self, CommandError> {
        Self::prepare_blend(
            document,
            picks,
            options.radius,
            EdgeBlendKind::Fillet(options.corner),
        )
    }
    pub fn prepare_chamfer(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        distance: Real,
        rail: crate::chamfer_edge::ChamferRailType,
    ) -> Result<Self, CommandError> {
        Self::prepare_blend(document, picks, distance, EdgeBlendKind::Chamfer(rail))
    }
    pub fn prepare_curvature(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        radius: Real,
        rail: crate::blend_edge::BlendRailType,
    ) -> Result<Self, CommandError> {
        Self::prepare_blend(document, picks, radius, EdgeBlendKind::Curvature(rail))
    }
    fn prepare_blend(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        radius: Real,
        kind: EdgeBlendKind,
    ) -> Result<Self, CommandError> {
        EdgeBlendPlan::capture(document, picks, radius, kind)?.solve()
    }
    /// Replacements to apply when trim and join is enabled.
    pub fn replacements(&self) -> &[(ObjectId, Geometry)] {
        if self.trim_and_join {
            &self.outputs
        } else {
            &[]
        }
    }
    /// Staged model-space geometry paired with the corresponding input.
    pub fn outputs(&self) -> &[(ObjectId, Geometry)] {
        &self.outputs
    }
    pub fn commit(&self, document: &mut Document) -> Result<usize, CommandError> {
        run_command_transaction(document, self.kind.command(), |doc| self.apply(doc))
    }
    /// Commit a result solved against the original edges of a joined blend.
    /// The scene used for solving can differ from the live object version.
    pub fn commit_edit(
        &self,
        document: &mut Document,
        object: ObjectId,
        live: &GeometrySnapshot,
    ) -> Result<usize, CommandError> {
        let [(source_id, source, _)] = self.sources.as_slice() else {
            return Err(CommandError::Usage(
                "Edit operates on one edge blend at a time",
            ));
        };
        if *source_id != object
            || document.tolerance() != self.tolerance
            || !document.is_object_selectable(object)
            || !document
                .object(object)
                .is_some_and(|o| o.geometry_snapshot().shares_storage_with(live))
            || !live
                .edge_blend_recipe()
                .is_some_and(|recipe| recipe.source.shares_storage_with(source))
        {
            return Err(CommandError::Usage(
                "The edge blend changed. Reopen Edit to continue.",
            ));
        }
        run_command_transaction(document, self.kind.command(), |doc| {
            if !self.trim_and_join {
                doc.restore_edge_blend_source(object)?;
            }
            self.apply_results(doc)
        })
    }

    pub(crate) fn apply(&self, document: &mut Document) -> Result<usize, CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, snapshot, _)| {
                !document.is_object_selectable(*id)
                    || !document
                        .object(*id)
                        .is_some_and(|object| object.geometry_snapshot() == snapshot)
            })
        {
            return Err(CommandError::Usage(
                "Edge blend source or tolerance changed; select edges again",
            ));
        }
        self.apply_results(document)
    }

    fn apply_results(&self, document: &mut Document) -> Result<usize, CommandError> {
        if self.trim_and_join {
            document.replace_edge_blend_results(self.sources.iter().zip(&self.outputs).map(
                |((source_id, source, edges), (output_id, geometry))| {
                    debug_assert_eq!(source_id, output_id);
                    (
                        *source_id,
                        geometry.clone(),
                        EdgeBlendRecipe {
                            mode: self.kind.mode(),
                            source: source.clone(),
                            edges: edges.clone(),
                        },
                    )
                },
            ))?;
            document.select_objects_direct(
                self.outputs.iter().map(|(id, _)| *id),
                SelectionMode::Replace,
            )?;
        } else {
            for (source, geometry) in &self.outputs {
                let parent = document
                    .object(*source)
                    .expect("verified edge blend source");
                document
                    .add_geometry_with_attributes(geometry.clone(), parent.attributes().clone())?;
            }
            document.clear_selection();
        }
        Ok(self.outputs.len())
    }
}

impl EdgeBlendPlan {
    pub fn with_trim_and_join(mut self, trim_and_join: bool) -> Self {
        self.trim_and_join = trim_and_join;
        self
    }

    pub fn matches_except_size(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.trim_and_join == other.trim_and_join
            && self.tolerance == other.tolerance
            && self.sources.len() == other.sources.len()
            && self.sources.iter().zip(&other.sources).all(|(a, b)| {
                a.0 == b.0
                    && a.1 == b.1
                    && a.2
                        .iter()
                        .map(|edge| edge.0)
                        .eq(b.2.iter().map(|edge| edge.0))
            })
    }

    pub fn capture(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        radius: Real,
        kind: EdgeBlendKind,
    ) -> Result<Self, CommandError> {
        if !radius.is_finite() || radius <= 0. {
            return Err(CommandError::Usage(kind.usage()));
        }
        Self::capture_sizes(
            document,
            picks.into_iter().map(|(id, edge)| (id, edge, radius)),
            kind,
        )
    }

    pub fn capture_sizes(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize, Real)>,
        kind: EdgeBlendKind,
    ) -> Result<Self, CommandError> {
        Self::capture_end_sizes(
            document,
            picks
                .into_iter()
                .map(|(id, index, size)| (id, index, [size; 2])),
            kind,
        )
    }

    /// Sizes at the start and end of each original edge's curve domain.
    pub fn capture_end_sizes(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize, [Real; 2])>,
        kind: EdgeBlendKind,
    ) -> Result<Self, CommandError> {
        Self::capture_profiles(
            document,
            picks
                .into_iter()
                .map(|(id, edge, sizes)| (id, edge, sizes.into())),
            kind,
        )
    }

    pub fn capture_profiles(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize, EdgeBlendSizes)>,
        kind: EdgeBlendKind,
    ) -> Result<Self, CommandError> {
        let mut by_object = BTreeMap::<ObjectId, BTreeMap<usize, EdgeBlendSizes>>::new();
        for (count, (id, edge, sizes)) in picks.into_iter().enumerate() {
            if count >= 100000 || !sizes.is_valid() || !document.is_object_selectable(id) {
                return Err(CommandError::Usage(kind.usage()));
            }
            match by_object.entry(id).or_default().entry(edge) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(sizes);
                }
                std::collections::btree_map::Entry::Occupied(entry) if entry.get() != &sizes => {
                    return Err(CommandError::Usage("An edge has conflicting blend sizes"));
                }
                _ => {}
            }
        }
        if by_object.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut sources = Vec::with_capacity(by_object.len());
        for (id, edges) in by_object {
            let object = document
                .object(id)
                .ok_or(CommandError::Usage(kind.usage()))?;
            let Geometry::Brep(brep) = object.geometry() else {
                return Err(CommandError::Usage(kind.usage()));
            };
            if !brep.is_solid() || edges.keys().any(|&edge| edge >= brep.edges().len()) {
                return Err(CommandError::Usage(kind.usage()));
            }
            if edges.iter().any(|(&edge, sizes)| {
                let [a, b] = brep.edges()[edge].vertices();
                a == b && sizes.ends[0] != sizes.ends[1]
            }) {
                return Err(CommandError::Usage(
                    "A closed edge has one size at its seam",
                ));
            }
            sources.push((
                id,
                object.geometry_snapshot().clone(),
                edges.into_iter().collect(),
            ));
        }
        Ok(Self {
            kind,
            trim_and_join: true,
            sources,
            tolerance: document.tolerance(),
        })
    }

    pub fn solve(self) -> Result<EdgeBlendSelection, CommandError> {
        let mut sources = Vec::with_capacity(self.sources.len());
        let mut outputs = Vec::with_capacity(self.sources.len());
        for (id, geometry, edges) in self.sources {
            let Geometry::Brep(brep) = &*geometry else {
                unreachable!("captured an edge blend on a B-rep");
            };
            if self.trim_and_join {
                let size = edges[0].1.ends[0];
                let result = if edges.iter().all(|(_, values)| values.all_equal_to(size)) {
                    let indices = edges.iter().map(|edge| edge.0).collect::<Vec<_>>();
                    blend(brep, &indices, size, self.kind, self.tolerance)?
                } else {
                    blend_with_sizes(brep, &edges, self.kind, self.tolerance)?
                };
                outputs.push((id, Geometry::Brep(result)));
            } else {
                outputs.extend(
                    blend_patches(brep, &edges, self.kind, self.tolerance)?
                        .into_iter()
                        .map(|patch| (id, Geometry::Brep(patch))),
                );
            }
            sources.push((id, geometry, edges));
        }
        Ok(EdgeBlendSelection {
            kind: self.kind,
            sources,
            outputs,
            trim_and_join: self.trim_and_join,
            tolerance: self.tolerance,
        })
    }
}

fn blend(
    brep: &Brep,
    edges: &[usize],
    radius: Real,
    kind: EdgeBlendKind,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let (local, to_world) = local_blend_source(brep, tolerance)?;
    let blended = match kind {
        EdgeBlendKind::Fillet(corner) => {
            let primary = viboceros_smlib::Solid::fillet_brep_edges_with_corner(
                &local,
                edges,
                radius,
                tolerance,
                corner == FilletCorner::Ball,
            )
            .and_then(|solid| solid.to_brep(tolerance));
            if corner == FilletCorner::Automatic {
                primary.or_else(|error| {
                    checked_local_fillet(&local, edges, radius, tolerance).ok_or(error)
                })?
            } else {
                primary?
            }
        }
        EdgeBlendKind::Chamfer(rail) | EdgeBlendKind::Curvature(rail) => {
            let rail = native_rail(rail);
            let solid = if matches!(kind, EdgeBlendKind::Curvature(_)) {
                viboceros_smlib::Solid::blend_brep_edges(&local, edges, radius, rail, tolerance)?
            } else {
                viboceros_smlib::Solid::chamfer_brep_edges(&local, edges, radius, rail, tolerance)?
            };
            solid.to_brep(tolerance)?
        }
    };
    Ok(blended.transformed(to_world, tolerance)?)
}

fn local_blend_source(
    brep: &Brep,
    tolerance: Tolerance,
) -> Result<(Brep, AffineTransform3), CommandError> {
    let origin = brep
        .vertices()
        .first()
        .map(|v| v.point())
        .unwrap_or_else(|| brep.bounds().min());
    let zero = Point3::try_new(0., 0., 0.)?;
    let local = brep.transformed(
        AffineTransform3::from_translation(origin.vector_to(zero)?),
        tolerance,
    )?;
    Ok((
        local,
        AffineTransform3::from_translation(zero.vector_to(origin)?),
    ))
}

fn checked_local_fillet(
    brep: &Brep,
    edges: &[usize],
    radius: Real,
    tolerance: Tolerance,
) -> Option<Brep> {
    let candidate =
        viboceros_smlib::Solid::fillet_brep_edges_with_local_merge(brep, edges, radius, tolerance)
            .ok()?;
    validate_local_fillet(candidate)
}

fn validate_local_fillet(candidate: Brep) -> Option<Brep> {
    let geometry = Geometry::Brep(candidate);
    let check = crate::geometry_check::check_geometry(&geometry);
    if let Geometry::Brep(candidate) = geometry
        && candidate.is_solid()
        && check.status == crate::geometry_check::GeometryCheckStatus::Valid
    {
        return Some(candidate);
    }
    None
}

pub(super) struct FilletEdgeCommand;
impl Command for FilletEdgeCommand {
    fn name(&self) -> &'static str {
        "FilletEdge"
    }
    fn component_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        let (arguments, _) = crate::edge_blend_input::trim_and_join(arguments, USAGE)?;
        if arguments
            .iter()
            .filter(|w| corner_option(w).is_none())
            .count()
            > 1
        {
            return Ok(None);
        }
        let mut options = FilletOptions::default();
        options.update(&arguments)?;
        Ok(Some(ComponentSelectionPrompt {
            command: "FilletEdge",
            kind: ComponentSelectionKind::BrepEdge,
            options: Vec::new(),
            numbers: vec![NumberSelectionOption {
                name: "Radius",
                value: options.radius,
                minimum: Real::MIN_POSITIVE,
            }],
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, trim_and_join) = crate::edge_blend_input::trim_and_join(arguments, USAGE)?;
        let mut options = FilletOptions::default();
        let mut positional = Vec::new();
        for word in arguments {
            if let Some(value) = corner_option(word) {
                options.corner = FilletCorner::parse(value)?;
            } else {
                positional.push(word);
            }
        }
        let arguments = positional.as_slice();
        let Some(first) = arguments.first() else {
            return Err(CommandError::Usage(USAGE));
        };
        options.radius = parse_radius(first)?;
        let mut picks = Vec::new();
        if arguments.len() == 2 && option_name_eq(arguments[1], "All=Yes") {
            for object in document.selected_objects() {
                let Geometry::Brep(brep) = object.geometry() else {
                    return Err(CommandError::Usage(USAGE));
                };
                picks.extend(
                    brep.edges_shared_by_distinct_faces()
                        .into_iter()
                        .enumerate()
                        .filter(|(_, shared)| *shared)
                        .map(|(edge, _)| (object.id(), edge)),
                );
            }
        } else {
            if arguments.len() < 3 || !arguments[1..].len().is_multiple_of(2) {
                return Err(CommandError::Usage(USAGE));
            }
            for pair in arguments[1..].chunks_exact(2) {
                let id = pair[0]
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                for edge in pair[1].split(',') {
                    picks.push((
                        id,
                        edge.parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))?,
                    ));
                }
            }
        }
        let count = EdgeBlendPlan::capture(
            document,
            picks,
            options.radius,
            EdgeBlendKind::Fillet(options.corner),
        )?
        .with_trim_and_join(trim_and_join)
        .solve()?
        .apply(document)?;
        Ok(if trim_and_join {
            format!("Filleted edges on {count} object(s)")
        } else {
            format!("Created {count} fillet surface object(s)")
        })
    }
}
fn parse_radius(text: &str) -> Result<Real, CommandError> {
    let value = if let Some((name, value)) = text.split_once('=') {
        if !option_name_eq(name, "Radius") {
            return Err(CommandError::Usage(USAGE));
        }
        value
    } else {
        text
    };
    let radius = parse_finite_real(value)?;
    if radius <= 0. {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(radius)
}

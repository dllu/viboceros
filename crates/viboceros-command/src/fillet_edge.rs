//! Stage owned circular, chamfer and G2 edge blends before document edits.
use super::*;
use viboceros_document::GeometrySnapshot;
#[cfg(test)]
mod tests;
const USAGE: &str = "FilletEdge radius object-id edge-index[,edge-index...] ... | FilletEdge Radius=radius All=Yes [Corner=Automatic|Ball]";

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
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    replacements: Vec<(ObjectId, Geometry)>,
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
impl EdgeBlendKind {
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

/// Read-only input captured from the original edge indices and owned geometry.
#[derive(Clone, PartialEq)]
pub struct EdgeBlendPlan {
    kind: EdgeBlendKind,
    radius: Real,
    tolerance: Tolerance,
    sources: Vec<(ObjectId, GeometrySnapshot, Vec<usize>)>,
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
    /// Model-space geometry produced by this staged operation.
    pub fn replacements(&self) -> &[(ObjectId, Geometry)] {
        &self.replacements
    }
    pub fn commit(&self, document: &mut Document) -> Result<usize, CommandError> {
        run_command_transaction(document, self.kind.command(), |doc| self.apply(doc))
    }
    pub(crate) fn apply(&self, document: &mut Document) -> Result<usize, CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, snapshot)| {
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
        document.replace_object_geometries(self.replacements.clone())?;
        document.select_objects_direct(
            self.replacements.iter().map(|(id, _)| *id),
            SelectionMode::Replace,
        )?;
        Ok(self.replacements.len())
    }
}

impl EdgeBlendPlan {
    pub fn matches_except_size(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.tolerance == other.tolerance
            && self.sources == other.sources
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
        let mut by_object = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for (count, (id, edge)) in picks.into_iter().enumerate() {
            if count >= 100000 || !document.is_object_selectable(id) {
                return Err(CommandError::Usage(kind.usage()));
            }
            by_object.entry(id).or_default().insert(edge);
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
            if !brep.is_solid() || edges.iter().any(|&edge| edge >= brep.edges().len()) {
                return Err(CommandError::Usage(kind.usage()));
            }
            sources.push((
                id,
                object.geometry_snapshot().clone(),
                edges.into_iter().collect(),
            ));
        }
        Ok(Self {
            kind,
            radius,
            sources,
            tolerance: document.tolerance(),
        })
    }

    pub fn solve(self) -> Result<EdgeBlendSelection, CommandError> {
        let mut sources = Vec::with_capacity(self.sources.len());
        let mut replacements = Vec::with_capacity(self.sources.len());
        for (id, geometry, edges) in self.sources {
            let Geometry::Brep(brep) = &*geometry else {
                unreachable!("captured an edge blend on a B-rep");
            };
            let result = blend(brep, &edges, self.radius, self.kind, self.tolerance)?;
            replacements.push((id, Geometry::Brep(result)));
            sources.push((id, geometry));
        }
        Ok(EdgeBlendSelection {
            kind: self.kind,
            sources,
            replacements,
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
            let rail = match rail {
                crate::chamfer_edge::ChamferRailType::DistanceBetweenRails => {
                    viboceros_smlib::EdgeBlendRailType::DistanceBetweenRails
                }
                crate::chamfer_edge::ChamferRailType::RollingBall => {
                    viboceros_smlib::EdgeBlendRailType::RollingBall
                }
            };
            let solid = if matches!(kind, EdgeBlendKind::Curvature(_)) {
                viboceros_smlib::Solid::blend_brep_edges(&local, edges, radius, rail, tolerance)?
            } else {
                viboceros_smlib::Solid::chamfer_brep_edges(&local, edges, radius, rail, tolerance)?
            };
            solid.to_brep(tolerance)?
        }
    };
    Ok(blended.transformed(
        AffineTransform3::from_translation(zero.vector_to(origin)?),
        tolerance,
    )?)
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
        if arguments
            .iter()
            .filter(|w| corner_option(w).is_none())
            .count()
            > 1
        {
            return Ok(None);
        }
        let mut options = FilletOptions::default();
        options.update(arguments)?;
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
        let mut options = FilletOptions::default();
        let mut positional = Vec::new();
        for word in arguments {
            if let Some(value) = corner_option(word) {
                options.corner = FilletCorner::parse(value)?;
            } else {
                positional.push(*word);
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
        let count =
            FilletEdgeSelection::prepare_fillet(document, picks, options)?.apply(document)?;
        Ok(format!("Filleted edges on {count} object(s)"))
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

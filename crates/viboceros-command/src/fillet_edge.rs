//! Stage owned circular, chamfer and G2 edge blends before document edits.
use super::*;
use viboceros_document::GeometrySnapshot;
#[cfg(test)]
mod tests;
const USAGE: &str =
    "FilletEdge radius object-id edge-index[,edge-index...] ... | FilletEdge Radius=radius All=Yes";

#[derive(Clone, Debug)]
pub struct EdgeBlendSelection {
    kind: EdgeBlendKind,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    replacements: Vec<(ObjectId, Geometry)>,
    tolerance: Tolerance,
}

/// Backward-compatible selected-edge fillet entry point.
pub type FilletEdgeSelection = EdgeBlendSelection;
#[derive(Clone, Copy, Debug)]
enum EdgeBlendKind {
    Fillet,
    Chamfer(crate::chamfer_edge::ChamferRailType),
    Curvature(crate::blend_edge::BlendRailType),
}
impl EdgeBlendKind {
    fn usage(self) -> &'static str {
        match self {
            Self::Fillet => USAGE,
            Self::Chamfer(_) => crate::chamfer_edge::USAGE,
            Self::Curvature(_) => crate::blend_edge::USAGE,
        }
    }
    fn command(self) -> &'static str {
        match self {
            Self::Fillet => "FilletEdge",
            Self::Chamfer(_) => "ChamferEdge",
            Self::Curvature(_) => "BlendEdge",
        }
    }
}
impl EdgeBlendSelection {
    pub fn prepare(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        radius: Real,
    ) -> Result<Self, CommandError> {
        Self::prepare_blend(document, picks, radius, EdgeBlendKind::Fillet)
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
        let mut sources = Vec::new();
        let mut jobs = Vec::new();
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
            sources.push((id, object.geometry_snapshot().clone()));
            jobs.push((id, brep, edges.into_iter().collect::<Vec<_>>()));
        }
        let mut replacements = Vec::new();
        for (id, brep, edges) in jobs {
            replacements.push((
                id,
                Geometry::Brep(blend(brep, &edges, radius, kind, document.tolerance())?),
            ));
        }
        Ok(Self {
            kind,
            sources,
            replacements,
            tolerance: document.tolerance(),
        })
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

fn blend(
    brep: &Brep,
    edges: &[usize],
    radius: Real,
    kind: EdgeBlendKind,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
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
        let solid = match kind {
            EdgeBlendKind::Fillet => {
                viboceros_smlib::Solid::fillet_brep_edges(&local, edges, radius, tolerance)?
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
                if matches!(kind, EdgeBlendKind::Curvature(_)) {
                    viboceros_smlib::Solid::blend_brep_edges(
                        &local, edges, radius, rail, tolerance,
                    )?
                } else {
                    viboceros_smlib::Solid::chamfer_brep_edges(
                        &local, edges, radius, rail, tolerance,
                    )?
                }
            }
        };
        Ok(solid.to_brep(tolerance)?.transformed(
            AffineTransform3::from_translation(zero.vector_to(origin)?),
            tolerance,
        )?)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (brep, edges, radius, tolerance);
        Err(match kind {
            EdgeBlendKind::Fillet => CommandError::FilletEdgeRequiresNativeKernel,
            EdgeBlendKind::Chamfer(rail) => {
                let _ = rail;
                CommandError::ChamferEdgeRequiresNativeKernel
            }
            EdgeBlendKind::Curvature(rail) => {
                let _ = rail;
                CommandError::BlendEdgeRequiresNativeKernel
            }
        })
    }
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
        let radius = if arguments.is_empty() {
            1.
        } else if arguments.len() == 1 {
            parse_radius(arguments[0])?
        } else {
            return Ok(None);
        };
        Ok(Some(ComponentSelectionPrompt {
            command: "FilletEdge",
            kind: ComponentSelectionKind::BrepEdge,
            options: Vec::new(),
            numbers: vec![NumberSelectionOption {
                name: "Radius",
                value: radius,
                minimum: Real::MIN_POSITIVE,
            }],
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let Some(first) = arguments.first() else {
            return Err(CommandError::Usage(USAGE));
        };
        let radius = parse_radius(first)?;
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
        let count = FilletEdgeSelection::prepare(document, picks, radius)?.apply(document)?;
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

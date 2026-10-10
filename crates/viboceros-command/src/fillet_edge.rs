//! Stage exact owned edge fillets before committing any document geometry.
use super::*;
use viboceros_document::GeometrySnapshot;
#[cfg(test)]
mod tests;
const USAGE: &str =
    "FilletEdge radius object-id edge-index[,edge-index...] ... | FilletEdge Radius=radius All=Yes";

#[derive(Clone, Debug)]
pub struct FilletEdgeSelection {
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    replacements: Vec<(ObjectId, Geometry)>,
    tolerance: Tolerance,
}

impl FilletEdgeSelection {
    pub fn prepare(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        radius: Real,
    ) -> Result<Self, CommandError> {
        if !radius.is_finite() || radius <= 0. {
            return Err(CommandError::Usage(USAGE));
        }
        let mut by_object = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for (count, (id, edge)) in picks.into_iter().enumerate() {
            if count >= 100000 || !document.is_object_selectable(id) {
                return Err(CommandError::Usage(USAGE));
            }
            by_object.entry(id).or_default().insert(edge);
        }
        if by_object.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut sources = Vec::new();
        let mut jobs = Vec::new();
        for (id, edges) in by_object {
            let object = document.object(id).ok_or(CommandError::Usage(USAGE))?;
            let Geometry::Brep(brep) = object.geometry() else {
                return Err(CommandError::Usage(USAGE));
            };
            if !brep.is_solid() || edges.iter().any(|&edge| edge >= brep.edges().len()) {
                return Err(CommandError::Usage(USAGE));
            }
            sources.push((id, object.geometry_snapshot().clone()));
            jobs.push((id, brep, edges.into_iter().collect::<Vec<_>>()));
        }
        let mut replacements = Vec::new();
        for (id, brep, edges) in jobs {
            replacements.push((
                id,
                Geometry::Brep(fillet(brep, &edges, radius, document.tolerance())?),
            ));
        }
        Ok(Self {
            sources,
            replacements,
            tolerance: document.tolerance(),
        })
    }
    pub fn commit(&self, document: &mut Document) -> Result<usize, CommandError> {
        run_command_transaction(document, "FilletEdge", |doc| self.apply(doc))
    }
    fn apply(&self, document: &mut Document) -> Result<usize, CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, snapshot)| {
                !document.is_object_selectable(*id)
                    || !document
                        .object(*id)
                        .is_some_and(|object| object.geometry_snapshot() == snapshot)
            })
        {
            return Err(CommandError::Usage(
                "FilletEdge source or tolerance changed; select edges again",
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

fn fillet(
    brep: &Brep,
    edges: &[usize],
    radius: Real,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        Ok(
            viboceros_smlib::Solid::fillet_brep_edges(brep, edges, radius, tolerance)?
                .to_brep(tolerance)?,
        )
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (brep, edges, radius, tolerance);
        Err(CommandError::FilletEdgeRequiresNativeKernel)
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

//! Staged crease edits share immutable source ordinals across scripts and getters.
use crate::*;
use viboceros_document::{GeometrySnapshot, ReplacementHistory};
mod arguments;

const USAGE: &str =
    "Crease|RemoveCrease [Edges=All|0,1,...] [Vertices=All|0,1,...] [Object=selected-uuid]";
const MAX_TARGETS: usize = 500_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreaseMode {
    Add,
    Remove,
}
impl CreaseMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::Add => "Crease",
            Self::Remove => "RemoveCrease",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreaseTarget {
    Edge(ObjectId, usize),
    Vertex(ObjectId, usize),
}
#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    candidate: Geometry,
    processed: bool,
    mesh: bool,
}
#[derive(Clone, Debug)]
pub struct CreaseSelection {
    mode: CreaseMode,
    tolerance: Tolerance,
    sources: Vec<Source>,
}
#[derive(Clone, Debug)]
pub struct CreaseOutcome {
    pub message: String,
    /// Components on these objects were processed and must be released.
    pub processed_objects: Vec<ObjectId>,
}

impl CreaseSelection {
    pub fn prepare(
        document: &Document,
        mode: CreaseMode,
        targets: impl IntoIterator<Item = CreaseTarget>,
    ) -> Result<Self, CommandError> {
        let mut grouped = BTreeMap::<ObjectId, (BTreeSet<usize>, BTreeSet<usize>)>::new();
        for (n, target) in targets.into_iter().enumerate() {
            if n >= MAX_TARGETS {
                return Err(CommandError::CreaseResourceLimit);
            }
            let (id, index, vertex) = match target {
                CreaseTarget::Edge(id, i) => (id, i, false),
                CreaseTarget::Vertex(id, i) => (id, i, true),
            };
            let entry = grouped.entry(id).or_default();
            if vertex {
                entry.1.insert(index);
            } else {
                entry.0.insert(index);
            }
        }
        if grouped.is_empty() {
            return Err(CommandError::CreaseUnavailable);
        }
        let selectable = document
            .selectable_objects()
            .map(|o| (o.id(), o))
            .collect::<BTreeMap<_, _>>();
        let order = document
            .selectable_objects()
            .filter(|o| grouped.contains_key(&o.id()))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if order.len() != grouped.len() {
            return Err(CommandError::CreaseUnavailable);
        }
        let mut sources = Vec::with_capacity(order.len());
        let mut total_vertices = 0_usize;
        let mut total_faces = 0_usize;
        for id in order {
            let object = selectable.get(&id).ok_or(CommandError::CreaseUnavailable)?;
            let (edges, vertices) = grouped.remove(&id).unwrap();
            let edges = edges.into_iter().collect::<Vec<_>>();
            let vertices = vertices.into_iter().collect::<Vec<_>>();
            let (candidate, processed, mesh) = match object.geometry() {
                Geometry::SubD(subd) => {
                    check_indices(&edges, subd.edges().len(), "edge")?;
                    check_indices(&vertices, subd.control_points().len(), "vertex")?;
                    let (edited, changed) =
                        subd.try_crease_components(&edges, &vertices, mode == CreaseMode::Add)?;
                    (Geometry::SubD(edited), changed, false)
                }
                Geometry::Mesh(mesh) if vertices.is_empty() => {
                    let (edited, _) = match mode {
                        CreaseMode::Add => mesh.unwelded_topology_edges(&edges)?,
                        CreaseMode::Remove => mesh.welded_topology_edges(&edges)?,
                    };
                    // Rhino processes selected mesh edges even when no seam changes.
                    (Geometry::Mesh(edited), true, true)
                }
                _ => return Err(CommandError::CreaseUnavailable),
            };
            let (points, faces) = match &candidate {
                Geometry::SubD(s) => (s.control_points().len(), s.faces().len()),
                Geometry::Mesh(m) => (m.vertices().len(), m.face_count()),
                _ => unreachable!(),
            };
            total_vertices = total_vertices
                .checked_add(points)
                .ok_or(CommandError::CreaseResourceLimit)?;
            total_faces = total_faces
                .checked_add(faces)
                .ok_or(CommandError::CreaseResourceLimit)?;
            if total_vertices > 4_000_000 || total_faces > 1_000_000 {
                return Err(CommandError::CreaseResourceLimit);
            }
            sources.push(Source {
                id,
                snapshot: object.geometry_snapshot().clone(),
                candidate,
                processed,
                mesh,
            });
        }
        Ok(Self {
            mode,
            tolerance: document.tolerance(),
            sources,
        })
    }
    pub fn commit(&self, document: &mut Document) -> Result<CreaseOutcome, CommandError> {
        run_command_transaction(document, self.mode.command(), |doc| self.apply(doc))
    }
    fn apply(&self, document: &mut Document) -> Result<CreaseOutcome, CommandError> {
        let selectable = document
            .selectable_objects()
            .map(|o| (o.id(), o.geometry_snapshot()))
            .collect::<BTreeMap<_, _>>();
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|source| {
                selectable
                    .get(&source.id)
                    .is_none_or(|snapshot| *snapshot != &source.snapshot)
            })
        {
            return Err(CommandError::CreaseStale);
        }
        document.replace_object_geometries_preserving_control_selection(
            self.sources
                .iter()
                .filter(|s| s.processed && !s.mesh)
                .map(|s| (s.id, s.candidate.clone())),
        )?;
        document.replace_object_geometries_with_history(
            self.sources
                .iter()
                .filter(|s| s.processed && s.mesh)
                .map(|s| (s.id, s.candidate.clone())),
            ReplacementHistory::EveryReplacement,
        )?;
        let processed_objects = [true, false]
            .into_iter()
            .flat_map(|mesh| {
                self.sources
                    .iter()
                    .filter(move |s| s.processed && s.mesh == mesh)
                    .map(|s| s.id)
            })
            .collect::<Vec<_>>();
        document.move_objects_to_end_in_order(processed_objects.iter().copied())?;
        document.select_objects_direct([], viboceros_document::SelectionMode::Replace)?;
        let message = if processed_objects.is_empty() {
            "No changes made".into()
        } else {
            format!(
                "{} creases {} {} object(s)",
                if self.mode == CreaseMode::Add {
                    "Added"
                } else {
                    "Removed"
                },
                if self.mode == CreaseMode::Add {
                    "to"
                } else {
                    "from"
                },
                processed_objects.len()
            )
        };
        Ok(CreaseOutcome {
            message,
            processed_objects,
        })
    }
}
fn check_indices(
    values: &[usize],
    count: usize,
    component: &'static str,
) -> Result<(), CommandError> {
    if let Some(&index) = values.iter().find(|&&i| i >= count) {
        return Err(CommandError::CreaseComponentOutOfRange {
            component,
            index,
            count,
        });
    }
    Ok(())
}

pub(super) struct CreaseCommand(pub CreaseMode);
impl Command for CreaseCommand {
    fn name(&self) -> &'static str {
        self.0.command()
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let targets = arguments::targets(document, arguments)?;
        CreaseSelection::prepare(document, self.0, targets)?
            .apply(document)
            .map(|out| out.message)
    }
}

//! Mesh and SubD seams share staging, selection and atomic history.
use crate::*;
use viboceros_document::GeometrySnapshot;
mod arguments;

mod mode;
pub use mode::{EdgeSeamMode, SeamMode, VertexSeamMode};

/// Edge or vertex ordinal in an immutable mesh topology or SubD control table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeamTarget {
    pub object: ObjectId,
    pub index: usize,
}
#[derive(Debug)]
struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    candidate: Option<Geometry>,
    subd: bool,
    retained_edges: Vec<usize>,
}
#[derive(Debug)]
pub struct ComponentSeamEdit {
    mode: SeamMode,
    tolerance: Tolerance,
    sources: Vec<Source>,
    source_count: usize,
    selected: usize,
    operations: usize,
    removed: usize,
}
#[derive(Debug)]
pub struct SeamOutcome {
    pub message: String,
    pub processed_objects: Vec<ObjectId>,
    pub released_objects: Vec<ObjectId>,
    pub retained_edges: Vec<SeamTarget>,
}

pub(super) fn available(document: &Document) -> BTreeSet<ObjectId> {
    // Preserve the existing mesh commands' group-expanded selection policy.
    document
        .selectable_objects()
        .chain(document.selected_objects())
        .map(|o| o.id())
        .collect()
}
fn component_count(geometry: &Geometry, mode: SeamMode) -> Result<usize, CommandError> {
    match geometry {
        Geometry::Mesh(m) => Ok(if mode.is_edge() {
            m.topology().edge_count()
        } else {
            m.topology().topological_vertex_count()
        }),
        Geometry::SubD(s) => Ok(if mode.is_edge() {
            s.edges().len()
        } else {
            s.control_points().len()
        }),
        _ => Err(mode.unsupported()),
    }
}
impl ComponentSeamEdit {
    pub fn prepare(
        document: &Document,
        mode: impl Into<SeamMode>,
        targets: impl IntoIterator<Item = SeamTarget>,
    ) -> Result<Self, CommandError> {
        let mode = mode.into();
        let mut grouped = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for (n, target) in targets.into_iter().enumerate() {
            if n >= 500_000 {
                return Err(mode.resource());
            }
            grouped
                .entry(target.object)
                .or_default()
                .insert(target.index);
        }
        if grouped.is_empty() {
            return Err(mode.empty());
        }
        let available = available(document);
        if grouped.keys().any(|id| !available.contains(id)) {
            return Err(mode.stale());
        }
        let mut sources = Vec::with_capacity(grouped.len());
        let mut selected = 0;
        let mut operations = 0;
        let mut removed = 0;
        let mut points = 0_usize;
        let mut faces = 0_usize;
        for object in document.objects().filter(|o| grouped.contains_key(&o.id())) {
            let indices = grouped[&object.id()].iter().copied().collect::<Vec<_>>();
            let count = component_count(object.geometry(), mode)?;
            if let Some(&index) = indices.iter().find(|&&i| i >= count) {
                return Err(mode.index_error(index, count));
            }
            let (candidate, subd, retained_edges) = match object.geometry() {
                Geometry::Mesh(mesh) => {
                    let (edited, count) = mode.mesh_edit(mesh, &indices)?;
                    operations += count;
                    removed += mesh
                        .vertices()
                        .len()
                        .saturating_sub(edited.vertices().len());
                    points = points.saturating_add(edited.vertices().len());
                    faces = faces.saturating_add(edited.face_count());
                    let changed = if mode.weld() {
                        mesh.vertices() != edited.vertices() || mesh.faces() != edited.faces()
                    } else {
                        mesh != &edited
                    };
                    (changed.then_some(Geometry::Mesh(edited)), false, Vec::new())
                }
                Geometry::SubD(subd) => {
                    points = points.saturating_add(subd.control_points().len());
                    faces = faces.saturating_add(subd.faces().len());
                    if mode.is_edge() {
                        let out = if mode.weld() {
                            subd.try_weld_edges_mapped(&indices)?
                        } else {
                            subd.try_unweld_edges_mapped(&indices)?
                        };
                        let retained = indices.iter().map(|&i| out.edge_map[i]).collect();
                        (
                            (out.changed_edges > 0).then_some(Geometry::SubD(out.subd)),
                            true,
                            retained,
                        )
                    } else {
                        let (edited, changed) = subd.try_vertex_seams(&indices, mode.weld())?;
                        (changed.then_some(Geometry::SubD(edited)), true, Vec::new())
                    }
                }
                _ => return Err(mode.unsupported()),
            };
            if points > 4_000_000 || faces > 1_000_000 {
                return Err(mode.resource());
            }
            selected += indices.len();
            sources.push(Source {
                id: object.id(),
                snapshot: object.geometry_snapshot().clone(),
                candidate,
                subd,
                retained_edges,
            });
        }
        Ok(Self {
            mode,
            tolerance: document.tolerance(),
            source_count: sources.len(),
            sources,
            selected,
            operations,
            removed,
        })
    }
    /// Consume the staged outputs so large mesh buffers are moved into the document.
    pub fn commit(self, document: &mut Document) -> Result<SeamOutcome, CommandError> {
        let mode = self.mode;
        run_command_transaction(document, mode.command(), move |doc| self.apply(doc))
    }
    fn apply(self, document: &mut Document) -> Result<SeamOutcome, CommandError> {
        let available = available(document);
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|s| {
                !available.contains(&s.id)
                    || document
                        .object(s.id)
                        .is_none_or(|o| o.geometry_snapshot() != &s.snapshot)
            })
        {
            return Err(self.mode.stale());
        }
        let subd = self.sources.iter().any(|s| s.subd);
        let mut processed = self
            .sources
            .iter()
            .filter(|s| s.candidate.is_some())
            .collect::<Vec<_>>();
        if self.mode.weld() {
            processed.sort_by_key(|s| !s.subd);
        }
        if self.mode.is_edge() && !self.mode.weld() {
            processed.sort_by_key(|s| s.subd);
        }
        let retained_edges = processed
            .iter()
            .flat_map(|s| {
                s.retained_edges.iter().map(move |&index| SeamTarget {
                    object: s.id,
                    index,
                })
            })
            .collect();
        let processed_objects = processed.into_iter().map(|s| s.id).collect::<Vec<_>>();
        if processed_objects.is_empty() && !subd {
            return Err(self.mode.empty());
        }
        let released_objects = self
            .sources
            .iter()
            .filter(|s| s.candidate.is_some() || (s.subd && self.mode.clear_vertices()))
            .map(|s| s.id)
            .collect();
        let message = if subd {
            if processed_objects.is_empty() {
                "No changes made".into()
            } else {
                format!(
                    "Processed {} selected {} in {} changed object(s)",
                    self.selected,
                    if self.mode.is_edge() {
                        "edges"
                    } else {
                        "vertices"
                    },
                    processed_objects.len()
                )
            }
        } else {
            let changed = processed_objects.len();
            let unchanged = self.source_count - changed;
            let selected = self.selected;
            let operations = self.operations;
            self.mode
                .mesh_message(selected, changed, operations, self.removed, unchanged)
        };
        let (mut subds, mut meshes) = (Vec::new(), Vec::new());
        if subd && self.mode.clear_vertices() {
            document
                .release_command_selection_on_history_replay(self.sources.iter().map(|s| s.id))?;
            document.clear_selection();
        }
        for s in self.sources {
            if let Some(geometry) = s.candidate {
                if s.subd {
                    subds.push((s.id, geometry));
                } else {
                    meshes.push((s.id, geometry));
                }
            }
        }
        document.replace_object_geometries_preserving_control_selection(subds)?;
        document.replace_object_geometries(meshes)?;
        if subd {
            document.move_objects_to_end_in_order(processed_objects.iter().copied())?;
        }
        Ok(SeamOutcome {
            message,
            processed_objects,
            released_objects,
            retained_edges,
        })
    }
}

pub(super) use arguments::run;

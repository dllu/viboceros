//! Mesh and SubD vertex seams share staging, selection and atomic history.
use crate::*;
use viboceros_document::GeometrySnapshot;
mod arguments;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexSeamMode {
    Weld,
    Unweld { modify_normals: bool },
}
impl VertexSeamMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::Weld => "WeldVertices",
            Self::Unweld { .. } => "UnweldVertex",
        }
    }
    fn unsupported(self) -> CommandError {
        match self {
            Self::Weld => CommandError::UnsupportedWeldVerticesGeometry,
            Self::Unweld { .. } => CommandError::UnsupportedUnweldVertexGeometry,
        }
    }
    fn empty(self) -> CommandError {
        match self {
            Self::Weld => CommandError::NoSelectedMeshVerticesToWeld,
            Self::Unweld { .. } => CommandError::NoSelectedMeshVerticesToUnweld,
        }
    }
    fn index_error(self, vertex: usize, vertex_count: usize) -> CommandError {
        match self {
            Self::Weld => CommandError::WeldVertexIndexOutOfRange {
                vertex,
                vertex_count,
            },
            Self::Unweld { .. } => CommandError::UnweldVertexIndexOutOfRange {
                vertex,
                vertex_count,
            },
        }
    }
}

/// Ordinal in an immutable mesh topology or SubD control-vertex table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexSeamTarget {
    pub object: ObjectId,
    pub index: usize,
}
#[derive(Debug)]
struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    candidate: Option<Geometry>,
    subd: bool,
}
#[derive(Debug)]
pub struct VertexSeamEdit {
    mode: VertexSeamMode,
    tolerance: Tolerance,
    sources: Vec<Source>,
    source_count: usize,
    selected: usize,
    operations: usize,
    removed: usize,
}
#[derive(Debug)]
pub struct VertexSeamOutcome {
    pub message: String,
    pub processed_objects: Vec<ObjectId>,
    pub released_objects: Vec<ObjectId>,
}

fn available(document: &Document) -> BTreeSet<ObjectId> {
    // Preserve the existing mesh commands' group-expanded selection policy.
    document
        .selectable_objects()
        .chain(document.selected_objects())
        .map(|o| o.id())
        .collect()
}
fn vertex_count(geometry: &Geometry, mode: VertexSeamMode) -> Result<usize, CommandError> {
    match geometry {
        Geometry::Mesh(m) => Ok(m.topology().topological_vertex_count()),
        Geometry::SubD(s) => Ok(s.control_points().len()),
        _ => Err(mode.unsupported()),
    }
}
impl VertexSeamEdit {
    pub fn prepare(
        document: &Document,
        mode: VertexSeamMode,
        targets: impl IntoIterator<Item = VertexSeamTarget>,
    ) -> Result<Self, CommandError> {
        let mut grouped = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for (n, target) in targets.into_iter().enumerate() {
            if n >= 500_000 {
                return Err(CommandError::VertexSeamResourceLimit);
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
            return Err(CommandError::VertexSeamStale);
        }
        let mut sources = Vec::with_capacity(grouped.len());
        let mut selected = 0;
        let mut operations = 0;
        let mut removed = 0;
        let mut points = 0_usize;
        let mut faces = 0_usize;
        for object in document.objects().filter(|o| grouped.contains_key(&o.id())) {
            let indices = grouped[&object.id()].iter().copied().collect::<Vec<_>>();
            let count = vertex_count(object.geometry(), mode)?;
            if let Some(&index) = indices.iter().find(|&&i| i >= count) {
                return Err(mode.index_error(index, count));
            }
            let (candidate, subd) = match object.geometry() {
                Geometry::Mesh(mesh) => {
                    let (edited, count) = match mode {
                        VertexSeamMode::Weld => {
                            mesh.welded_topology_vertices_with_normal_generation(&indices, true)?
                        }
                        VertexSeamMode::Unweld { modify_normals } => mesh
                            .unwelded_topology_vertices_with_normal_update(
                                &indices,
                                modify_normals,
                            )?,
                    };
                    operations += count;
                    removed += mesh
                        .vertices()
                        .len()
                        .saturating_sub(edited.vertices().len());
                    points = points.saturating_add(edited.vertices().len());
                    faces = faces.saturating_add(edited.face_count());
                    let changed = match mode {
                        VertexSeamMode::Weld => {
                            mesh.vertices() != edited.vertices() || mesh.faces() != edited.faces()
                        }
                        VertexSeamMode::Unweld { .. } => mesh != &edited,
                    };
                    (changed.then_some(Geometry::Mesh(edited)), false)
                }
                Geometry::SubD(subd) => {
                    points = points.saturating_add(subd.control_points().len());
                    faces = faces.saturating_add(subd.faces().len());
                    let (edited, changed) =
                        subd.try_vertex_seams(&indices, matches!(mode, VertexSeamMode::Weld))?;
                    (changed.then_some(Geometry::SubD(edited)), true)
                }
                _ => return Err(mode.unsupported()),
            };
            if points > 4_000_000 || faces > 1_000_000 {
                return Err(CommandError::VertexSeamResourceLimit);
            }
            selected += indices.len();
            sources.push(Source {
                id: object.id(),
                snapshot: object.geometry_snapshot().clone(),
                candidate,
                subd,
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
    pub fn commit(self, document: &mut Document) -> Result<VertexSeamOutcome, CommandError> {
        let mode = self.mode;
        run_command_transaction(document, mode.command(), move |doc| self.apply(doc))
    }
    fn apply(self, document: &mut Document) -> Result<VertexSeamOutcome, CommandError> {
        let available = available(document);
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|s| {
                !available.contains(&s.id)
                    || document
                        .object(s.id)
                        .is_none_or(|o| o.geometry_snapshot() != &s.snapshot)
            })
        {
            return Err(CommandError::VertexSeamStale);
        }
        let subd = self.sources.iter().any(|s| s.subd);
        let mut processed = self
            .sources
            .iter()
            .filter(|s| s.candidate.is_some())
            .collect::<Vec<_>>();
        if self.mode == VertexSeamMode::Weld {
            processed.sort_by_key(|s| !s.subd);
        }
        let processed_objects = processed.into_iter().map(|s| s.id).collect::<Vec<_>>();
        if processed_objects.is_empty() && !subd {
            return Err(self.mode.empty());
        }
        let released_objects = self
            .sources
            .iter()
            .filter(|s| s.candidate.is_some() || (s.subd && self.mode == VertexSeamMode::Weld))
            .map(|s| s.id)
            .collect();
        let message = if subd {
            if processed_objects.is_empty() {
                "No changes made".into()
            } else {
                format!(
                    "Processed {} selected vertex/vertices in {} changed mesh/SubD object(s)",
                    self.selected,
                    processed_objects.len()
                )
            }
        } else {
            let changed = processed_objects.len();
            let unchanged = self.source_count - changed;
            let selected = self.selected;
            let operations = self.operations;
            match self.mode {
                VertexSeamMode::Weld => format!(
                    "Processed {selected} selected mesh topology vertex index(es) in {changed} changed mesh(es): welded {operations} incident seam edge(s) and removed {} raw vertex occurrence(s); {unchanged} mesh(es) unchanged",
                    self.removed
                ),
                VertexSeamMode::Unweld { .. } => format!(
                    "Processed {selected} selected mesh topology vertex index(es) in {changed} changed mesh(es): {operations} previously shared vertex set(s) separated; {unchanged} mesh(es) unchanged"
                ),
            }
        };
        let (mut subds, mut meshes) = (Vec::new(), Vec::new());
        if subd && self.mode == VertexSeamMode::Weld {
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
        Ok(VertexSeamOutcome {
            message,
            processed_objects,
            released_objects,
        })
    }
}

pub(super) use arguments::run;

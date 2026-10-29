//! Component-selected mesh and SubD merging, staged before one history admission.
use crate::*;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{MeshMergeComponent, SubDMergeComponent};
mod arguments;
mod targets;
pub use arguments::Parsed;
pub const USAGE: &str = "MergeFaces [Results=Selected|NotSelected] [Faces=All|0,1,...] [Edges=All|0,1,...] [Vertices=All|0,1,...] [Object=selected-uuid]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Results {
    #[default]
    Selected,
    NotSelected,
}
impl Results {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim_start_matches('_') {
            s if s.eq_ignore_ascii_case("Selected") => Some(Self::Selected),
            s if s.eq_ignore_ascii_case("NotSelected") => Some(Self::NotSelected),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Selected => "Selected",
            Self::NotSelected => "NotSelected",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub object: ObjectId,
    pub kind: ComponentSelectionKind,
    pub index: usize,
}
impl Target {
    pub fn eligible(kind: ComponentSelectionKind) -> bool {
        matches!(
            kind,
            ComponentSelectionKind::MeshFace
                | ComponentSelectionKind::MeshEdge
                | ComponentSelectionKind::MeshVertex
                | ComponentSelectionKind::SubDFace
                | ComponentSelectionKind::SubDEdge
                | ComponentSelectionKind::SubDVertex
        )
    }
}
#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    snapshot: GeometrySnapshot,
    candidate: Option<Geometry>,
    selected: Vec<Target>,
    result_faces: Vec<Target>,
    mesh: bool,
}
#[derive(Clone, Debug)]
pub struct MergeSelection {
    tolerance: Tolerance,
    sources: Vec<Source>,
    removed: usize,
    results: Results,
}
#[derive(Clone, Debug)]
pub struct Outcome {
    pub message: String,
    pub released_objects: Vec<ObjectId>,
    pub changed_objects: Vec<ObjectId>,
    pub selected_components: Vec<Target>,
    /// Redo restores result faces; surviving input vertices/edges are transient.
    pub replay_components: Vec<Target>,
    pub removed_faces: usize,
}
impl MergeSelection {
    pub fn prepare(
        document: &Document,
        targets: impl IntoIterator<Item = Target>,
        results: Results,
    ) -> Result<Self, CommandError> {
        let mut grouped = BTreeMap::<ObjectId, BTreeSet<Target>>::new();
        for (n, target) in targets.into_iter().enumerate() {
            if n >= 500_000 {
                return Err(CommandError::MergeFacesResourceLimit);
            }
            if !Target::eligible(target.kind) || !document.is_object_selectable(target.object) {
                return Err(CommandError::MergeFacesUnavailable);
            }
            grouped.entry(target.object).or_default().insert(target);
        }
        if grouped.is_empty() {
            return Err(CommandError::MergeFacesUnavailable);
        }
        let order = document
            .objects()
            .filter(|o| grouped.contains_key(&o.id()))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if order.len() != grouped.len() {
            return Err(CommandError::MergeFacesUnavailable);
        }
        let mut sources = Vec::new();
        let mut removed = 0;
        let mut points = 0usize;
        let mut faces = 0usize;
        for id in order {
            let object = document.object(id).unwrap();
            let selected = grouped.remove(&id).unwrap();
            let mut retained = Vec::new();
            let candidate;
            let result_faces;
            let mesh;
            match object.geometry() {
                Geometry::Mesh(original) => {
                    points = points.saturating_add(original.vertices().len());
                    faces = faces.saturating_add(original.face_count());
                    if points > 4_000_000 || faces > 1_000_000 {
                        return Err(CommandError::MergeFacesResourceLimit);
                    }
                    let components = targets::mesh_components(original, &selected)?;
                    let out = original.merge_face_components_mapped(&components)?;
                    let changed = original != &out.mesh;
                    removed += out.removed_faces;
                    result_faces = out
                        .merged_faces
                        .into_iter()
                        .map(|index| Target {
                            object: id,
                            kind: ComponentSelectionKind::MeshFace,
                            index,
                        })
                        .collect::<Vec<_>>();
                    candidate = changed.then_some(Geometry::Mesh(out.mesh));
                    mesh = true;
                }
                Geometry::SubD(original) => {
                    points = points.saturating_add(original.control_points().len());
                    faces = faces.saturating_add(original.faces().len());
                    if points > 4_000_000 || faces > 1_000_000 {
                        return Err(CommandError::MergeFacesResourceLimit);
                    }
                    let components = targets::subd_components(original, &selected)?;
                    let out = original.merge_face_components(&components)?;
                    removed += out.removed_faces;
                    if out.removed_faces > 0 && results == Results::Selected {
                        for target in &selected {
                            let (before, after) = match target.kind {
                                ComponentSelectionKind::SubDFace => {
                                    (original.face_ids(), out.subd.face_ids())
                                }
                                ComponentSelectionKind::SubDEdge => {
                                    (original.edge_ids(), out.subd.edge_ids())
                                }
                                ComponentSelectionKind::SubDVertex => {
                                    (original.vertex_ids(), out.subd.vertex_ids())
                                }
                                _ => unreachable!(),
                            };
                            if let Ok(index) = after.binary_search(&before[target.index]) {
                                retained.push(Target { index, ..*target });
                            }
                        }
                    }
                    result_faces = out
                        .merged_faces
                        .into_iter()
                        .map(|index| Target {
                            object: id,
                            kind: ComponentSelectionKind::SubDFace,
                            index,
                        })
                        .collect::<Vec<_>>();
                    candidate = (out.removed_faces > 0).then_some(Geometry::SubD(out.subd));
                    mesh = false;
                }
                _ => return Err(CommandError::MergeFacesUnavailable),
            }
            if results == Results::Selected && candidate.is_some() {
                retained.extend(result_faces.iter().copied());
            }
            retained.sort();
            retained.dedup();
            sources.push(Source {
                id,
                snapshot: object.geometry_snapshot().clone(),
                candidate,
                selected: retained,
                result_faces,
                mesh,
            });
        }
        Ok(Self {
            tolerance: document.tolerance(),
            sources,
            removed,
            results,
        })
    }
    pub fn commit(self, document: &mut Document) -> Result<Outcome, CommandError> {
        run_command_transaction(document, "MergeFaces", |doc| self.apply(doc))
    }
    fn apply(self, document: &mut Document) -> Result<Outcome, CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|s| {
                !document.is_object_selectable(s.id)
                    || document
                        .object(s.id)
                        .is_none_or(|o| o.geometry_snapshot() != &s.snapshot)
            })
        {
            return Err(CommandError::MergeFacesStale);
        }
        let released_objects = self.sources.iter().map(|s| s.id).collect::<Vec<_>>();
        document.release_command_selection_on_history_replay(released_objects.iter().copied())?;
        let released = released_objects.iter().copied().collect::<BTreeSet<_>>();
        let retained = document
            .selected_object_ids()
            .filter(|id| !released.contains(id))
            .collect::<Vec<_>>();
        document.select_objects_direct(retained, SelectionMode::Replace)?;
        let ordered = self
            .sources
            .iter()
            .filter(|s| s.candidate.is_some() && s.mesh)
            .chain(
                self.sources
                    .iter()
                    .filter(|s| s.candidate.is_some() && !s.mesh),
            )
            .collect::<Vec<_>>();
        let changed_objects = ordered.iter().map(|s| s.id).collect::<Vec<_>>();
        document.replace_object_geometries(
            ordered
                .iter()
                .map(|s| (s.id, s.candidate.as_ref().unwrap().clone())),
        )?;
        document.move_objects_to_end_in_order(changed_objects.iter().copied())?;
        if !changed_objects.is_empty() {
            document
                .remember_command_result_selection_on_replay(released_objects.iter().copied())?;
        }
        let selected_components = self
            .sources
            .iter()
            .flat_map(|s| s.selected.iter().copied())
            .collect();
        let replay_components = if self.results == Results::Selected {
            ordered
                .iter()
                .flat_map(|s| s.result_faces.iter().copied())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Outcome {
            message: if changed_objects.is_empty() {
                "No changes made".into()
            } else {
                format!(
                    "Merged {} face(s) in {} object(s)",
                    self.removed,
                    changed_objects.len()
                )
            },
            released_objects,
            changed_objects,
            selected_components,
            replay_components,
            removed_faces: self.removed,
        })
    }
}

pub(super) struct MergeFacesCommand(pub std::sync::Arc<remembered::Remembered<Results>>);
impl Command for MergeFacesCommand {
    fn name(&self) -> &'static str {
        "MergeFaces"
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let parsed = Parsed::parse(args, self.0.get())?;
        self.0.set(parsed.results);
        let targets = parsed.targets(document)?;
        MergeSelection::prepare(document, targets, parsed.results)?
            .apply(document)
            .map(|o| o.message)
    }
}
impl CommandRegistry {
    pub fn merge_faces_results(&self) -> Results {
        self.merge_faces_preferences.get()
    }
    pub fn remember_merge_faces_results(&self, value: Results) {
        self.merge_faces_preferences.set(value);
    }
}

//! Transient UI components retain exact source identities, outside Undo.
use super::*;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use std::collections::BTreeMap;
use viboceros_command::ComponentSelectionKind;
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Copy, Debug, Default)]
enum ComponentAction {
    #[default]
    Add,
    Remove,
    Toggle,
}

#[derive(Debug, Default)]
pub(super) struct ComponentSelection {
    history: BTreeMap<viboceros_document::HistoryEntryId, ComponentReplay>,
    picks: BTreeMap<ComponentPick, GeometrySnapshot>,
    order: Vec<ComponentPick>,
    candidates: Vec<(ComponentPick, GeometrySnapshot)>,
    choice_action: ComponentAction,
    hover: Option<ComponentPick>,
}

mod history;
use history::ComponentReplay;

impl ComponentSelection {
    pub(super) fn release_objects(&mut self, ids: &[ObjectId]) {
        let ids = ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        self.picks.retain(|p, _| !ids.contains(&p.object));
        self.order.retain(|p| !ids.contains(&p.object));
        self.clear_choices();
    }
    pub(super) fn release_grip_vertices(&mut self, document: &Document) {
        let owners = document
            .all_control_points()
            .map(|(g, _, _)| g.object)
            .collect::<std::collections::BTreeSet<_>>();
        self.picks.retain(|p, _| {
            !matches!(
                p.kind,
                ComponentSelectionKind::SubDVertex | ComponentSelectionKind::MeshVertex
            ) || !owners.contains(&p.object)
        });
        self.order.retain(|p| self.picks.contains_key(p));
    }
    pub(super) fn has_choices(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub(super) fn valid_picks(&mut self, document: &Document) -> Vec<ComponentPick> {
        if self.picks.is_empty() {
            return Vec::new();
        }
        let snapshots = selectable_snapshots(document);
        self.picks
            .retain(|pick, snapshot| snapshots.get(&pick.object) == Some(snapshot));
        self.order.retain(|pick| self.picks.contains_key(pick));
        self.picks.keys().copied().collect()
    }
    pub(super) fn highlights(&mut self, document: &Document) -> Vec<ComponentPick> {
        if self.picks.is_empty() && self.candidates.is_empty() {
            return Vec::new();
        }
        let snapshots = selectable_snapshots(document);
        let mut picks = self
            .picks
            .iter()
            .filter(|(pick, snapshot)| snapshots.get(&pick.object) == Some(*snapshot))
            .map(|(pick, _)| *pick)
            .collect::<Vec<_>>();
        if let Some(hover) = self.hover.filter(|hover| {
            self.candidates.iter().any(|(pick, snapshot)| {
                pick == hover && snapshots.get(&pick.object) == Some(snapshot)
            })
        }) {
            picks.push(hover);
        } else {
            picks.extend(
                self.candidates
                    .iter()
                    .filter(|(pick, snapshot)| snapshots.get(&pick.object) == Some(snapshot))
                    .map(|(pick, _)| *pick),
            );
        }
        picks.sort();
        picks.dedup();
        picks
    }
    pub(super) fn checked_picks(
        &self,
        document: &Document,
    ) -> Result<Vec<ComponentPick>, &'static str> {
        if self.picks.is_empty() {
            return Ok(Vec::new());
        }
        let snapshots = selectable_snapshots(document);
        if self
            .picks
            .iter()
            .any(|(pick, snapshot)| snapshots.get(&pick.object) != Some(snapshot))
        {
            return Err("component source changed; select the components again");
        }
        Ok(self.order.clone())
    }
    pub(super) fn replace_subdivision_faces(
        &mut self,
        document: &Document,
        faces: Vec<(ObjectId, usize)>,
    ) -> Result<(), &'static str> {
        let picks = faces
            .into_iter()
            .map(|(object, index)| ComponentPick {
                object,
                index,
                kind: ComponentSelectionKind::SubDFace,
            })
            .collect();
        self.clear();
        self.apply(document, picks, ComponentAction::Add)
    }
    pub(super) fn clear(&mut self) {
        self.picks.clear();
        self.order.clear();
        self.clear_choices();
    }
    /// Store fresh witnesses for surviving edge components after a topology edit.
    pub(super) fn retain_subd_edges(
        &mut self,
        document: &Document,
        edges: Vec<(ObjectId, usize)>,
    ) -> Result<(), &'static str> {
        self.apply(
            document,
            edges
                .into_iter()
                .map(|(object, index)| ComponentPick {
                    object,
                    index,
                    kind: ComponentSelectionKind::SubDEdge,
                })
                .collect(),
            ComponentAction::Add,
        )
    }
    pub(super) fn clear_choices(&mut self) {
        self.candidates.clear();
        self.choice_action = ComponentAction::Add;
        self.hover = None;
    }
    fn apply(
        &mut self,
        document: &Document,
        picks: Vec<ComponentPick>,
        action: ComponentAction,
    ) -> Result<(), &'static str> {
        let staged = stage_sources(document, picks)?;
        self.clear_choices();
        for (pick, snapshot) in staged {
            if matches!(action, ComponentAction::Remove)
                || (matches!(action, ComponentAction::Toggle) && self.picks.contains_key(&pick))
            {
                self.picks.remove(&pick);
                self.order.retain(|old| *old != pick);
            } else {
                if !self.picks.contains_key(&pick) {
                    self.order.push(pick);
                }
                self.picks.insert(pick, snapshot);
            }
        }
        Ok(())
    }
}

fn selectable_snapshots(document: &Document) -> BTreeMap<ObjectId, GeometrySnapshot> {
    document
        .selectable_objects()
        .map(|o| (o.id(), o.geometry_snapshot().clone()))
        .collect()
}

fn current(document: &Document, pick: ComponentPick, snapshot: &GeometrySnapshot) -> bool {
    document.is_object_selectable(pick.object)
        && document
            .object(pick.object)
            .is_some_and(|object| object.geometry_snapshot() == snapshot)
}

fn stage_sources(
    document: &Document,
    picks: Vec<ComponentPick>,
) -> Result<Vec<(ComponentPick, GeometrySnapshot)>, &'static str> {
    if picks.is_empty() {
        return Ok(Vec::new());
    }
    let objects = document
        .selectable_objects()
        .map(|o| (o.id(), o))
        .collect::<BTreeMap<_, _>>();
    let mut counts = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut staged = Vec::new();
    for pick in picks.into_iter().filter(|p| seen.insert(*p)) {
        let object = objects
            .get(&pick.object)
            .ok_or("component source is unavailable")?;
        let count = if let Some(&count) = counts.get(&(pick.object, pick.kind)) {
            count
        } else {
            let count = match (pick.kind, object.geometry()) {
                (ComponentSelectionKind::MeshFace, Geometry::Mesh(m)) => m.face_count(),
                (ComponentSelectionKind::MeshVertex, Geometry::Mesh(m)) => {
                    m.topology_vertex_points().len()
                }
                (ComponentSelectionKind::MeshEdge, Geometry::Mesh(m)) => {
                    m.topology_edge_points().len()
                }
                (ComponentSelectionKind::SubDFace, Geometry::SubD(s)) => s.faces().len(),
                (ComponentSelectionKind::SubDEdge, Geometry::SubD(s)) => s.edges().len(),
                (ComponentSelectionKind::SubDVertex, Geometry::SubD(s)) => s.control_points().len(),
                (ComponentSelectionKind::BrepEdge, Geometry::Brep(b)) => b.edges().len(),
                (ComponentSelectionKind::BrepFace, Geometry::Brep(b)) => b.faces().len(),
                (ComponentSelectionKind::BrepEdge, Geometry::NurbsSurface(s)) => {
                    viboceros_geometry::Brep::try_surface_face(s.clone(), document.tolerance())
                        .map_err(|_| "invalid component surface")?
                        .edges()
                        .len()
                }
                (ComponentSelectionKind::BrepFace, Geometry::NurbsSurface(surface)) => {
                    viboceros_geometry::Brep::try_surface_face(
                        surface.clone(),
                        document.tolerance(),
                    )
                    .map_err(|_| "invalid component surface")?
                    .faces()
                    .len()
                }
                _ => return Err("component kind does not match its source"),
            };
            counts.insert((pick.object, pick.kind), count);
            count
        };
        if pick.index >= count {
            return Err("component index is outside its source");
        }
        staged.push((pick, object.geometry_snapshot().clone()));
    }
    Ok(staged)
}

mod dispatch;

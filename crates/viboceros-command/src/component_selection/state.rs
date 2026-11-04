//! Component witnesses and action order shared by GUI and headless commands.
use super::*;
use std::sync::{Arc, Mutex, MutexGuard};
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentPick {
    pub object: ObjectId,
    pub kind: ComponentSelectionKind,
    pub index: usize,
}
#[derive(Clone, Copy, Debug, Default)]
pub enum ComponentAction {
    #[default]
    Add,
    Remove,
    Toggle,
}
#[derive(Debug, Default)]
struct Stored {
    picks: BTreeMap<ComponentPick, GeometrySnapshot>,
    order: Vec<ComponentPick>,
}
#[derive(Clone, Debug, Default)]
pub struct ComponentSelection(Arc<Mutex<Stored>>);
impl ComponentSelection {
    fn stored(&self) -> MutexGuard<'_, Stored> {
        self.0
            .lock()
            .expect("component locks never invoke user code")
    }
    pub fn clear(&self) {
        *self.stored() = Stored::default();
    }
    pub fn len(&self) -> usize {
        self.stored().picks.len()
    }
    pub fn is_empty(&self) -> bool {
        self.stored().picks.is_empty()
    }
    pub fn contains(&self, pick: &ComponentPick) -> bool {
        self.stored().picks.contains_key(pick)
    }
    pub fn release_objects(&self, ids: &[ObjectId]) {
        let ids = ids.iter().copied().collect::<BTreeSet<_>>();
        let mut s = self.stored();
        s.picks.retain(|p, _| !ids.contains(&p.object));
        s.order.retain(|p| !ids.contains(&p.object));
    }
    pub fn release_grip_vertices(&self, doc: &Document) {
        let owners = doc
            .all_control_points()
            .map(|(g, _, _)| g.object)
            .collect::<BTreeSet<_>>();
        let mut s = self.stored();
        s.picks.retain(|p, _| {
            !matches!(
                p.kind,
                ComponentSelectionKind::SubDVertex | ComponentSelectionKind::MeshVertex
            ) || !owners.contains(&p.object)
        });
        let Stored { picks, order } = &mut *s;
        order.retain(|p| picks.contains_key(p));
    }
    pub fn valid_picks(&self, doc: &Document) -> Vec<ComponentPick> {
        if self.is_empty() {
            return Vec::new();
        }
        let snapshots = selectable_snapshots(doc);
        let mut s = self.stored();
        s.picks.retain(|p, w| snapshots.get(&p.object) == Some(w));
        let Stored { picks, order } = &mut *s;
        order.retain(|p| picks.contains_key(p));
        picks.keys().copied().collect()
    }
    pub fn live_picks(&self, doc: &Document) -> Vec<ComponentPick> {
        if self.is_empty() {
            return Vec::new();
        }
        let snapshots = selectable_snapshots(doc);
        self.stored()
            .picks
            .iter()
            .filter(|(p, w)| snapshots.get(&p.object) == Some(*w))
            .map(|(p, _)| *p)
            .collect()
    }
    pub fn checked_picks(&self, doc: &Document) -> Result<Vec<ComponentPick>, &'static str> {
        if self.is_empty() {
            return Ok(Vec::new());
        }
        let snapshots = selectable_snapshots(doc);
        let s = self.stored();
        if s.picks
            .iter()
            .any(|(p, w)| snapshots.get(&p.object) != Some(w))
        {
            return Err("component source changed; select the components again");
        }
        Ok(s.order.clone())
    }
    pub fn replace(&self, doc: &Document, picks: Vec<ComponentPick>) -> Result<(), &'static str> {
        let staged = stage_sources(doc, picks)?;
        let mut s = Stored::default();
        for (p, w) in staged {
            s.order.push(p);
            s.picks.insert(p, w);
        }
        *self.stored() = s;
        Ok(())
    }
    pub fn apply(
        &self,
        doc: &Document,
        picks: Vec<ComponentPick>,
        action: ComponentAction,
    ) -> Result<(), &'static str> {
        let staged = stage_sources(doc, picks)?;
        let mut s = self.stored();
        let mut removed = BTreeSet::new();
        for (p, w) in staged {
            if matches!(action, ComponentAction::Remove)
                || (matches!(action, ComponentAction::Toggle) && s.picks.contains_key(&p))
            {
                s.picks.remove(&p);
                removed.insert(p);
            } else {
                if !s.picks.contains_key(&p) {
                    s.order.push(p);
                }
                s.picks.insert(p, w);
            }
        }
        if !removed.is_empty() {
            s.order.retain(|p| !removed.contains(p));
        }
        Ok(())
    }
    pub fn order_edges(&self, owner: ObjectId, kind: ComponentSelectionKind) {
        let mut s = self.stored();
        let mut edges = s
            .order
            .iter()
            .filter(|p| p.object == owner && p.kind == kind)
            .copied()
            .collect::<Vec<_>>();
        edges.sort_unstable_by_key(|p| std::cmp::Reverse(p.index));
        let mut edges = edges.into_iter();
        for p in &mut s.order {
            if p.object == owner && p.kind == kind {
                *p = edges.next().unwrap();
            }
        }
    }
}
fn selectable_snapshots(document: &Document) -> BTreeMap<ObjectId, GeometrySnapshot> {
    document
        .selectable_objects()
        .map(|o| (o.id(), o.geometry_snapshot().clone()))
        .collect()
}
pub fn current(document: &Document, pick: ComponentPick, snapshot: &GeometrySnapshot) -> bool {
    document.is_object_selectable(pick.object)
        && document
            .object(pick.object)
            .is_some_and(|o| o.geometry_snapshot() == snapshot)
}
pub fn stage_sources(
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

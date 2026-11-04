//! Mesh/SubD loop and ring seed selections stay outside model history.
use super::*;
use component_selection::{ComponentAction, ComponentPick, ComponentSelection, stage_sources};
use std::sync::Arc;
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Loop,
    Ring,
    FaceLoop,
}
impl Pattern {
    pub fn from_command(name: &str) -> Option<Self> {
        [Self::Loop, Self::Ring, Self::FaceLoop]
            .into_iter()
            .find(|p| {
                p.command()
                    .eq_ignore_ascii_case(name.trim_start_matches(['_', '-']))
            })
    }
    pub fn command(self) -> &'static str {
        match self {
            Self::Loop => "SelEdgeLoop",
            Self::Ring => "SelEdgeRing",
            Self::FaceLoop => "SelFaceLoop",
        }
    }
    pub fn completed_message(self, picks: &[ComponentPick]) -> String {
        if self == Self::FaceLoop {
            let edges = picks.iter().filter(|p| eligible(**p)).count();
            format!(
                "Selected {} face(s) and {edges} seed edge(s)",
                picks
                    .iter()
                    .filter(|p| matches!(
                        p.kind,
                        ComponentSelectionKind::MeshFace | ComponentSelectionKind::SubDFace
                    ))
                    .count()
            )
        } else {
            format!(
                "Selected {} edge(s)",
                picks.iter().filter(|p| eligible(**p)).count()
            )
        }
    }
    fn output_kind(self, seed: ComponentPick) -> ComponentSelectionKind {
        if self == Self::FaceLoop {
            match seed.kind {
                ComponentSelectionKind::MeshEdge => ComponentSelectionKind::MeshFace,
                ComponentSelectionKind::SubDEdge => ComponentSelectionKind::SubDFace,
                _ => unreachable!(),
            }
        } else {
            seed.kind
        }
    }
    fn usage(self) -> &'static str {
        match self {
            Self::Loop => "SelEdgeLoop [Edges=0,1,...|All Object=uuid]",
            Self::Ring => "SelEdgeRing [Edges=0,1,...|All Object=uuid]",
            Self::FaceLoop => "SelFaceLoop [Edges=0,1,...|All Object=uuid]",
        }
    }
    fn path(self, geometry: &Geometry, seed: usize) -> Result<Vec<usize>, CommandError> {
        Ok(match (self, geometry) {
            (Self::Loop, Geometry::Mesh(m)) => m.try_edge_loop(seed)?.edges,
            (Self::Loop, Geometry::SubD(s)) => s
                .try_edge_loop(
                    seed,
                    viboceros_geometry::SubDEdgeChainType::EqualEdgeTagAndOrdinary,
                )?
                .edges
                .into_iter()
                .map(|e| e.index)
                .collect(),
            (Self::Ring, Geometry::Mesh(m)) => m.try_edge_ring(seed)?.edges,
            (Self::Ring, Geometry::SubD(s)) => s.try_edge_ring(seed)?.edges,
            (Self::FaceLoop, Geometry::Mesh(m)) => m.try_face_loop(seed)?.faces,
            (Self::FaceLoop, Geometry::SubD(s)) => s.try_face_loop(seed)?.faces,
            _ => return Err(CommandError::EdgeLoopUnavailable),
        })
    }
}
const MAX_SEEDS: usize = 100_000;
const MAX_COMPONENTS: usize = 400_000;
pub fn eligible(p: ComponentPick) -> bool {
    matches!(
        p.kind,
        ComponentSelectionKind::MeshEdge | ComponentSelectionKind::SubDEdge
    )
}

#[derive(Debug, Default)]
pub struct Session {
    pattern: Pattern,
    ring_indices: BTreeMap<ObjectId, Arc<viboceros_geometry::PolygonEdgeRingIndex>>,
    seeds: BTreeSet<ComponentPick>,
    sources: BTreeMap<ObjectId, GeometrySnapshot>,
    paths: BTreeMap<ComponentPick, Arc<[ComponentPick]>>,
    preview: Vec<ComponentPick>,
}
impl Session {
    pub fn new(pattern: Pattern) -> Self {
        Self {
            pattern,
            ..Default::default()
        }
    }
    pub fn pattern(&self) -> Pattern {
        self.pattern
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.sources.iter().all(|(&id, w)| {
            doc.is_object_selectable(id)
                && doc.object(id).is_some_and(|o| o.geometry_snapshot() == w)
        })
    }
    pub fn seeds(&self) -> Vec<ComponentPick> {
        self.seeds.iter().copied().collect()
    }
    pub fn preview(&self) -> &[ComponentPick] {
        &self.preview
    }
    pub fn pick(
        &mut self,
        doc: &Document,
        picks: &[ComponentPick],
        action: ComponentAction,
    ) -> Result<(), CommandError> {
        if !self.current(doc) {
            return Err(CommandError::EdgeLoopStale);
        }
        if picks.len() > MAX_SEEDS || picks.iter().any(|p| !eligible(*p)) {
            return Err(CommandError::EdgeLoopUnavailable);
        }
        let staged =
            stage_sources(doc, picks.to_vec()).map_err(|_| CommandError::EdgeLoopUnavailable)?;
        let mut seeds = self.seeds.clone();
        let mut paths = BTreeMap::<ComponentPick, Arc<[ComponentPick]>>::new();
        let mut sources = BTreeMap::new();
        let mut ring_indices = BTreeMap::new();
        for (p, w) in staged {
            let remove = matches!(action, ComponentAction::Remove)
                || matches!(action, ComponentAction::Toggle) && seeds.contains(&p);
            if remove {
                seeds.remove(&p);
                continue;
            }
            if !self.paths.contains_key(&p) && !paths.contains_key(&p) {
                let geometry = doc.object(p.object).unwrap().geometry();
                let (indices, aliases) = if self.pattern != Pattern::Loop {
                    let index = if let Some(i) = self.ring_indices.get(&p.object) {
                        i
                    } else {
                        ring_indices.entry(p.object).or_insert_with(|| {
                            Arc::new(match geometry {
                                Geometry::Mesh(m) => m.edge_ring_index(),
                                Geometry::SubD(s) => s.edge_ring_index(),
                                _ => unreachable!(),
                            })
                        })
                    };
                    if self.pattern == Pattern::FaceLoop {
                        let path = index.try_face_loop(p.index)?;
                        (path.faces, path.edges)
                    } else {
                        let path = index.try_ring(p.index)?;
                        (path.edges.clone(), path.edges)
                    }
                } else {
                    let edges = self.pattern.path(geometry, p.index)?;
                    (edges.clone(), edges)
                };
                if indices
                    .len()
                    .saturating_add(paths.len())
                    .saturating_add(self.paths.len())
                    > MAX_COMPONENTS
                {
                    return Err(CommandError::EdgeLoopResourceLimit);
                }
                let path: Arc<[ComponentPick]> = indices
                    .into_iter()
                    .map(|index| ComponentPick {
                        index,
                        kind: self.pattern.output_kind(p),
                        ..p
                    })
                    .collect::<Vec<_>>()
                    .into();
                paths.insert(p, path.clone());
                for index in aliases {
                    paths.insert(ComponentPick { index, ..p }, path.clone());
                }
                sources.insert(p.object, w);
            }
            seeds.insert(p);
        }
        if seeds.len() > MAX_SEEDS {
            return Err(CommandError::EdgeLoopResourceLimit);
        }
        self.sources.extend(sources);
        self.ring_indices.extend(ring_indices);
        self.paths.extend(paths);
        self.seeds = seeds;
        let mut seen = BTreeSet::new();
        let mut selected = BTreeSet::new();
        for seed in &self.seeds {
            let path = &self.paths[seed];
            if seen.insert(Arc::as_ptr(path) as *const () as usize) {
                selected.extend(path.iter().copied());
            }
        }
        self.preview = selected.into_iter().collect();
        self.sort(doc);
        Ok(())
    }
    fn sort(&mut self, doc: &Document) {
        let rank = doc
            .objects()
            .enumerate()
            .map(|(i, o)| (o.id(), i))
            .collect::<BTreeMap<_, _>>();
        self.preview
            .sort_by_key(|p| (rank[&p.object], std::cmp::Reverse(p.index)));
    }
    pub fn finish(&self, doc: &Document) -> Result<Vec<ComponentPick>, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::EdgeLoopStale);
        }
        let mut selected = self.preview.clone();
        if self.pattern == Pattern::FaceLoop {
            selected.extend(
                self.seeds
                    .iter()
                    .filter(|p| !self.paths[*p].is_empty())
                    .copied(),
            );
            if selected.len() > MAX_COMPONENTS {
                return Err(CommandError::EdgeLoopResourceLimit);
            }
            let rank = doc
                .objects()
                .enumerate()
                .map(|(i, o)| (o.id(), i))
                .collect::<BTreeMap<_, _>>();
            selected.sort_by_key(|p| (rank[&p.object], !eligible(*p), std::cmp::Reverse(p.index)));
        }
        Ok(selected)
    }
}

mod command;
pub(super) use command::SelEdgePatternCommand;

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
}
impl Pattern {
    pub fn command(self) -> &'static str {
        match self {
            Self::Loop => "SelEdgeLoop",
            Self::Ring => "SelEdgeRing",
        }
    }
    fn usage(self) -> &'static str {
        match self {
            Self::Loop => "SelEdgeLoop [Edges=0,1,...|All Object=uuid]",
            Self::Ring => "SelEdgeRing [Edges=0,1,...|All Object=uuid]",
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
                let indices = if self.pattern == Pattern::Ring {
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
                    index.try_ring(p.index)?.edges
                } else {
                    self.pattern.path(geometry, p.index)?
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
                    .map(|index| ComponentPick { index, ..p })
                    .collect::<Vec<_>>()
                    .into();
                paths.insert(p, path.clone());
                for &edge in path.iter() {
                    paths.insert(edge, path.clone());
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
        Ok(self.preview.clone())
    }
}

pub(super) struct SelEdgePatternCommand {
    pattern: Pattern,
    selection: ComponentSelection,
}
impl SelEdgePatternCommand {
    pub fn new(selection: ComponentSelection, pattern: Pattern) -> Self {
        Self { selection, pattern }
    }
}
impl Command for SelEdgePatternCommand {
    fn name(&self) -> &'static str {
        self.pattern.command()
    }
    fn records_history(&self) -> bool {
        false
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        if !args.is_empty() {
            return Err(CommandError::Usage(self.pattern.usage()));
        }
        Ok(Some(ComponentSelectionPrompt {
            command: self.pattern.command(),
            kind: ComponentSelectionKind::MeshEdge,
            options: Vec::new(),
            numbers: Vec::new(),
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let targets = if args.is_empty() {
            self.selection
                .checked_picks(doc)
                .map_err(|_| CommandError::EdgeLoopStale)?
                .into_iter()
                .filter(|p| eligible(*p))
                .collect()
        } else {
            selectors(doc, args, self.pattern.usage())?
        };
        let mut session = Session::new(self.pattern);
        session.pick(doc, &targets, ComponentAction::Add)?;
        let selected = session.finish(doc)?;
        let count = selected.len();
        let owners = selected.iter().map(|p| p.object).collect::<BTreeSet<_>>();
        self.selection
            .replace(doc, selected)
            .map_err(|_| CommandError::EdgeLoopStale)?;
        doc.select_objects_direct(owners, viboceros_document::SelectionMode::Remove)?;
        Ok(format!("Selected {count} edge(s)"))
    }
}
fn selectors(
    doc: &Document,
    args: &[&str],
    usage: &'static str,
) -> Result<Vec<ComponentPick>, CommandError> {
    let (mut edges, mut object) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let (name, value, n) = orient_option(args, i, usage)?;
        let slot = if option_name_eq(name, "Edges") || option_name_eq(name, "Edge") {
            &mut edges
        } else if option_name_eq(name, "Object") {
            &mut object
        } else {
            return Err(CommandError::Usage(usage));
        };
        if slot.replace(value).is_some() {
            return Err(CommandError::Usage(usage));
        }
        i += n;
    }
    let edges = edges.ok_or(CommandError::Usage(usage))?;
    let id = if let Some(id) = object {
        id.parse::<ObjectId>()
            .map_err(|_| CommandError::EdgeLoopUnavailable)?
    } else {
        let mut objects = doc.selected_object_ids();
        let id = objects.next().ok_or(CommandError::NoObjectsSelected)?;
        if objects.next().is_some() {
            return Err(CommandError::EdgeLoopUnavailable);
        }
        id
    };
    let obj = doc
        .object(id)
        .filter(|_| doc.is_object_selectable(id))
        .ok_or(CommandError::EdgeLoopUnavailable)?;
    let (kind, count) = match obj.geometry() {
        Geometry::Mesh(m) => (ComponentSelectionKind::MeshEdge, m.topology().edge_count()),
        Geometry::SubD(s) => (ComponentSelectionKind::SubDEdge, s.edges().len()),
        _ => return Err(CommandError::EdgeLoopUnavailable),
    };
    let indices = if option_name_eq(edges, "All") {
        if count > MAX_SEEDS {
            return Err(CommandError::EdgeLoopResourceLimit);
        }
        (0..count).collect::<Vec<_>>()
    } else {
        if edges.split(',').count() > MAX_SEEDS {
            return Err(CommandError::EdgeLoopResourceLimit);
        }
        edges
            .split(',')
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| CommandError::EdgeLoopUnavailable)
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(indices
        .into_iter()
        .map(|index| ComponentPick {
            object: id,
            kind,
            index,
        })
        .collect())
}

//! SelEdgeLoop stages seed picks and previews, independently of model history.
use super::*;
use component_selection::{ComponentAction, ComponentPick, ComponentSelection, stage_sources};
use std::sync::Arc;
use viboceros_document::GeometrySnapshot;

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
    seeds: BTreeSet<ComponentPick>,
    sources: BTreeMap<ObjectId, GeometrySnapshot>,
    paths: BTreeMap<ComponentPick, Arc<[ComponentPick]>>,
    preview: Vec<ComponentPick>,
}
impl Session {
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
        for (p, w) in staged {
            let remove = matches!(action, ComponentAction::Remove)
                || matches!(action, ComponentAction::Toggle) && seeds.contains(&p);
            if remove {
                seeds.remove(&p);
                continue;
            }
            if !self.paths.contains_key(&p) && !paths.contains_key(&p) {
                let indices = match doc.object(p.object).unwrap().geometry() {
                    Geometry::Mesh(m) => m.try_edge_loop(p.index)?.edges,
                    Geometry::SubD(s) => s
                        .try_edge_loop(
                            p.index,
                            viboceros_geometry::SubDEdgeChainType::EqualEdgeTagAndOrdinary,
                        )?
                        .edges
                        .into_iter()
                        .map(|e| e.index)
                        .collect(),
                    _ => return Err(CommandError::EdgeLoopUnavailable),
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

pub(super) struct SelEdgeLoopCommand {
    selection: ComponentSelection,
}
impl SelEdgeLoopCommand {
    pub fn new(selection: ComponentSelection) -> Self {
        Self { selection }
    }
}
impl Command for SelEdgeLoopCommand {
    fn name(&self) -> &'static str {
        "SelEdgeLoop"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        if !args.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Some(ComponentSelectionPrompt {
            command: "SelEdgeLoop",
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
            selectors(doc, args)?
        };
        let mut session = Session::default();
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
const USAGE: &str = "SelEdgeLoop [Edges=0,1,...|All Object=uuid]";
fn selectors(doc: &Document, args: &[&str]) -> Result<Vec<ComponentPick>, CommandError> {
    let (mut edges, mut object) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let (name, value, n) = orient_option(args, i, USAGE)?;
        let slot = if option_name_eq(name, "Edges") || option_name_eq(name, "Edge") {
            &mut edges
        } else if option_name_eq(name, "Object") {
            &mut object
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if slot.replace(value).is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        i += n;
    }
    let edges = edges.ok_or(CommandError::Usage(USAGE))?;
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

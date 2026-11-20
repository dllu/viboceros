//! Source-safe mixed curve and B-rep edge chains, outside model history.
use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{
    ChainContinuation, ChainContinuity, ChainDirection, CurveChainIndex, CurveChainOptions,
};
mod arguments;
mod growth;
type Preferences = std::sync::Arc<remembered::Remembered<Option<CurveChainOptions>>>;
pub use arguments::{defaults, parse_options};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Curve(ObjectId),
    Edge(ObjectId, usize),
}
impl Target {
    pub fn object(self) -> ObjectId {
        match self {
            Self::Curve(id) | Self::Edge(id, _) => id,
        }
    }
}
#[derive(Clone, Debug, Default)]
struct State {
    selected: BTreeSet<usize>,
    fronts: [Option<(usize, bool)>; 2],
    preferred: Option<u8>,
}
#[derive(Clone, Debug)]
struct UndoFrame {
    added: usize,
    placement: Option<(usize, bool)>,
    fronts: [Option<(usize, bool)>; 2],
    preferred: Option<u8>,
}
impl UndoFrame {
    fn new(before: &State, added: usize, placement: Option<(usize, bool)>) -> Self {
        Self {
            added,
            placement,
            fronts: before.fronts,
            preferred: before.preferred,
        }
    }
}
#[derive(Clone, Debug)]
struct Pending {
    state: State,
    journal: Vec<UndoFrame>,
    candidates: Vec<ChainContinuation>,
    one: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Mixed,
    EdgeBlend(Option<ObjectId>),
}
impl Scope {
    fn includes(self, id: ObjectId, geometry: &Geometry) -> bool {
        if matches!(self, Self::EdgeBlend(Some(owner)) if owner != id) {
            return false;
        }
        matches!(geometry, Geometry::Brep(_))
            || self == Self::Mixed
                && (geometry.curve_ref().is_some() || matches!(geometry, Geometry::NurbsSurface(_)))
    }
}
#[derive(Clone, Debug)]
pub struct Session {
    scope: Scope,
    options: CurveChainOptions,
    index: CurveChainIndex,
    targets: Vec<Target>,
    lookup: BTreeMap<Target, usize>,
    classes: Vec<u8>,
    sources: BTreeMap<ObjectId, GeometrySnapshot>,
    tolerance: Tolerance,
    baseline_whole: Vec<ObjectId>,
    baseline_components: Vec<ComponentPick>,
    baseline_sources: BTreeMap<ObjectId, GeometrySnapshot>,
    state: State,
    undo: Vec<UndoFrame>,
    pending: Option<Pending>,
}
impl Session {
    pub fn new(
        doc: &Document,
        selection: &component_selection::ComponentSelection,
        options: CurveChainOptions,
    ) -> Result<Self, CommandError> {
        Self::with_selection(doc, Some(selection), options, Scope::Mixed, &[])
    }
    pub fn new_for_measurement(
        doc: &Document,
        options: CurveChainOptions,
    ) -> Result<Self, CommandError> {
        Self::with_selection(doc, None, options, Scope::Mixed, &[])
    }
    pub fn new_for_edge_blend(
        doc: &Document,
        selection: &component_selection::ComponentSelection,
        options: CurveChainOptions,
    ) -> Result<Self, CommandError> {
        Self::new_for_edge_blend_on(doc, selection, options, None)
    }
    pub fn new_for_edge_blend_on(
        doc: &Document,
        selection: &component_selection::ComponentSelection,
        options: CurveChainOptions,
        owner: Option<ObjectId>,
    ) -> Result<Self, CommandError> {
        Self::with_selection(doc, Some(selection), options, Scope::EdgeBlend(owner), &[])
    }
    /// Build a chain picker that only returns geometry and never changes
    /// document or component selection. Existing rail pieces are excluded.
    pub fn new_for_rail(
        doc: &Document,
        options: CurveChainOptions,
        excluded: &[Target],
    ) -> Result<Self, CommandError> {
        Self::with_selection(doc, None, options, Scope::Mixed, excluded)
    }

    fn with_selection(
        doc: &Document,
        selection: Option<&component_selection::ComponentSelection>,
        options: CurveChainOptions,
        scope: Scope,
        excluded: &[Target],
    ) -> Result<Self, CommandError> {
        options.validate()?;
        let mut sources = BTreeMap::new();
        let mut curves = Vec::new();
        let mut targets = Vec::new();
        let excluded = excluded.iter().copied().collect::<BTreeSet<_>>();
        let mut classes = Vec::new();
        for object in doc.selectable_objects() {
            let geo = object.geometry();
            let id = object.id();
            if !scope.includes(id, geo) {
                continue;
            }
            sources.insert(id, object.geometry_snapshot().clone());
            if let Some(curve) = geo.curve_ref() {
                if excluded.contains(&Target::Curve(id)) {
                    continue;
                }
                curves.push(curve.to_owned());
                targets.push(Target::Curve(id));
                classes.push(0);
            } else {
                let brep = match geo {
                    Geometry::Brep(b) => std::borrow::Cow::Borrowed(b),
                    Geometry::NurbsSurface(s) => std::borrow::Cow::Owned(
                        viboceros_geometry::Brep::try_surface_face(s.clone(), doc.tolerance())?,
                    ),
                    _ => continue,
                };
                let joined = if matches!(scope, Scope::EdgeBlend(_)) {
                    if !brep.is_solid() {
                        continue;
                    }
                    Some(brep.edges_shared_by_distinct_faces())
                } else {
                    None
                };
                for (i, edge) in brep.edges().iter().enumerate() {
                    if excluded.contains(&Target::Edge(id, i))
                        || joined.as_ref().is_some_and(|shared| !shared[i])
                    {
                        continue;
                    }
                    curves.push(viboceros_geometry::Curve3::NurbsCurve(edge.curve().clone()));
                    targets.push(Target::Edge(id, i));
                    classes.push(1);
                }
            }
        }
        let (baseline_whole, baseline_components) = if let Some(selection) = selection {
            (
                doc.selected_object_ids().collect(),
                selection
                    .checked_picks(doc)
                    .map_err(|_| CommandError::ChainStale)?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let baseline_sources = baseline_components
            .iter()
            .map(|p| {
                (
                    p.object,
                    doc.object(p.object).unwrap().geometry_snapshot().clone(),
                )
            })
            .collect();
        let index = CurveChainIndex::new(&curves)?;
        let lookup = targets.iter().enumerate().map(|(i, &t)| (t, i)).collect();
        Ok(Self {
            scope,
            options,
            index,
            targets,
            lookup,
            classes,
            sources,
            tolerance: doc.tolerance(),
            baseline_whole,
            baseline_components,
            baseline_sources,
            state: State::default(),
            undo: Vec::new(),
            pending: None,
        })
    }
    pub fn options(&self) -> CurveChainOptions {
        self.options
    }
    pub fn set_options(&mut self, options: CurveChainOptions) -> Result<(), CommandError> {
        options.validate()?;
        self.options = options;
        self.pending = None;
        Ok(())
    }
    pub fn current(&self, doc: &Document) -> bool {
        if self.tolerance != doc.tolerance() {
            return false;
        }
        let now = doc
            .selectable_objects()
            .filter(|o| self.scope.includes(o.id(), o.geometry()))
            .map(|o| (o.id(), o.geometry_snapshot()))
            .collect::<BTreeMap<_, _>>();
        self.baseline_sources.iter().all(|(&id, w)| {
            doc.is_object_selectable(id)
                && doc.object(id).is_some_and(|o| o.geometry_snapshot() == w)
        }) && now.len() == self.sources.len()
            && self.sources.iter().all(|(id, w)| now.get(id) == Some(&w))
    }
    pub fn targets(&self) -> Vec<Target> {
        self.state
            .selected
            .iter()
            .map(|&i| self.targets[i])
            .collect()
    }
    /// Accepted pieces in traversal order. The boolean reverses the piece
    /// relative to its stored direction. The seed retains its direction.
    pub fn ordered_targets(&self) -> Vec<(Target, bool)> {
        let mut path = VecDeque::with_capacity(self.undo.len());
        for item in &self.undo {
            let target = self.targets[item.added];
            match item.placement {
                Some((0, reversed)) => path.push_front((target, !reversed)),
                Some((_, reversed)) => path.push_back((target, reversed)),
                None => path.push_back((target, false)),
            }
        }
        path.into()
    }

    fn validate(&self, doc: &Document) -> Result<(), CommandError> {
        if self.current(doc) {
            Ok(())
        } else {
            Err(CommandError::ChainStale)
        }
    }
    pub fn has_seed(&self) -> bool {
        !self.state.selected.is_empty()
    }
    pub fn contains(&self, target: Target) -> bool {
        self.lookup.contains_key(&target)
    }
    /// Native object getters ignore whole curves already selected at startup.
    pub fn pick_input(&mut self, doc: &Document, target: Target) -> Result<(), CommandError> {
        self.pick_input_with_growth(doc, target, true)
    }

    pub fn pick_input_with_growth(
        &mut self,
        doc: &Document,
        target: Target,
        auto_chain: bool,
    ) -> Result<(), CommandError> {
        self.validate(doc)?;
        if let Target::Curve(id) = target
            && self.baseline_whole.contains(&id)
        {
            return Ok(());
        }
        self.pick_segment(doc, target, auto_chain)
    }
    pub fn pick(&mut self, doc: &Document, target: Target) -> Result<(), CommandError> {
        self.pick_segment(doc, target, true)
    }

    fn pick_segment(
        &mut self,
        doc: &Document,
        target: Target,
        auto_chain: bool,
    ) -> Result<(), CommandError> {
        self.validate(doc)?;
        let &seed = self
            .lookup
            .get(&target)
            .ok_or(CommandError::Usage("Select a curve or surface edge"))?;
        if self.state.selected.contains(&seed) {
            return Ok(());
        }
        let before = self.state.clone();
        let mut state = before.clone();
        let mut journal = Vec::new();
        if state.selected.is_empty() {
            journal.push(UndoFrame::new(&before, seed, None));
            state.selected.insert(seed);
            state.preferred = Some(self.classes[seed]);
            state.fronts = [Some((seed, false)), Some((seed, true))];
        } else {
            let mut attached = false;
            let options = CurveChainOptions {
                continuity: ChainContinuity::Position,
                ..self.options
            };
            for side in self.sides() {
                if let Some((source, at_end)) = state.fronts[side]
                    && let Some(next) = self
                        .index
                        .continuations(source, at_end, options, &state.selected, &self.classes)?
                        .into_iter()
                        .find(|c| c.index == seed)
                {
                    journal.push(UndoFrame::new(&state, seed, Some((side, next.reversed))));
                    state.selected.insert(seed);
                    state.fronts[side] = Some((seed, !next.reversed));
                    attached = true;
                    break;
                }
            }
            if !attached {
                return Err(CommandError::Usage(
                    "Select a connected curve or surface edge",
                ));
            }
        }
        let pending = if !auto_chain || self.pending.as_ref().is_some_and(|pending| pending.one) {
            None
        } else {
            self.grow(&mut state, false, &mut journal, true)?
        };
        self.undo.extend(journal);
        self.state = state;
        self.pending = pending;
        Ok(())
    }
    fn sides(&self) -> Vec<usize> {
        match self.options.direction {
            ChainDirection::Forward => vec![1],
            ChainDirection::Backward => vec![0],
            ChainDirection::Both => vec![1, 0],
        }
    }
    pub fn choosing_branch(&self) -> bool {
        self.pending.is_some()
    }
    pub fn discard_proposal(&mut self) {
        self.pending = None;
    }
    pub fn branch_choices(&self) -> Vec<Target> {
        self.pending
            .as_ref()
            .map(|p| p.candidates.iter().map(|c| self.targets[c.index]).collect())
            .unwrap_or_default()
    }
    pub fn preview(&self) -> Vec<Target> {
        self.pending
            .as_ref()
            .map(|p| {
                p.state
                    .selected
                    .difference(&self.state.selected)
                    .map(|&i| self.targets[i])
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Enter confirms an ambiguous proposal first, then ends the getter.
    pub fn enter(&mut self, doc: &Document) -> Result<bool, CommandError> {
        self.validate(doc)?;
        if let Some(p) = self.pending.take() {
            self.state = p.state;
            self.undo.extend(p.journal);
            Ok(false)
        } else {
            Ok(true)
        }
    }
    pub fn extend(&mut self, doc: &Document, one: bool) -> Result<(), CommandError> {
        self.validate(doc)?;
        if self.pending.is_some() {
            return Err(CommandError::Usage(
                "Select a branch to follow, or Enter to confirm the proposal",
            ));
        }
        let mut state = self.state.clone();
        let mut journal = Vec::new();
        let pending = self.grow(&mut state, one, &mut journal, true)?;
        self.state = state;
        self.undo.extend(journal);
        self.pending = pending;
        Ok(())
    }
    pub fn undo(&mut self, doc: &Document) -> Result<(), CommandError> {
        self.validate(doc)?;
        if self.pending.is_some() {
            return Err(CommandError::Usage(
                "Select a branch to follow, or Enter to confirm the proposal",
            ));
        }
        if let Some(frame) = self.undo.pop() {
            self.state.selected.remove(&frame.added);
            self.state.fronts = frame.fronts;
            self.state.preferred = frame.preferred;
        }
        Ok(())
    }
    pub fn apply(
        &self,
        doc: &mut Document,
        selection: &component_selection::ComponentSelection,
    ) -> Result<(), CommandError> {
        self.validate(doc)?;
        let mut whole = self
            .baseline_whole
            .iter()
            .copied()
            .filter(|&id| doc.is_object_selectable(id))
            .collect::<Vec<_>>();
        let mut components = self.baseline_components.clone();
        for target in self.targets() {
            match target {
                Target::Curve(id) => whole.push(id),
                Target::Edge(object, index) => components.push(ComponentPick {
                    object,
                    index,
                    kind: ComponentSelectionKind::BrepEdge,
                }),
            }
        }
        selection
            .replace(doc, components)
            .map_err(|_| CommandError::ChainStale)?;
        doc.select_objects_direct(whole, SelectionMode::Replace)?;
        Ok(())
    }
    pub fn cancel(
        &self,
        doc: &mut Document,
        selection: &component_selection::ComponentSelection,
    ) -> Result<(), CommandError> {
        let whole = self
            .baseline_whole
            .iter()
            .copied()
            .filter(|&id| doc.is_object_selectable(id))
            .collect::<Vec<_>>();
        let components = self
            .baseline_components
            .iter()
            .copied()
            .filter(|p| {
                doc.is_object_selectable(p.object)
                    && doc.object(p.object).is_some_and(|o| {
                        self.baseline_sources.get(&p.object) == Some(o.geometry_snapshot())
                    })
            })
            .collect();
        selection
            .replace(doc, components)
            .map_err(|_| CommandError::ChainStale)?;
        doc.select_objects_direct(whole, SelectionMode::Replace)?;
        Ok(())
    }
}
mod command;
pub(super) use command::SelChainCommand;

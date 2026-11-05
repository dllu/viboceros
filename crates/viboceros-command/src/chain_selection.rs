//! Source-safe mixed curve and B-rep edge chains, outside model history.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
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
    fronts: [Option<(usize, bool)>; 2],
    preferred: Option<u8>,
}
impl UndoFrame {
    fn new(before: &State, added: usize) -> Self {
        Self {
            added,
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
}
#[derive(Clone, Debug)]
pub struct Session {
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
        options.validate()?;
        let mut sources = BTreeMap::new();
        let mut curves = Vec::new();
        let mut targets = Vec::new();
        let mut classes = Vec::new();
        for object in doc.selectable_objects() {
            let geo = object.geometry();
            let id = object.id();
            if let Some(curve) = geo.curve_ref() {
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
                for (i, edge) in brep.edges().iter().enumerate() {
                    curves.push(viboceros_geometry::Curve3::NurbsCurve(edge.curve().clone()));
                    targets.push(Target::Edge(id, i));
                    classes.push(1);
                }
            }
            sources.insert(id, object.geometry_snapshot().clone());
        }
        let baseline_components = selection
            .checked_picks(doc)
            .map_err(|_| CommandError::ChainStale)?;
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
            options,
            index,
            targets,
            lookup,
            classes,
            sources,
            tolerance: doc.tolerance(),
            baseline_whole: doc.selected_object_ids().collect(),
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
            .filter(|o| {
                o.geometry().curve_ref().is_some()
                    || matches!(o.geometry(), Geometry::Brep(_) | Geometry::NurbsSurface(_))
            })
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
    /// Native object getters ignore whole curves already selected at startup.
    pub fn pick_input(&mut self, doc: &Document, target: Target) -> Result<(), CommandError> {
        self.validate(doc)?;
        if let Target::Curve(id) = target
            && self.baseline_whole.contains(&id)
        {
            return Ok(());
        }
        self.pick(doc, target)
    }
    pub fn pick(&mut self, doc: &Document, target: Target) -> Result<(), CommandError> {
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
        let mut journal = vec![UndoFrame::new(&before, seed)];
        if state.selected.is_empty() {
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
        let pending = self.grow(&mut state, false, &mut journal, true)?;
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

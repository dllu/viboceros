//! Stateful edge spins stage local Undo before one document-history admission.
use crate::*;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::SubD;
mod arguments;
pub use arguments::Direction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub object: ObjectId,
    pub edge: usize,
}
#[derive(Clone, Debug)]
struct Source {
    snapshot: GeometrySnapshot,
    candidate: SubD,
}
#[derive(Clone, Debug)]
struct Step {
    target: Target,
    previous: SubD,
}
#[derive(Clone, Debug)]
pub struct SpinSession {
    tolerance: Tolerance,
    sources: BTreeMap<ObjectId, Source>,
    steps: Vec<Step>,
    last: Option<Target>,
    order: Vec<ObjectId>,
    pub direction: Direction,
}
#[derive(Debug)]
pub struct Outcome {
    pub message: String,
    pub processed_objects: Vec<ObjectId>,
}
impl SpinSession {
    pub fn new(document: &Document, direction: Direction) -> Self {
        Self {
            tolerance: document.tolerance(),
            sources: BTreeMap::new(),
            steps: Vec::new(),
            last: None,
            order: Vec::new(),
            direction,
        }
    }
    pub fn validate(&self, document: &Document) -> Result<(), CommandError> {
        if document.tolerance() != self.tolerance
            || self.sources.iter().any(|(id, s)| {
                !document.is_object_selectable(*id)
                    || document
                        .object(*id)
                        .is_none_or(|o| o.geometry_snapshot() != &s.snapshot)
            })
        {
            return Err(CommandError::SpinEdgeStale);
        }
        Ok(())
    }
    pub fn spin(&mut self, document: &Document, target: Target) -> Result<bool, CommandError> {
        self.validate(document)?;
        if self.steps.len() >= 256 {
            return Err(CommandError::SpinEdgeResourceLimit);
        }
        if !document.is_object_selectable(target.object) {
            return Err(CommandError::SpinEdgeUnavailable);
        }
        if !self.sources.contains_key(&target.object) {
            let object = document
                .object(target.object)
                .ok_or(CommandError::SpinEdgeUnavailable)?;
            let Geometry::SubD(subd) = object.geometry() else {
                return Err(CommandError::SpinEdgeUnavailable);
            };
            if self
                .sources
                .values()
                .map(|s| s.candidate.control_points().len())
                .sum::<usize>()
                + subd.control_points().len()
                > 500_000
            {
                return Err(CommandError::SpinEdgeResourceLimit);
            }
            self.sources.insert(
                target.object,
                Source {
                    snapshot: object.geometry_snapshot().clone(),
                    candidate: subd.clone(),
                },
            );
        }
        let source = self.sources.get_mut(&target.object).unwrap();
        if target.edge >= source.candidate.edges().len() {
            return Err(CommandError::SpinEdgeOutOfRange {
                edge: target.edge,
                count: source.candidate.edges().len(),
            });
        }
        let (candidate, changed) = source
            .candidate
            .try_spin_edge(target.edge, self.direction == Direction::Clockwise)?;
        if changed {
            let previous = std::mem::replace(&mut source.candidate, candidate);
            self.steps.push(Step { target, previous });
            self.order.retain(|id| *id != target.object);
            self.order.push(target.object);
            self.last = Some(target);
        }
        Ok(changed)
    }
    pub fn spin_again(&mut self, document: &Document) -> Result<bool, CommandError> {
        self.spin(
            document,
            self.last.ok_or(CommandError::SpinEdgeUnavailable)?,
        )
    }
    pub fn undo_spin(&mut self) -> bool {
        if let Some(step) = self.steps.pop() {
            self.sources.get_mut(&step.target.object).unwrap().candidate = step.previous;
            self.last = self.steps.last().map(|s| s.target);
            self.order.retain(|id| *id != step.target.object);
            self.order.push(step.target.object);
            true
        } else {
            false
        }
    }
    pub fn last_target(&self) -> Option<Target> {
        self.last
    }
    pub fn spin_count(&self) -> usize {
        self.steps.len()
    }
    fn changed_order(&self) -> Vec<ObjectId> {
        self.order.clone()
    }
    pub fn candidates(&self) -> impl Iterator<Item = (ObjectId, &SubD)> {
        self.changed_order()
            .into_iter()
            .map(|id| (id, &self.sources[&id].candidate))
    }
    pub fn apply_preview(&self, document: &mut Document) -> Result<(), CommandError> {
        document.replace_object_geometries_preserving_control_selection(
            self.candidates()
                .map(|(id, s)| (id, Geometry::SubD(s.clone()))),
        )?;
        document.move_objects_to_end_in_order(self.changed_order())?;
        document.clear_selection();
        Ok(())
    }
    pub fn finish(self, document: &mut Document) -> Result<Outcome, CommandError> {
        run_command_transaction(document, "SubDSpinEdge", move |doc| self.apply(doc))
    }
    pub(super) fn apply(self, doc: &mut Document) -> Result<Outcome, CommandError> {
        self.validate(doc)?;
        let ids = self.changed_order();
        let count = self.steps.len();
        // A full spin cycle can restore the original net while still being
        // an accepted sequence of edits; retain the command's history step.
        doc.replace_object_geometries_preserving_control_selection_with_history(
            self.candidates()
                .map(|(id, s)| (id, Geometry::SubD(s.clone()))),
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        doc.move_objects_to_end_in_order(ids.iter().copied())?;
        if !ids.is_empty() {
            doc.clear_selection();
        }
        Ok(Outcome {
            message: if ids.is_empty() {
                "No changes made".into()
            } else {
                format!("Spun {count} edge(s) in {} SubD object(s)", ids.len())
            },
            processed_objects: ids,
        })
    }
}

pub(super) struct SubDSpinEdgeCommand;
impl Command for SubDSpinEdgeCommand {
    fn name(&self) -> &'static str {
        "SubDSpinEdge"
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        arguments::run(document, args).map(|r| r.0)
    }
}

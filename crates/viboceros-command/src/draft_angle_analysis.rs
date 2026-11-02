//! Persistent false-color analysis state, outside document history.
use super::*;
use std::sync::{Arc, Mutex};
mod options;
pub use options::Options;
use options::{Action, USAGE, parse};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct View {
    pub options: Options,
    pub objects: Arc<BTreeSet<ObjectId>>,
    pub sources: usize,
}

#[derive(Clone, Default)]
struct State {
    enabled: bool,
    options: Options,
    objects: Arc<BTreeSet<ObjectId>>,
}

#[derive(Default)]
pub(crate) struct Session(Mutex<State>);

impl Session {
    pub(crate) fn remove(&self, ids: &BTreeSet<ObjectId>) {
        if ids.is_empty() {
            return;
        }
        let mut state = self.0.lock().unwrap();
        let objects = Arc::make_mut(&mut state.objects);
        for id in ids {
            objects.remove(id);
        }
    }

    pub(crate) fn view(&self, doc: &Document) -> Option<View> {
        let state = self.0.lock().unwrap();
        state.enabled.then(|| View {
            options: state.options,
            objects: state.objects.clone(),
            sources: state
                .objects
                .iter()
                .filter(|id| doc.object(**id).is_some())
                .count(),
        })
    }
    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        let state = self.0.lock().unwrap();
        state.enabled && state.objects.contains(&id)
    }
}

pub(crate) struct AnalysisCommand {
    pub session: Arc<Session>,
    pub off: bool,
    pub curvature: Arc<curvature_analysis::Session>,
}

impl Command for AnalysisCommand {
    fn name(&self) -> &'static str {
        if self.off {
            "DraftAngleAnalysisOff"
        } else {
            "DraftAngleAnalysis"
        }
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if self.off {
            require_consumed(args, 0, USAGE)?;
            return Ok(None);
        }
        let action = parse(&mut Options::default(), args)?;
        let enabled = self.session.0.lock().unwrap().enabled;
        Ok(
            (action != Action::Configure || !enabled).then(|| ObjectSelectionPrompt {
                command: "DraftAngleAnalysis",
                filter: ObjectSelectionFilter::SurfaceAnalysis,
                options: vec![],
                menus: vec![],
                choices: vec![],
                workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            }),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut state = self.session.0.lock().unwrap();
        if self.off {
            require_consumed(args, 0, USAGE)?;
            *state = State {
                options: state.options,
                ..State::default()
            };
            return Ok("Draft angle analysis off".into());
        }
        let mut proposed = state.clone();
        let mut targets = BTreeSet::new();
        let action = parse(&mut proposed.options, args)?;
        if action != Action::Configure || !state.enabled {
            let selected = doc
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::SurfaceAnalysis.accepts_object(o))
                .map(|o| o.id())
                .collect::<BTreeSet<_>>();
            if selected.is_empty() {
                return Err(CommandError::NoObjectsSelected);
            }
            if action != Action::Remove {
                targets = selected.clone();
            }
            // Native command entry enables mesh edges even when the global
            // isocurve preference was off. Later panel changes remain allowed.
            if action != Action::Remove
                && selected
                    .iter()
                    .any(|id| matches!(doc.object(*id).unwrap().geometry(), Geometry::Mesh(_)))
            {
                proposed.options.show_isocurves = true;
            }
            let objects = Arc::make_mut(&mut proposed.objects);
            if action == Action::Remove {
                objects.retain(|id| !selected.contains(id));
            } else {
                objects.extend(selected);
            }
            doc.clear_selection();
        }
        proposed.enabled = true;
        let count = proposed
            .objects
            .iter()
            .filter(|id| doc.object(**id).is_some())
            .count();
        *state = proposed;
        drop(state);
        self.curvature.remove(&targets);
        Ok(format!("Draft angle analysis on {count} object(s)"))
    }
}

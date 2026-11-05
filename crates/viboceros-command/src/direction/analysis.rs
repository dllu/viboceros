//! Persistent analysis membership and snapshot caches; independent of selection.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::GeometrySnapshot;
mod options;
#[cfg(test)]
mod tests;
pub use options::Options;
use options::{Action, parse};

#[derive(Clone, Debug)]
pub struct View {
    pub options: Options,
    pub markers: Arc<[Marker]>,
    pub sources: usize,
}

#[derive(Clone)]
struct Cache {
    snapshot: GeometrySnapshot,
    tolerance: Tolerance,
    markers: Arc<[Marker]>,
}

#[derive(Clone, Default)]
struct State {
    enabled: bool,
    options: Options,
    sources: BTreeSet<ObjectId>,
    cache: BTreeMap<ObjectId, Cache>,
    visible: Vec<ObjectId>,
    flat: Arc<[Marker]>,
}

#[derive(Default)]
pub(crate) struct Session(Mutex<State>);

impl Session {
    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        let state = self.0.lock().unwrap();
        state.enabled && state.sources.contains(&id)
    }

    pub(crate) fn view(&self, doc: &Document) -> Option<View> {
        let mut state = self.0.lock().unwrap();
        if !state.enabled {
            return None;
        }
        let mut visible = Vec::new();
        let mut dirty = false;
        for object in state
            .sources
            .iter()
            .filter_map(|id| doc.object(*id))
            .collect::<Vec<_>>()
        {
            if !object.attributes().is_visible()
                || !doc
                    .layer(object.attributes().layer_id())
                    .is_some_and(|l| l.is_visible())
            {
                continue;
            }
            visible.push(object.id());
            if state.cache.get(&object.id()).is_none_or(|c| {
                c.tolerance != doc.tolerance()
                    || !c.snapshot.shares_storage_with(object.geometry_snapshot())
            }) {
                let markers = collect_markers([(object.id(), object.geometry())], doc.tolerance());
                state.cache.insert(
                    object.id(),
                    Cache {
                        snapshot: object.geometry_snapshot().clone(),
                        tolerance: doc.tolerance(),
                        markers: markers.into(),
                    },
                );
                dirty = true;
            }
        }
        if dirty || visible != state.visible {
            state.flat = visible
                .iter()
                .flat_map(|id| state.cache[id].markers.iter().cloned())
                .collect::<Vec<_>>()
                .into();
            state.visible = visible;
        }
        // Membership survives Delete/Undo; large deleted snapshots need not.
        let State { sources, cache, .. } = &mut *state;
        cache.retain(|id, _| sources.contains(id) && doc.object(*id).is_some());
        Some(View {
            options: state.options.clone(),
            markers: state.flat.clone(),
            sources: state
                .sources
                .iter()
                .filter(|id| doc.object(**id).is_some())
                .count(),
        })
    }
}

pub(crate) struct AnalysisCommand {
    pub session: Arc<Session>,
    pub off: bool,
}

impl Command for AnalysisCommand {
    fn name(&self) -> &'static str {
        if self.off { "ShowDirOff" } else { "ShowDir" }
    }
    fn records_history(&self) -> bool {
        false
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if self.off {
            require_consumed(args, 0, options::USAGE)?;
            return Ok(None);
        }
        let action = parse(&mut Options::default(), args)?;
        let enabled = self.session.0.lock().unwrap().enabled;
        Ok((matches!(
            action,
            Action::Start | Action::Add | Action::Remove | Action::Edit(_)
        ) || (!enabled && action == Action::Configure))
            .then(|| ObjectSelectionPrompt {
                command: "ShowDir",
                filter: ObjectSelectionFilter::Direction,
                options: vec![],
                menus: vec![],
                choices: vec![],
                workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            }))
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl AnalysisCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let mut state = self.session.0.lock().unwrap();
        if self.off {
            require_consumed(args, 0, options::USAGE)?;
            *state = State {
                options: state.options.clone(),
                ..State::default()
            };
            return Ok("Direction analysis off".into());
        }
        let mut proposed = state.clone();
        let action = parse(&mut proposed.options, args)?;
        if let Action::Edit(edit) = action {
            if !state.enabled {
                return Err(CommandError::Usage(
                    "start ShowDir before editing analyzed directions",
                ));
            }
            proposed.sources.extend(
                doc.selected_objects()
                    .filter(|o| ObjectSelectionFilter::Direction.accepts_object(o))
                    .map(|o| o.id()),
            );
            let result = run_command_transaction(
                doc,
                if edit == DirectionEdit::Flip {
                    "Flip"
                } else {
                    "Dir"
                },
                |doc| {
                    let mut replacements = Vec::new();
                    let options = super::Options { edit, faces: None };
                    if doc.selected_object_count() == 0 {
                        return Err(CommandError::NoObjectsSelected);
                    }
                    for object in doc
                        .selected_objects()
                        .filter(|o| ObjectSelectionFilter::Direction.accepts_object(o))
                    {
                        if edit != DirectionEdit::Flip
                            && !ObjectSelectionFilter::Surfaces.accepts_object(object)
                        {
                            continue;
                        }
                        if let Some(geometry) =
                            edited_geometry(object.geometry(), &options, doc.tolerance())?
                        {
                            replacements.push((object.id(), geometry));
                        }
                    }
                    replacements
                        .retain(|(id, geometry)| doc.object(*id).unwrap().geometry() != geometry);
                    let selected = doc.selected_object_ids().collect::<Vec<_>>();
                    let release_on_undo =
                        postselected || (edit != DirectionEdit::Flip && selected.len() == 1);
                    if release_on_undo && !replacements.is_empty() {
                        let changed = replacements
                            .iter()
                            .map(|(id, _)| *id)
                            .collect::<BTreeSet<_>>();
                        doc.select_command_results(
                            selected.iter().copied().filter(|id| !changed.contains(id)),
                        )?;
                    }
                    let count = doc.replace_object_geometries(replacements)?;
                    if release_on_undo && !postselected && count > 0 {
                        doc.select_command_results(selected)?;
                    }
                    Ok(format!("Edited {count} analyzed object direction(s)"))
                },
            );
            if result.is_ok() {
                *state = proposed;
            }
            return result;
        }
        let selected = doc
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Direction.accepts_object(o))
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        match action {
            Action::Start | Action::Add => {
                if selected.is_empty() {
                    return Err(CommandError::NoObjectsSelected);
                }
                proposed.sources.extend(selected);
            }
            Action::Remove => {
                if selected.is_empty() {
                    return Err(CommandError::NoObjectsSelected);
                }
                proposed.sources.retain(|id| !selected.contains(id));
            }
            Action::Configure if !state.enabled => {
                if selected.is_empty() {
                    return Err(CommandError::NoObjectsSelected);
                }
                proposed.sources.extend(selected);
            }
            _ => {}
        }
        proposed.enabled = true;
        let count = proposed
            .sources
            .iter()
            .filter(|id| doc.object(**id).is_some())
            .count();
        *state = proposed;
        Ok(format!("Direction analysis on {count} object(s)"))
    }
}

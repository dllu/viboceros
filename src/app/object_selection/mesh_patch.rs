//! Source, hole and reference picking share the command's immutable preparation.
use super::*;
use std::sync::Arc;
#[cfg(test)]
mod tests;
use viboceros_command::mesh_patch::{self as solver, Options};
use viboceros_document::{GeometrySnapshot, HistoryGroup};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Sources,
    Holes,
    Surface,
    Angle,
    Divisions,
}
#[derive(Clone, Debug)]
struct Stamp(Arc<HistoryGroup>);
impl PartialEq for Stamp {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    options: Options,
    phase: Phase,
    resume: Phase,
    saved_selection: Vec<ObjectId>,
    parents: Vec<(ObjectId, GeometrySnapshot)>,
    tolerance: viboceros_geometry::Tolerance,
    history: Stamp,
}
impl Session {
    pub(super) fn label(&self) -> &'static str {
        match self.phase {
            Phase::Sources => "MeshPatch sources",
            Phase::Holes => "MeshPatch holes",
            Phase::Surface => "StartingSurface",
            Phase::Angle => "AngleTolerance",
            Phase::Divisions => "MainSegmentCount",
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        match self.phase {
            Phase::Sources => {
                "Select curves, points or point clouds; Enter selects holes, Esc cancels"
            }
            Phase::Holes => {
                "Select closed hole curves; Enter builds the patch. AngleTolerance, TrimBack, MainSegmentCount, StartingSurface"
            }
            Phase::Surface => "Select one starting surface; Enter accepts, Esc cancels",
            Phase::Angle => "Enter an angle in degrees between 0 and 180",
            Phase::Divisions => "Enter a positive perimeter segment count",
        }
    }
    fn valid(&self, doc: &Document) -> bool {
        doc.history_group_is_current(&self.history.0)
            && self.tolerance == doc.tolerance()
            && self.parents.iter().all(|(id, snapshot)| {
                doc.is_object_selectable(*id)
                    && doc
                        .object(*id)
                        .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            })
    }
    fn capture(&mut self, doc: &Document, ids: &[ObjectId]) {
        self.parents.extend(ids.iter().filter_map(|id| {
            doc.object(*id)
                .map(|o| (*id, o.geometry_snapshot().clone()))
        }));
    }
    fn settings(&self) -> String {
        let p = self.options.preferences;
        format!(
            "MeshPatch AngleTolerance={} TrimBack={} MainSegmentCount={}",
            p.angle,
            if p.trim_back { "Yes" } else { "No" },
            p.divisions
        )
    }
    fn command(&self) -> String {
        let mut command = self.settings();
        for (name, ids) in [
            (
                "Sources",
                self.options.sources.as_deref().unwrap_or_default(),
            ),
            ("Holes", self.options.holes.as_slice()),
        ] {
            if !ids.is_empty() {
                command.push_str(&format!(
                    " {name}={}",
                    ids.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
        }
        if let Some(id) = self.options.surface {
            command.push_str(&format!(" StartingSurface={id}"));
        }
        if let Some(face) = self.options.face {
            command.push_str(&format!(" Face={face}"));
        }
        if let Some(id) = self.options.outer {
            command.push_str(&format!(" OuterBoundary={id}"));
        }
        command
    }
}
fn configure(pending: &mut PendingObjectCommand) {
    let session = pending.mesh_patch.as_ref().unwrap();
    if let Some(option) = pending
        .description
        .options
        .iter_mut()
        .find(|o| o.name == "TrimBack")
    {
        option.value = session.options.preferences.trim_back;
    }
    pending.description.filter = match session.phase {
        Phase::Sources => ObjectSelectionFilter::MeshPatchSources,
        Phase::Holes => ObjectSelectionFilter::MeshPatchHoles,
        Phase::Surface => ObjectSelectionFilter::SurfaceComponents,
        _ => pending.description.filter,
    };
    pending.phase = if matches!(session.phase, Phase::Angle | Phase::Divisions) {
        ObjectPromptPhase::Options
    } else {
        ObjectPromptPhase::Selecting
    };
    pending.command_override = Some(session.settings());
}
impl VibocerosApp {
    pub(super) fn start_mesh_patch_prompt(
        &mut self,
        input: &str,
        description: ObjectSelectionPrompt,
    ) -> bool {
        let words = input.split_whitespace().skip(1).collect::<Vec<_>>();
        let options = match self.commands.mesh_patch_options(&words) {
            Ok(v) => v,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let history = match self.document.begin_history_group("MeshPatch") {
            Ok(v) => v,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let before = self
            .document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let selected = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::MeshPatchSources.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let mut session = Session {
            options,
            phase: Phase::Sources,
            resume: Phase::Sources,
            saved_selection: vec![],
            parents: vec![],
            tolerance: self.document.tolerance(),
            history: Stamp(Arc::new(history)),
        };
        if !selected.is_empty() {
            session.capture(&self.document, &selected);
            session.options.sources = Some(selected);
            session.phase = Phase::Holes;
            self.document.clear_selection();
        }
        let mut pending = PendingObjectCommand::selecting(description);
        pending.selection_before = Some(before);
        pending.mesh_patch = Some(session);
        configure(&mut pending);
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
    pub(super) fn continue_mesh_patch_prompt(&mut self, input: &str) -> bool {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.mesh_patch.is_none())
        {
            return false;
        }
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_object_prompt(true);
            return true;
        }
        if !input.is_empty()
            && input
                .split_whitespace()
                .next()
                .is_some_and(|w| self.commands.recognizes(w))
        {
            self.cancel_object_prompt(true);
            return false;
        }
        let mut pending = self.object_prompt.take().unwrap();
        let mut session = pending.mesh_patch.take().unwrap();
        if !session.valid(&self.document) {
            self.push_log(
                "MeshPatch inputs or document history changed; restart the command".into(),
            );
            self.command_input.clear();
            return true;
        }
        let selected = self
            .document
            .selected_objects()
            .filter(|o| pending.description.filter.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if input.is_empty() {
            match session.phase {
                Phase::Sources => {
                    if selected.is_empty() {
                        self.push_log("Select curves or points first".into());
                    } else {
                        session.capture(&self.document, &selected);
                        session.options.sources = Some(selected);
                        session.phase = Phase::Holes;
                        self.document.clear_selection();
                    }
                }
                Phase::Holes => {
                    session.options.holes = selected;
                    let command = session.command();
                    self.command_input.clear();
                    self.execute_command(&command);
                    return true;
                }
                Phase::Surface => {
                    if selected.len() != 1 {
                        self.push_log("Select one starting surface".into());
                    } else {
                        session.options.surface = Some(selected[0]);
                        session.options.face = None;
                        session.capture(&self.document, &selected);
                        session.phase = session.resume;
                        let _ = self.document.select_objects_direct(
                            session.saved_selection.clone(),
                            SelectionMode::Replace,
                        );
                    }
                }
                _ => self.push_log("Enter a numeric option value".into()),
            }
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("TrimBack")
        {
            session.options.preferences.trim_back = !session.options.preferences.trim_back;
            self.commands.accept_mesh_patch_options(&session.options);
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("StartingSurface")
        {
            session.saved_selection = selected;
            session.resume = session.phase;
            session.phase = Phase::Surface;
            self.document.clear_selection();
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("AngleTolerance")
        {
            session.resume = session.phase;
            session.phase = Phase::Angle;
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("MainSegmentCount")
        {
            session.resume = session.phase;
            session.phase = Phase::Divisions;
        } else {
            let value = match session.phase {
                Phase::Angle => format!("AngleTolerance={input}"),
                Phase::Divisions => format!("MainSegmentCount={input}"),
                _ => input.to_owned(),
            };
            let words = value.split_whitespace().collect::<Vec<_>>();
            match solver::parse(&words, session.options.preferences) {
                Ok(parsed)
                    if parsed.sources.is_none()
                        && parsed.holes.is_empty()
                        && parsed.surface.is_none()
                        && parsed.face.is_none()
                        && parsed.outer.is_none() =>
                {
                    session.options.preferences = parsed.preferences;
                    self.commands.accept_mesh_patch_options(&session.options);
                    if matches!(session.phase, Phase::Angle | Phase::Divisions) {
                        session.phase = session.resume;
                    }
                }
                Ok(_) => {
                    self.push_log("Pick holes and use StartingSurface to select a reference".into())
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        pending.mesh_patch = Some(session);
        configure(&mut pending);
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.log_object_prompt();
        true
    }
}

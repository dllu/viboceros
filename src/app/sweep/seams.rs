use super::*;
use crate::viewport::curve_seam::Event;
use std::sync::Arc;
use viboceros_command::curve_reference::CapturedCurve;

impl Session {
    pub(in crate::app) fn seam_editor(&self) -> Option<&Editor> {
        match &self.state.stage {
            ProfileStage::Seams {
                editor,
                editing: true,
            } => Some(editor),
            _ => None,
        }
    }
}

pub(super) fn with_seam_profiles(
    source: Arc<Source>,
    editor: Option<&Editor>,
    tolerance: Tolerance,
) -> Result<Arc<Source>, CommandError> {
    let Some(editor) = editor else {
        return Ok(source);
    };
    let profiles = editor.profiles(tolerance)?;
    let ids = editor.sources().map(CapturedCurve::object_id);
    Arc::unwrap_or_clone(source)
        .with_profile_curves(ids.zip(profiles))
        .map(Arc::new)
}

impl VibocerosApp {
    /// Return true if Enter opened the seam editor or review.
    pub(super) fn advance_sweep_review(&mut self) -> bool {
        let session = self.sweep_session.as_mut().unwrap();
        if session.seam_editor().is_some() {
            let before = session.state.clone();
            if let ProfileStage::Seams { editor, editing } = &mut session.state.stage
                && !std::mem::take(&mut session.seam_id_pending)
                && !editor.finish_profile_flip()
            {
                *editing = false;
                session.undo.push(before);
            }
            self.log_sweep_input();
            return true;
        }
        if matches!(
            session.state.stage,
            ProfileStage::Review | ProfileStage::Seams { .. }
        ) {
            return false;
        }
        let document = &self.document;
        let result = session
            .state
            .capture_source(document)
            .and_then(|source| {
                source.validate_point_end_stations(&session.state.options)?;
                Ok((
                    source.closed_profile_ids()?,
                    source.can_close(&session.state.options),
                ))
            })
            .and_then(|(ids, can_close)| {
                let sources = ids
                    .into_iter()
                    .map(|id| CapturedCurve::capture(document, id, None))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((sources, can_close))
            });
        match result {
            Ok((sources, can_close))
                if session.review_open_profiles || can_close || !sources.is_empty() =>
            {
                session.undo.push(session.state.clone());
                session.state.stage = if sources.is_empty() {
                    ProfileStage::Review
                } else {
                    ProfileStage::Seams {
                        editor: Editor::for_sweep_profiles(sources, document.tolerance(), false),
                        editing: true,
                    }
                };
                self.log_sweep_input();
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                true
            }
        }
    }

    pub(super) fn continue_sweep_seams(&mut self, input: &str) {
        if !self.validate_sweep_sources() {
            return;
        }
        let point = self.profile_seam_point(input);
        let viewport = self.active_viewport;
        let tolerance = self.document.tolerance();
        let session = self.sweep_session.as_mut().unwrap();
        let before = session.state.clone();
        let ProfileStage::Seams { editor, .. } = &mut session.state.stage else {
            return;
        };
        match editor.edit_profile(
            input,
            &mut session.seam_id_pending,
            viewport,
            point,
            tolerance,
        ) {
            Ok(true) => session.undo.push(before),
            Ok(false) => {}
            Err(error) => self.push_log(error.into()),
        }
        self.log_sweep_input();
    }

    pub(in crate::app) fn apply_sweep_one_seam_event(
        &mut self,
        viewport: usize,
        event: Event,
    ) -> bool {
        let Some(session) = self.sweep_session.as_mut() else {
            return false;
        };
        if session.seam_editor().is_none() || !self.validate_sweep_sources() {
            return false;
        }
        let session = self.sweep_session.as_mut().unwrap();
        if matches!(event, Event::Grab(..) | Event::Flip(_)) {
            session.undo.push(session.state.clone());
        }
        let ProfileStage::Seams { editor, .. } = &mut session.state.stage else {
            return false;
        };
        let changed = editor.apply(viewport, event);
        if matches!(event, Event::Flip(_)) {
            self.log_sweep_input();
        }
        changed
    }
}

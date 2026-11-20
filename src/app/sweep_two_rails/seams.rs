use super::*;
use crate::viewport::curve_seam::Event;
use viboceros_command::{CommandError, curve_reference::CapturedCurve};

impl State {
    pub(super) fn prepare_seams(
        &self,
        document: &Document,
        natural: bool,
    ) -> Result<Option<Editor>, CommandError> {
        let closure = self
            .picks
            .iter()
            .map(|pick| {
                pick.geometry
                    .curve_ref()
                    .ok_or(CommandError::Usage("Select Sweep2 cross sections"))?
                    .is_closed()
                    .map_err(CommandError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !closure.contains(&true) {
            return Ok(None);
        }
        if closure.contains(&false) {
            return Err(CommandError::Usage(
                "Select either open or closed Sweep2 profiles",
            ));
        }
        let sources = self
            .picks
            .iter()
            .map(|pick| CapturedCurve::capture(document, pick.id, None))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Editor::for_sweep_profiles(
            sources,
            document.tolerance(),
            natural,
        )))
    }

    pub(super) fn capture_edited(&self, document: &Document) -> Result<Source, CommandError> {
        with_seam_profiles(
            self.capture(document)?,
            self.seams.as_ref(),
            document.tolerance(),
        )
    }
}

pub(super) fn with_seam_profiles(
    source: Source,
    editor: Option<&Editor>,
    tolerance: Tolerance,
) -> Result<Source, CommandError> {
    match editor {
        Some(editor) => source.with_profile_curves(editor.profiles(tolerance)?),
        None => Ok(source),
    }
}

impl Session {
    pub(in crate::app) fn seam_editor(&self) -> Option<&Editor> {
        self.state.seams.as_ref().filter(|_| !self.state.review)
    }
}

impl VibocerosApp {
    pub(super) fn advance_sweep_two_rails(&mut self) {
        if !self.validate_sweep_two_rails() {
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        let pending = std::mem::take(&mut session.seam_id_pending);
        if session.ready() && !pending {
            let mut next = session.state.clone();
            if next.seams.is_some() {
                next.review = true;
            } else {
                match next.prepare_seams(&self.document, session.options.natural_seams) {
                    Ok(seams) => {
                        next.review = seams.is_none();
                        next.seams = seams;
                    }
                    Err(error) => {
                        self.command_file.fail_dispatch();
                        self.push_log(format!("Error: {error}"));
                        return;
                    }
                }
            }
            session
                .undo
                .push(std::mem::replace(&mut session.state, next));
        }
        self.log_sweep_two_rails();
    }

    pub(super) fn continue_sweep_two_rail_seams(&mut self, input: &str) {
        if !self.validate_sweep_two_rails() {
            return;
        }
        let point = self.profile_seam_point(input);
        let viewport = self.active_viewport;
        let tolerance = self.document.tolerance();
        let session = self.sweep_two_rails.as_mut().unwrap();
        let before = session.state.clone();
        let editor = session.state.seams.as_mut().unwrap();
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
        self.log_sweep_two_rails();
    }

    pub(in crate::app) fn apply_sweep_two_rail_seam_event(
        &mut self,
        viewport: usize,
        event: Event,
    ) -> bool {
        let Some(session) = self
            .sweep_two_rails
            .as_mut()
            .filter(|session| session.seam_editor().is_some())
        else {
            return false;
        };
        if matches!(event, Event::Grab(..)) {
            session.undo.push(session.state.clone());
        }
        let changed = session.state.seams.as_mut().unwrap().apply(viewport, event);
        if matches!(event, Event::Flip(_)) {
            self.log_sweep_two_rails();
        }
        changed
    }
}

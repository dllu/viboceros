use super::*;
use crate::viewport::curve_seam::Event;
use viboceros_command::{CommandError, curve_reference::CapturedCurve};
use viboceros_drafting::PointInput;
use viboceros_geometry::Real;

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
        let (name, value) = input
            .split_once(|c: char| c == '=' || c.is_whitespace())
            .map_or((input, ""), |(name, value)| (name, value.trim()));
        let session = self.sweep_two_rails.as_mut().unwrap();
        let before = session.state.clone();
        let editor = session.state.seams.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        if session.seam_id_pending || name.eq_ignore_ascii_case("SelID") {
            let id = if session.seam_id_pending {
                input
            } else {
                value
            };
            session.seam_id_pending = id.is_empty();
            if !session.seam_id_pending
                && !id
                    .parse::<ObjectId>()
                    .is_ok_and(|id| editor.select_id(self.active_viewport, id))
            {
                self.push_log("Select a closed Sweep2 profile by object ID".into());
                return;
            }
        } else if let Some(result) = editor.option(input, tolerance) {
            match result {
                Ok(()) => session.undo.push(before),
                Err(error) => {
                    self.push_log(error.into());
                    return;
                }
            }
        } else {
            let parameter = name
                .eq_ignore_ascii_case("Parameter")
                .then(|| value.parse::<Real>().ok())
                .flatten()
                .filter(|value| value.is_finite());
            let point = PointInput::parse_with_units(input, self.document.units())
                .and_then(Result::ok)
                .and_then(|point| {
                    point
                        .resolve(
                            self.viewports[self.active_viewport].construction_plane(),
                            self.last_point,
                        )
                        .ok()
                });
            if !point
                .is_some_and(|point| editor.flip_at_point(self.active_viewport, point, tolerance))
                && editor.relocate_target(parameter, point, tolerance)
            {
                session.undo.push(before);
            }
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

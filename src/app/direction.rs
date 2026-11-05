//! Bare Dir inspection; geometry stays provisional until Enter accepts it.
use super::*;
use crate::viewport::direction::{Marker, Overlay, collect_markers};
use std::collections::BTreeSet;
use viboceros_command::direction::{DirectionEdit, DirectionSession};

pub(super) struct Session {
    edits: DirectionSession,
    ids: BTreeSet<ObjectId>,
    markers: Vec<Marker>,
    mode: DirectionEdit,
    choosing_mode: bool,
}

impl Session {
    pub(super) fn overlay(&self) -> Overlay<'_> {
        Overlay {
            edits: &self.edits,
            ids: &self.ids,
            markers: &self.markers,
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        if self.choosing_mode {
            "Mode: FlipNormal / FlipU / FlipV / SwapUV; Esc discards edits"
        } else if self.edits.geometries().all(|(_, geometry)| {
            geometry.curve_ref().is_some()
                || matches!(geometry, Geometry::Mesh(_) | Geometry::SubD(_))
        }) {
            "Click to flip direction; Flip / FlipAll; Enter accepts, Esc discards edits"
        } else {
            "Click to edit direction; Flip / UReverse / VReverse / SwapUV / FlipAll / Mode / NextMode; Enter accepts, Esc discards edits"
        }
    }
}

impl VibocerosApp {
    pub(super) fn validate_direction(&mut self) {
        if self
            .direction_session
            .as_ref()
            .is_some_and(|session| !session.edits.is_current(&self.document))
        {
            self.finish_direction(false);
            self.push_log("Dir source changed; inspection discarded".into());
        }
    }
    pub(super) fn start_direction(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            matches!(
                w.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "dir" | "direction"
            )
        }) || words.next().is_some()
        {
            return false;
        }
        self.begin_direction(false);
        true
    }

    pub(super) fn begin_direction(&mut self, postselected: bool) {
        self.cancel_interactive_command(false);
        match DirectionSession::new(&self.document, postselected) {
            Ok(edits) => {
                let ids = edits
                    .geometries()
                    .map(|(id, _)| id)
                    .collect::<BTreeSet<_>>();
                let mode = if ids.len() == 1 {
                    DirectionEdit::Flip
                } else {
                    self.direction_mode
                };
                let markers = collect_markers(edits.geometries(), self.document.tolerance());
                self.direction_session = Some(Session {
                    edits,
                    ids,
                    markers,
                    mode,
                    choosing_mode: false,
                });
                self.command_input.clear();
                self.push_log(self.direction_session.as_ref().unwrap().hint().into());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn continue_direction(&mut self, input: &str) -> bool {
        let Some(session) = self.direction_session.as_ref() else {
            return false;
        };
        if !session.edits.is_current(&self.document) {
            self.finish_direction(false);
            self.push_log("Dir source changed; inspection discarded".into());
            return false;
        }
        let token = input.trim().trim_start_matches(['_', '-']);
        if token.is_empty() {
            self.finish_direction(true);
        } else if token.eq_ignore_ascii_case("Cancel") {
            self.finish_direction(false);
        } else if token.eq_ignore_ascii_case("Mode") {
            self.direction_session.as_mut().unwrap().choosing_mode = true;
            self.push_log(self.direction_session.as_ref().unwrap().hint().into());
        } else if token.eq_ignore_ascii_case("NextMode") {
            let mode = session.mode.next_mode();
            self.set_direction_mode(mode);
        } else if let Some(value) = token
            .split_once('=')
            .filter(|(k, _)| k.eq_ignore_ascii_case("Mode"))
            .map(|(_, v)| v)
            .or_else(|| {
                token
                    .split_once(char::is_whitespace)
                    .filter(|(k, _)| k.eq_ignore_ascii_case("Mode"))
                    .map(|(_, v)| v.trim())
            })
        {
            if let Some(edit) = parse_edit(value) {
                self.set_direction_mode(edit);
            } else {
                self.push_log("Mode expects FlipNormal, FlipU, FlipV or SwapUV".into());
            }
        } else if session.choosing_mode {
            if let Some(edit) = parse_edit(token) {
                self.set_direction_mode(edit);
            } else {
                self.push_log("Mode expects FlipNormal, FlipU, FlipV or SwapUV".into());
            }
        } else if token.eq_ignore_ascii_case("FlipAll") {
            self.edit_direction(session.mode, None);
        } else if let Some(edit) = parse_edit(token) {
            self.edit_direction(edit, None);
        } else {
            self.push_log("Unknown Dir option; Enter accepts, Esc discards edits".into());
        }
        self.command_input.clear();
        true
    }

    fn set_direction_mode(&mut self, mode: DirectionEdit) {
        self.direction_mode = mode;
        let session = self.direction_session.as_mut().unwrap();
        session.mode = mode;
        session.choosing_mode = false;
        self.push_log(format!("Dir click mode: {}", mode.label()));
    }

    fn edit_direction(&mut self, edit: DirectionEdit, target: Option<ObjectId>) {
        let session = self.direction_session.as_mut().unwrap();
        match session.edits.apply(&self.document, edit, target) {
            Ok(count) => {
                session.markers =
                    collect_markers(session.edits.geometries(), self.document.tolerance());
                self.push_log(format!(
                    "Previewed {count} direction edit(s); Enter accepts, Esc discards"
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn direction_click(&mut self, id: ObjectId) {
        if let Some(session) = &self.direction_session {
            self.edit_direction(session.mode, Some(id));
        }
    }

    pub(super) fn finish_direction(&mut self, accept: bool) {
        let Some(session) = self.direction_session.take() else {
            return;
        };
        let result = if accept {
            session
                .edits
                .accept(&mut self.document)
                .map(|n| format!("Edited {n} object direction(s)"))
        } else {
            session
                .edits
                .cancel(&mut self.document)
                .map(|()| "Dir inspection discarded".into())
        };
        match result {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
    }
}

fn parse_edit(value: &str) -> Option<DirectionEdit> {
    match value
        .trim_start_matches(['_', '-'])
        .to_ascii_lowercase()
        .as_str()
    {
        "flip" | "flipnormal" => Some(DirectionEdit::Flip),
        "ureverse" | "flipu" => Some(DirectionEdit::ReverseU),
        "vreverse" | "flipv" => Some(DirectionEdit::ReverseV),
        "swapuv" => Some(DirectionEdit::SwapUv),
        _ => None,
    }
}

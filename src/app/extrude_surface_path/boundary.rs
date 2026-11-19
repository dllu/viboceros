//! Boundary selection before the path getter, retaining the picked face.
use super::*;
use viboceros_command::surface_reference::BoundaryRef;

impl SurfacePathSession {
    pub(in crate::app) fn picked_boundary(&self, document: &Document) -> Option<BoundaryRef> {
        if let Boundary::Picked { reference, .. } = self.boundary
            && self.boundary.is_current(document)
        {
            Some(reference)
        } else {
            None
        }
    }
}

impl VibocerosApp {
    pub(in crate::app) fn picking_surface_path_boundary(&self) -> bool {
        self.surface_path_prompt
            .as_ref()
            .is_some_and(|session| matches!(session.boundary, Boundary::Selecting))
    }

    pub(super) fn start_surface_path_boundary(
        &mut self,
        input: BoundaryInput,
        path: Option<ObjectId>,
    ) {
        let session = self.surface_path_prompt.as_mut().unwrap();
        session.boundary = Boundary::Selecting;
        session.pending_path = path;
        match input {
            BoundaryInput::Reference(reference) => self.pick_surface_path_boundary(reference),
            _ => self.log_surface_path_prompt(),
        }
    }

    pub(in crate::app) fn pick_surface_path_boundary(&mut self, reference: BoundaryRef) {
        match Boundary::capture(&self.document, reference) {
            Ok(boundary) => {
                let session = self.surface_path_prompt.as_mut().unwrap();
                session.boundary = boundary;
                if let Some(path) = session.pending_path.take() {
                    self.select_surface_path(Some(&SurfacePathSelector::Id(path)), None);
                } else {
                    self.log_surface_path_prompt();
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn continue_surface_path_boundary(&mut self, input: &str) {
        let token = input.trim_start_matches(['_', '-', '\'']);
        if token.is_empty()
            || token.eq_ignore_ascii_case("Enter")
            || token.eq_ignore_ascii_case("Cancel")
            || token == "!"
        {
            self.cancel_current_prompt_or_selection();
        } else {
            let reference = token
                .split_once(char::is_whitespace)
                .filter(|(key, _)| key.eq_ignore_ascii_case("SelID"))
                .or_else(|| {
                    token
                        .split_once('=')
                        .filter(|(key, _)| key.eq_ignore_ascii_case("ToBoundary"))
                })
                .map_or(token, |(_, value)| value.trim());
            match BoundaryRef::parse(reference) {
                Ok(boundary) => self.pick_surface_path_boundary(boundary),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.command_input.clear();
    }

    pub(super) fn reset_surface_path_boundary(&mut self) {
        let session = self.surface_path_prompt.as_mut().unwrap();
        session.boundary = Boundary::Selecting;
        session.points = None;
        self.push_log("Boundary surface changed. Select a boundary surface".into());
    }
}

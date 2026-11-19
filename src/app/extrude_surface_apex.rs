//! Frozen face sources, optional boundary selection, and shared apex drafting.
use super::*;
use crate::viewport::surface_apex::SurfaceApexDraft;
use viboceros_command::extrude_surface_apex::{
    ApexPoint, BoundaryInput, BoundaryRef, SurfaceApexInput, SurfaceApexOptions,
};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::Brep;

pub(super) struct Session {
    draft: Option<SurfaceApexDraft>,
    boundary: Boundary,
}

enum Boundary {
    None,
    Selecting,
    Picked {
        reference: BoundaryRef,
        source: GeometrySnapshot,
        face: Box<Brep>,
    },
}

impl Boundary {
    fn capture(
        document: &Document,
        reference: BoundaryRef,
    ) -> Result<Self, viboceros_command::CommandError> {
        let face = reference.capture(document)?;
        Ok(Self::Picked {
            reference,
            source: document
                .object(reference.object)
                .unwrap()
                .geometry_snapshot()
                .clone(),
            face: Box::new(face),
        })
    }

    fn is_current(&self, document: &Document) -> bool {
        match self {
            Self::Picked {
                reference, source, ..
            } => {
                document.is_object_selectable(reference.object)
                    && document
                        .object(reference.object)
                        .is_some_and(|object| object.geometry_snapshot() == source)
            }
            _ => true,
        }
    }

    fn face(&self) -> Option<&Brep> {
        match self {
            Self::Picked { face, .. } => Some(face),
            _ => None,
        }
    }
}

impl super::extrude_surface::SurfaceExtrusionSources {
    pub(super) fn apex_draft(&self, document: &Document) -> Option<&SurfaceApexDraft> {
        let session = self.apex.as_ref()?;
        if matches!(session.boundary, Boundary::Selecting)
            || !self.is_current(document)
            || !session.boundary.is_current(document)
        {
            return None;
        }
        session.draft.as_ref()
    }
}

impl VibocerosApp {
    pub(super) fn try_start_surface_apex_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrfToPoint")
        }) {
            return false;
        }
        let Ok(parsed) = SurfaceApexInput::parse_with_options(
            &words.collect::<Vec<_>>(),
            self.commands.surface_apex_options(),
        ) else {
            return false;
        };
        let mut sources = match self.prepare_surface_extrusion_sources(parsed.faces) {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let boundary = match parsed.boundary {
            BoundaryInput::None => Boundary::None,
            BoundaryInput::Select => Boundary::Selecting,
            BoundaryInput::Reference(reference) => {
                match Boundary::capture(&self.document, reference) {
                    Ok(boundary) => boundary,
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        return true;
                    }
                }
            }
        };
        sources.apex = Some(Session {
            draft: None,
            boundary,
        });
        self.cancel_interactive_command(false);
        self.surface_extrusion_sources = Some(sources);
        self.active_command = Some(InteractiveCommand::ExtrudeSurfaceToPoint {
            options: parsed.options,
        });
        self.command_input.clear();
        if let Some(apex) = parsed.apex {
            let plane = self.viewports[self.active_viewport].construction_plane();
            if self.accept_drafting_point(apex.resolve(plane)) {
                return true;
            }
        }
        if let Some(sources) = &mut self.surface_extrusion_sources {
            let session = sources.apex.as_mut().unwrap();
            let mut draft = SurfaceApexDraft::capture(&self.document, &sources.targets);
            draft.set_boundary(session.boundary.face(), self.document.tolerance());
            session.draft = Some(draft);
        }
        if self.picking_surface_apex_boundary() {
            self.push_log("Select one boundary surface".into());
        } else {
            self.log_surface_apex(parsed.options);
        }
        true
    }

    pub(super) fn picking_surface_apex_boundary(&self) -> bool {
        self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && self
                .apex_session()
                .is_some_and(|session| matches!(session.boundary, Boundary::Selecting))
    }

    pub(super) fn surface_apex_hint(&self) -> Option<&'static str> {
        match &self.apex_session()?.boundary {
            Boundary::Selecting => Some("Select one boundary surface"),
            Boundary::Picked { .. } => {
                Some("Pick the apex beyond the boundary (DeleteInput / SplitAtTangents)")
            }
            Boundary::None => None,
        }
    }

    pub(super) fn update_surface_apex_hover(&mut self, hover: Option<Point3>) -> bool {
        let Some(draft) = self.apex_session_mut().and_then(|s| s.draft.as_mut()) else {
            return false;
        };
        if hover == draft.hover {
            return false;
        }
        draft.hover = hover;
        true
    }

    pub(super) fn pick_surface_apex_boundary(&mut self, reference: BoundaryRef) {
        let Some(InteractiveCommand::ExtrudeSurfaceToPoint { options }) = self.active_command
        else {
            return;
        };
        let boundary = match Boundary::capture(&self.document, reference) {
            Ok(boundary) => boundary,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        let tolerance = self.document.tolerance();
        let session = self.apex_session_mut().unwrap();
        if let Some(draft) = &mut session.draft {
            draft.set_boundary(boundary.face(), tolerance);
        }
        session.boundary = boundary;
        self.log_surface_apex(options);
    }

    pub(super) fn point_to_surface_apex(
        &mut self,
        apex: Point3,
        options: SurfaceApexOptions,
    ) -> bool {
        let Some(session) = self.apex_session() else {
            return false;
        };
        if !session.boundary.is_current(&self.document) {
            self.push_log("Error: boundary surface changed; pick a new boundary".into());
            self.begin_surface_apex_boundary();
            return false;
        }
        let target = match session.boundary {
            Boundary::Picked { reference, .. } => format!(" ToBoundary={reference}"),
            Boundary::None => String::new(),
            Boundary::Selecting => return false,
        };
        self.finish_surface_face_command(&format!(
            "ExtrudeSrfToPoint {} {}{target}",
            format_model_point(apex),
            options.command_options()
        ))
    }

    pub(super) fn try_continue_surface_apex(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeSurfaceToPoint { mut options }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.set_view_prompt.is_some() {
            return false;
        }
        let token = input.trim().trim_start_matches(['_', '-']);
        if let Some(value) = token
            .split_once('=')
            .filter(|(key, _)| key.eq_ignore_ascii_case("ToBoundary"))
            .map(|(_, value)| value)
        {
            self.continue_surface_apex_boundary(value);
            return true;
        }
        if token.eq_ignore_ascii_case("ToBoundary") {
            self.begin_surface_apex_boundary();
            self.command_input.clear();
            return true;
        }
        if self.picking_surface_apex_boundary() {
            if token.is_empty()
                || token.eq_ignore_ascii_case("Enter")
                || token.eq_ignore_ascii_case("Cancel")
                || token == "!"
            {
                self.cancel_interactive_command(true);
                return true;
            }
            let id = token
                .split_once(char::is_whitespace)
                .filter(|(name, _)| {
                    name.trim_start_matches(['_', '\''])
                        .eq_ignore_ascii_case("SelID")
                })
                .map_or(token, |(_, id)| id.trim());
            self.continue_surface_apex_boundary(id);
            return true;
        }
        if let Some(apex) = ApexPoint::origin_token(input) {
            let plane = self.viewports[self.active_viewport].construction_plane();
            self.accept_filtered_drafting_point(apex.resolve(plane), true);
            self.command_input.clear();
            return true;
        }
        if viboceros_drafting::PointInput::parse_length_with_units(input, self.document.units())
            .is_some()
        {
            self.push_log(
                "Enter an apex point, 0 for the CPlane origin, or w0 for world origin".into(),
            );
            self.command_input.clear();
            return true;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty()
            || !words
                .iter()
                .all(|w| w.contains('=') || SurfaceApexOptions::recognizes(w))
        {
            return false;
        }
        match options.update(&words) {
            Ok(()) => {
                self.active_command = Some(InteractiveCommand::ExtrudeSurfaceToPoint { options });
                self.log_surface_apex(options);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }

    fn apex_session(&self) -> Option<&Session> {
        self.surface_extrusion_sources.as_ref()?.apex.as_ref()
    }

    fn apex_session_mut(&mut self) -> Option<&mut Session> {
        self.surface_extrusion_sources.as_mut()?.apex.as_mut()
    }

    fn begin_surface_apex_boundary(&mut self) {
        let Some(session) = self.apex_session_mut() else {
            return;
        };
        if let Some(draft) = &mut session.draft {
            draft.hover = None;
        }
        session.boundary = Boundary::Selecting;
        self.push_log("Select one boundary surface".into());
    }

    fn continue_surface_apex_boundary(&mut self, input: &str) {
        match BoundaryRef::parse(input) {
            Ok(reference) => self.pick_surface_apex_boundary(reference),
            Err(_) => self
                .push_log("Select one boundary surface or enter SelID with its object ID".into()),
        }
        self.command_input.clear();
    }

    fn log_surface_apex(&mut self, options: SurfaceApexOptions) {
        let message = if self
            .apex_session()
            .is_some_and(|s| matches!(s.boundary, Boundary::Picked { .. }))
        {
            format!("Point to extrude to ({})", options.boundary_options())
        } else {
            format!(
                "Point to extrude to ({} ToBoundary)",
                options.command_options()
            )
        };
        self.push_log(message);
    }
}

//! Solid ellipsoid base picks and the normal-axis prompt.

use super::*;
use viboceros_command::{
    curve_reference::CapturedCurve,
    ellipse_input::{Input, Options, Slot},
    ellipsoid::{Definition, ThirdAxis, base_option, commit},
};
use viboceros_geometry::{Ellipse3, GeometryError};

pub(super) struct Session {
    axis: ThirdAxis,
    guide: Option<CapturedCurve>,
    postselected: bool,
}

impl Session {
    fn current(&self, document: &Document) -> bool {
        self.guide
            .as_ref()
            .is_none_or(|guide| guide.current(document))
    }

    fn picked_guide(&self) -> Option<ObjectId> {
        self.guide
            .as_ref()
            .filter(|_| self.postselected)
            .map(CapturedCurve::object_id)
    }
}

impl VibocerosApp {
    pub(super) fn try_start_ellipsoid(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Ellipsoid")
        }) {
            return false;
        }
        let mut input = Input::new(
            Options {
                mark_foci: self.commands.ellipsoid_mark_foci(),
                ..Default::default()
            },
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|word| matches!(base_option(&mut input, word), Ok(true))) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.push_log(format!("> {text}"));
        self.start_solid_ellipsoid_ellipse(input);
        true
    }

    pub(super) fn start_ellipsoid_third_axis(
        &mut self,
        axis: ThirdAxis,
        guide: Option<CapturedCurve>,
        postselected: bool,
    ) {
        self.ellipsoid_session = Some(Session {
            axis,
            guide,
            postselected,
        });
        self.active_command = Some(InteractiveCommand::Ellipsoid {
            anchor: Some(axis.base.center()),
            reference: axis.base.quadrants().ok().map(|points| points[1]),
        });
        self.command_input.clear();
        self.push_log("Ellipsoid: pick the end of the third axis or enter its size".into());
    }

    pub(super) fn release_ellipsoid_pick(&mut self) {
        if let Some(id) = self
            .ellipsoid_session
            .as_ref()
            .and_then(Session::picked_guide)
            && self.document.is_selected(id)
        {
            let _ = self
                .document
                .select_objects_direct([id], SelectionMode::Remove);
        }
    }

    fn validate_ellipsoid_guide(&mut self) -> bool {
        if self
            .ellipsoid_session
            .as_ref()
            .is_some_and(|session| !session.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log(
                "Ellipsoid guide changed or became unavailable. Start the command again.".into(),
            );
            false
        } else {
            true
        }
    }

    pub(super) fn accept_focal_ellipsoid(&mut self, input: Input, base: Ellipse3) -> bool {
        self.finish_ellipsoid(
            Definition::focal(input, base, self.document.tolerance()),
            None,
        )
    }

    pub(super) fn accept_ellipsoid_point(&mut self, point: Point3) -> Option<bool> {
        self.ellipsoid_session.as_ref()?;
        if !self.validate_ellipsoid_guide() {
            return Some(false);
        }
        let session = self.ellipsoid_session.as_ref().unwrap();
        Some(self.finish_ellipsoid(session.axis.point(point), session.picked_guide()))
    }

    pub(super) fn continue_ellipsoid(&mut self, text: &str) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        if text == "0"
            && self.ellipse_session.as_ref().is_some_and(|s| {
                s.solid_ellipsoid() && s.input.slot() == Slot::First && !s.input.requests_curve()
            })
        {
            let origin = self.viewports[self.active_viewport]
                .construction_plane()
                .origin();
            self.accept_filtered_drafting_point(origin, true);
            return true;
        }
        if self.ellipsoid_session.is_none() {
            return false;
        }
        if !self.validate_ellipsoid_guide() {
            return true;
        }
        let Some(size) =
            viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
        else {
            return false;
        };
        match size {
            Ok(size) => {
                let session = self.ellipsoid_session.as_ref().unwrap();
                self.finish_ellipsoid(session.axis.number(size), session.picked_guide());
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
            }
        }
        true
    }

    fn finish_ellipsoid(
        &mut self,
        definition: Result<Definition, GeometryError>,
        release: Option<ObjectId>,
    ) -> bool {
        match definition
            .map_err(Into::into)
            .and_then(|d| commit(&mut self.document, d, release))
        {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.command_input.clear();
                self.push_log(message);
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn ellipsoid_preview(&self) -> Option<crate::viewport::EllipsoidPreview> {
        use crate::viewport::EllipsoidPreview;
        if self.plane_prompt.is_some() {
            return None;
        }
        if let Some(session) = &self.ellipsoid_session {
            return session
                .current(&self.document)
                .then_some(EllipsoidPreview::Third(session.axis));
        }
        self.ellipse_session
            .as_ref()
            .filter(|session| session.solid_ellipsoid())
            .map(|session| EllipsoidPreview::Base(session.input))
    }
}

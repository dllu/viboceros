//! Ellipsoid base picks and the normal-axis prompt.

use super::*;
mod output;
pub(super) use output::Output;
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
    output: Output,
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
        let Some(word) = words.next().map(|word| word.trim_start_matches(['_', '-'])) else {
            return false;
        };
        let (mut output, mark_foci) = if word.eq_ignore_ascii_case("MeshEllipsoid") {
            let preferences = self.commands.mesh_ellipsoid_preferences();
            (
                Output::Mesh {
                    preferences,
                    pending: None,
                },
                preferences.mark_foci,
            )
        } else if word.eq_ignore_ascii_case("Ellipsoid") {
            (Output::Solid, self.commands.ellipsoid_mark_foci())
        } else {
            return false;
        };
        let mut input = Input::new(
            Options {
                mark_foci,
                ..Default::default()
            },
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        if !words.all(|word| {
            output.edit_topology(word, true).map_or_else(
                || matches!(base_option(&mut input, word), Ok(true)),
                |result| result.is_ok(),
            )
        }) || output.validate_topology().is_err()
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.push_log(format!("> {text}"));
        self.start_ellipsoid_ellipse(input, output);
        true
    }

    pub(super) fn start_ellipsoid_third_axis(
        &mut self,
        axis: ThirdAxis,
        guide: Option<CapturedCurve>,
        postselected: bool,
        output: Output,
    ) {
        self.ellipsoid_session = Some(Session {
            axis,
            guide,
            postselected,
            output,
        });
        self.active_command = Some(output.command(
            Some(axis.base.center()),
            axis.base.quadrants().ok().map(|points| points[1]),
        ));
        self.command_input.clear();
        self.push_log(format!(
            "{}: pick the end of the third axis or enter its size",
            output.name()
        ));
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
            let name = self.ellipsoid_session.as_ref().unwrap().output.name();
            self.cancel_interactive_command(false);
            self.push_log(format!(
                "{name} guide changed or became unavailable. Start the command again."
            ));
            false
        } else {
            true
        }
    }

    pub(super) fn accept_focal_ellipsoid(
        &mut self,
        input: Input,
        base: Ellipse3,
        output: Output,
    ) -> bool {
        self.finish_ellipsoid(
            Definition::focal(input, base, self.document.tolerance()),
            None,
            output,
        )
    }

    pub(super) fn accept_ellipsoid_point(&mut self, point: Point3) -> Option<bool> {
        self.ellipsoid_session.as_ref()?;
        if !self.validate_ellipsoid_guide() {
            return Some(false);
        }
        let session = self.ellipsoid_session.as_ref().unwrap();
        Some(self.finish_ellipsoid(
            session.axis.point(point),
            session.picked_guide(),
            session.output,
        ))
    }

    pub(super) fn continue_ellipsoid(&mut self, text: &str) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        if text == "0"
            && self.ellipse_session.as_ref().is_some_and(|s| {
                s.ellipsoid_output().is_some()
                    && !s.waiting_for_topology()
                    && s.input.slot() == Slot::First
                    && !s.input.requests_curve()
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
        let mut output = self.ellipsoid_session.as_ref().unwrap().output;
        if let Some(Err(error)) = output.edit_topology(text, false) {
            self.command_file.fail_dispatch();
            self.push_log(format!("Error: {error}"));
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
                self.finish_ellipsoid(
                    session.axis.number(size),
                    session.picked_guide(),
                    session.output,
                );
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
        output: Output,
    ) -> bool {
        match definition.map_err(Into::into).and_then(|definition| {
            if let Some(topology) = output.topology() {
                viboceros_command::mesh_ellipsoid::commit(
                    &mut self.document,
                    definition,
                    topology,
                    release,
                )
            } else {
                commit(&mut self.document, definition, release)
            }
        }) {
            Ok(message) => {
                if let Output::Mesh { preferences, .. } = output {
                    self.commands
                        .remember_mesh_ellipsoid_preferences(preferences);
                }
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
                .then_some(EllipsoidPreview::Third(
                    session.axis,
                    session.output.topology(),
                ));
        }
        self.ellipse_session.as_ref().and_then(|session| {
            let output = session.ellipsoid_output()?;
            Some(EllipsoidPreview::Base(session.input, output.topology()))
        })
    }
}

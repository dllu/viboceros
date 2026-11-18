//! The accepting click uses the same prepared placement as the live overlay.
use super::*;

pub(super) struct Preview {
    options: AlignmentOptions,
    plane: Frame3,
    pub(super) rectangle: Option<viboceros_command::rectangle_input::RectangleDefinition>,
    pub(super) placement: Result<AlignedPlacement, CommandError>,
}

impl Preview {
    pub(super) fn matches(
        &self,
        options: AlignmentOptions,
        plane: Frame3,
        document: &Document,
    ) -> bool {
        self.options == options
            && (options.world || self.plane == plane)
            && self
                .placement
                .as_ref()
                .map_or(true, |p| p.target_is_current(document))
    }
}

impl VibocerosApp {
    pub(in crate::app) fn validate_align(&mut self) {
        if self.align_session.as_ref().is_some_and(|session| {
            !session.sources.is_current(&self.document)
                || session.components != self.alignment_components()
        }) {
            self.cancel_interactive_command(false);
            self.push_log(format!("Error: {}", CommandError::AlignStale));
        }
    }

    pub(in crate::app) fn update_align_preview(
        &mut self,
        point: Option<Point3>,
        curve: Option<ObjectId>,
        viewport: usize,
    ) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(InteractiveCommand::Align { options, .. }) = self.active_command else {
            return false;
        };
        let Some(session) = &mut self.align_session else {
            return false;
        };
        if session.frame_choice {
            return session.preview.take().is_some();
        }
        let draft = if options.mode == Some(AlignmentMode::ToCurve) {
            curve
                .filter(|&id| !session.sources.includes_owner(id))
                .map(|id| AlignmentOptions {
                    curve: Some(id),
                    ..options
                })
        } else if options.mode.is_some() {
            point
                .and_then(|point| options.with_point(point).ok())
                .filter(|o| o.ready())
        } else {
            None
        };
        let Some(options) = draft else {
            return session.preview.take().is_some();
        };
        let plane = self.viewports[viewport].construction_plane();
        if session
            .preview
            .as_ref()
            .is_some_and(|p| p.matches(options, plane, &self.document))
        {
            return false;
        }
        let rectangle = options.references[2]
            .and_then(|point| session.rectangle.input(options, plane).ok()?.preview(point));
        let placement = session.placement(&self.document, options, plane);
        session.preview = Some(Preview {
            options,
            plane,
            rectangle,
            placement,
        });
        true
    }
}

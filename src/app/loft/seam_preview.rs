//! Solve the current seam positions without blocking drag events.
use super::{
    preview::{Draft, Profiles},
    *,
};
use viboceros_geometry::NurbsCurve;

#[derive(Debug, PartialEq)]
pub(super) struct Request {
    placements: Vec<(f64, bool)>,
    options: Options,
    crease: bool,
}

impl Seams {
    pub(super) fn wires(&self) -> &[NurbsCurve] {
        self.preview
            .latest()
            .map_or(&[], |(_, draft)| draft.wires.as_slice())
    }

    fn request(&self, options: Options, crease: bool) -> Request {
        Request {
            placements: self.editor.placements().collect(),
            options,
            crease,
        }
    }

    pub(super) fn take_preview(
        &mut self,
        options: Options,
        crease: bool,
    ) -> Option<(Profiles, Draft)> {
        self.preview.take(&self.request(options, crease))
    }

    fn update_preview(
        &mut self,
        options: Options,
        crease: bool,
        tolerance: Tolerance,
        context: &egui::Context,
    ) {
        let request = self.request(options, crease);
        self.preview.update(
            Some(request),
            context,
            |previous, current| {
                previous.crease == current.crease && previous.options == current.options
            },
            |_| {
                let editor = self.editor.clone();
                let oriented = self.oriented;
                let supports = self.supports.clone();
                move || {
                    let profiles = Profiles::from_seams(&editor, oriented, tolerance).ok()?;
                    let draft = profiles.build(options, crease, tolerance, &supports).ok()?;
                    Some((profiles, draft))
                }
            },
        );
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_loft_seam_preview(&mut self, context: &egui::Context) {
        if !self
            .loft_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.seams.is_some() && prompt.review.is_none())
        {
            return;
        }
        if !self.validate_loft() {
            context.request_repaint();
            return;
        }
        let prompt = self.loft_prompt.as_mut().unwrap();
        let seams = prompt.seams.as_mut().unwrap();
        seams.update_preview(
            prompt.state.options,
            self.commands.crease_splitting(),
            self.document.tolerance(),
            context,
        );
    }
}

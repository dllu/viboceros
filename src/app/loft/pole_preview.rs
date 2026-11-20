//! Solve the current pole without blocking viewport input or editing the model.
use super::{
    draft_worker::Worker,
    preview::{Draft, Profiles, Sections},
    *,
};
use std::sync::Arc;
use viboceros_command::loft;
use viboceros_geometry::{LoftProfiles, NurbsCurve};

#[derive(Debug)]
enum Source {
    Automatic(Vec<NurbsCurve>),
    Aligned(Profiles, Arc<[Option<loft::TangentSupport>; 2]>),
}

impl Source {
    fn build(
        &self,
        options: Options,
        crease: bool,
        tolerance: Tolerance,
    ) -> Result<Draft, CommandError> {
        match self {
            Self::Automatic(curves) => {
                loft::prepare(LoftProfiles::Automatic(curves), options, crease, tolerance)
                    .map(|brep| Draft::new(brep, tolerance))
            }
            Self::Aligned(profiles, supports) => {
                profiles.build(options, crease, tolerance, supports)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    point: Point3,
    crease: bool,
}

#[derive(Debug)]
pub(super) struct Preview {
    source: Arc<Source>,
    options: Options,
    end: End,
    tolerance: Tolerance,
    worker: Worker<Request, Vec<NurbsCurve>>,
}

impl Preview {
    pub(super) fn wires(&self) -> &[NurbsCurve] {
        self.worker.latest().map_or(&[], Vec::as_slice)
    }

    fn update(&mut self, request: Option<Request>, context: &egui::Context) {
        self.worker.update(
            request,
            context,
            |previous, current| previous.crease == current.crease,
            |&request| {
                let mut options = self.options;
                self.end.set_point(&mut options, request.point);
                let source = self.source.clone();
                let tolerance = self.tolerance;
                move || {
                    source
                        .build(options, request.crease, tolerance)
                        .ok()
                        .map(|draft| draft.wires)
                }
            },
        );
    }
}

impl VibocerosApp {
    pub(super) fn begin_loft_pole_preview(&mut self, end: End) {
        let prompt = self.loft_prompt.as_ref().unwrap();
        let tolerance = self.document.tolerance();
        let source = (|| {
            if prompt.state.sources.is_empty() {
                return None;
            }
            if prompt.uses_script() {
                return Some(Source::Automatic(
                    prompt
                        .state
                        .sources
                        .iter()
                        .map(|section| section.source.curve().clone())
                        .collect(),
                ));
            }
            let (profiles, supports) = match self.prepare_loft_sections().ok()? {
                Sections::Open(profiles) => {
                    let supports = profiles.capture_supports(&self.document);
                    (profiles, supports)
                }
                Sections::Closed(seams) => {
                    (seams.aligned_profiles(tolerance).ok()?, seams.supports)
                }
            };
            Some(Source::Aligned(profiles, supports))
        })();
        let options = prompt.state.options;
        self.loft_prompt.as_mut().unwrap().pole_preview = source.map(|source| Preview {
            source: Arc::new(source),
            options,
            end,
            tolerance,
            worker: Worker::default(),
        });
    }

    pub(in crate::app) fn update_loft_pole_preview(
        &mut self,
        point: Option<Point3>,
        context: &egui::Context,
    ) {
        if !self
            .loft_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.pole_preview.is_some())
        {
            return;
        }
        if !self.validate_loft() {
            context.request_repaint();
            return;
        }
        let request = point.map(|point| Request {
            point,
            crease: self.commands.crease_splitting(),
        });
        let preview = self
            .loft_prompt
            .as_mut()
            .unwrap()
            .pole_preview
            .as_mut()
            .unwrap();
        preview.update(request, context);
    }
}

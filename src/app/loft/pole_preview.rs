//! Solve the current pole without blocking viewport input or editing the model.
use super::{
    preview::{Draft, Profiles, Sections},
    *,
};
use std::sync::{Arc, mpsc};
use viboceros_command::loft;
use viboceros_geometry::{LoftProfiles, NurbsCurve};

#[derive(Debug)]
enum Source {
    Automatic(Vec<NurbsCurve>),
    Aligned(Profiles, Box<[Option<loft::TangentSupport>; 2]>),
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
    job: Option<(Request, mpsc::Receiver<Vec<NurbsCurve>>)>,
    result: Option<(Request, Vec<NurbsCurve>)>,
}

impl Preview {
    pub(super) fn wires(&self) -> &[NurbsCurve] {
        self.result
            .as_ref()
            .map_or(&[], |(_, wires)| wires.as_slice())
    }

    fn update(&mut self, request: Option<Request>, context: &egui::Context) {
        if self.result.as_ref().is_some_and(|(previous, _)| {
            !request.is_some_and(|current| current.crease == previous.crease)
        }) {
            self.result = None;
            context.request_repaint();
        }
        if let Some((pending, receiver)) = &self.job {
            let wires = match receiver.try_recv() {
                Ok(wires) => wires,
                Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
                Err(mpsc::TryRecvError::Empty) => return,
            };
            if request.is_some_and(|current| current.crease == pending.crease) {
                // Publish completed positions during motion and coalesce any
                // intervening input into the next solve.
                self.result = Some((*pending, wires));
                context.request_repaint();
            }
            self.job = None;
        }
        let Some(request) = request else { return };
        if self
            .result
            .as_ref()
            .is_some_and(|(previous, _)| *previous == request)
        {
            return;
        }
        let mut options = self.options;
        self.end.set_point(&mut options, request.point);
        let source = self.source.clone();
        let tolerance = self.tolerance;
        let worker_context = context.clone();
        let (send, receive) = mpsc::channel();
        if std::thread::Builder::new()
            .name("loft-pole-preview".into())
            .spawn(move || {
                let wires = source
                    .build(options, request.crease, tolerance)
                    .map_or_else(|_| Vec::new(), |draft| draft.wires);
                if send.send(wires).is_ok() {
                    worker_context.request_repaint();
                }
            })
            .is_ok()
        {
            self.job = Some((request, receive));
        } else {
            self.result = Some((request, Vec::new()));
            context.request_repaint();
        }
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
            let profiles = match self.prepare_loft_sections().ok()? {
                Sections::Open(profiles) => profiles,
                Sections::Closed(seams) => seams.aligned_profiles(tolerance).ok()?,
            };
            let supports = profiles.capture_supports(&self.document);
            Some(Source::Aligned(profiles, Box::new(supports)))
        })();
        let options = prompt.state.options;
        self.loft_prompt.as_mut().unwrap().pole_preview = source.map(|source| Preview {
            source: Arc::new(source),
            options,
            end,
            tolerance,
            job: None,
            result: None,
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

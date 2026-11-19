//! Native wire geometry for the current tapered distance.
use super::sources::Profile;
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use viboceros_command::{
    extrude_surface_tapered::produce_surface,
    extrude_tapered::{Distance, produce_curve},
};
use viboceros_geometry::NurbsCurve;

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Request {
    pub height: f64,
    pub options: TaperedExtrusionOptions,
    pub tolerance: Tolerance,
}

struct Job {
    request: Request,
    receiver: mpsc::Receiver<Vec<NurbsCurve>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct Preview {
    job: Option<Job>,
    result: Option<(Request, Vec<NurbsCurve>)>,
}

impl Drop for Preview {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Preview {
    fn update(
        &mut self,
        request: Option<Request>,
        sources: &Sources,
        plane: Frame3,
        context: &egui::Context,
    ) {
        if let Some(job) = &self.job {
            if request != Some(job.request) {
                job.cancel.store(true, Ordering::Relaxed);
            }
            let edges = match job.receiver.try_recv() {
                Ok(edges) => edges,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
            };
            if !job.cancel.load(Ordering::Relaxed) {
                self.result = Some((job.request, edges));
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
        let (sender, receiver) = mpsc::channel();
        let profiles = sources.profiles.clone();
        let context = context.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        if std::thread::Builder::new()
            .name("tapered-extrusion-preview".into())
            .spawn(move || {
                let mut edges = Vec::new();
                let distance = Distance::Numeric(request.height);
                for profile in profiles.iter() {
                    if worker_cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let brep = match profile {
                        Profile::Curve(curve) => produce_curve(
                            curve,
                            distance,
                            request.options,
                            plane,
                            request.tolerance,
                        ),
                        Profile::Face(face) => produce_surface(
                            face,
                            distance,
                            request.options,
                            plane,
                            request.tolerance,
                        ),
                    };
                    if let Ok(brep) = brep {
                        edges.extend(brep.edges().iter().map(|edge| edge.curve().clone()));
                    }
                }
                let _ = sender.send(edges);
                context.request_repaint();
            })
            .is_ok()
        {
            self.job = Some(Job {
                request,
                receiver,
                cancel,
            });
        } else {
            self.result = Some((request, Vec::new()));
        }
    }
}

impl Session {
    pub(in crate::app) fn preview_edges(
        &self,
        document: &Document,
        command: Option<InteractiveCommand>,
    ) -> Option<(f64, &[NurbsCurve])> {
        let Some(InteractiveCommand::ExtrudeTapered {
            stage: Stage::Distance,
            options,
            ..
        }) = command
        else {
            return None;
        };
        let (request, edges) = self.preview.result.as_ref()?;
        (request.options == options
            && request.tolerance == document.tolerance()
            && self.sources.is_current(document))
        .then_some((request.height, edges))
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_tapered_extrusion_preview(
        &mut self,
        point: Option<Point3>,
        context: &egui::Context,
    ) {
        let height = self.update_tapered_hover(point);
        let Some(InteractiveCommand::ExtrudeTapered { options, .. }) = self.active_command else {
            return;
        };
        let Some(session) = self.tapered_extrusion_session.as_mut() else {
            return;
        };
        let request = height.map(|height| Request {
            height,
            options,
            tolerance: self.document.tolerance(),
        });
        session
            .preview
            .update(request, &session.sources, session.plane, context);
    }
}

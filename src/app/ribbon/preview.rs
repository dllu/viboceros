//! Solve the ribbon at the current side without putting draft geometry into the model.
use super::*;
use std::sync::{Arc, mpsc};
use viboceros_geometry::CurveRef;

#[derive(Clone, Copy, PartialEq)]
struct Request {
    options: Options,
    distance: f64,
    through_point: bool,
    side_point: Option<Point3>,
}

impl Request {
    fn compatible_with(self, current: Self) -> bool {
        self == current
            || (self.through_point
                && current.through_point
                && self.options == current.options
                && self.side_point == current.side_point
                && self.distance.is_sign_positive() == current.distance.is_sign_positive())
    }
}

#[derive(Default)]
pub(super) struct Preview {
    source: Option<(bool, Option<Arc<solver::Source>>)>,
    current: Option<Request>,
    result: Option<(Request, Vec<NurbsCurve>)>,
    job: Option<(Request, mpsc::Receiver<Vec<NurbsCurve>>)>,
}

impl Preview {
    fn source(
        &mut self,
        curve: CurveRef<'_>,
        in_cplane: bool,
        plane: Frame3,
        tolerance: Tolerance,
    ) -> Option<Arc<solver::Source>> {
        if self
            .source
            .as_ref()
            .is_none_or(|(fixed, _)| *fixed != in_cplane)
        {
            let context = viboceros_command::CommandContext {
                construction_plane: plane,
            };
            self.source = Some((
                in_cplane,
                solver::Source::capture(curve, in_cplane, context, tolerance)
                    .ok()
                    .map(Arc::new),
            ));
        }
        self.source.as_ref()?.1.clone()
    }

    fn update(&mut self, input: Option<(Request, Arc<solver::Source>)>, ctx: &egui::Context) {
        let current = input.as_ref().map(|(request, _)| *request);
        if self.current != current {
            self.current = current;
            ctx.request_repaint();
        }
        if let Some((request, receiver)) = &self.job {
            let edges = match receiver.try_recv() {
                Ok(edges) => edges,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
            };
            if self
                .current
                .is_some_and(|current| request.compatible_with(current))
            {
                self.result = Some((*request, edges));
            }
            self.job = None;
            ctx.request_repaint();
        }
        let Some((request, source)) = input else {
            return;
        };
        if self.result.as_ref().is_some_and(|(key, _)| *key == request) {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        if std::thread::Builder::new()
            .name("ribbon-preview".into())
            .spawn(move || {
                let mut edges = Vec::new();
                if let Ok(outputs) =
                    source.solve_at_pick(request.options, request.distance, request.side_point)
                {
                    for brep in outputs {
                        edges.extend(brep.edges().iter().map(|edge| edge.curve().clone()));
                    }
                }
                if sender.send(edges).is_ok() {
                    ctx.request_repaint();
                }
            })
            .is_ok()
        {
            self.job = Some((request, receiver));
        } else {
            self.result = Some((request, Vec::new()));
        }
    }
}

impl Session {
    pub(in crate::app) fn preview_curves(&self, document: &Document) -> Option<&[NurbsCurve]> {
        let (request, edges) = self.preview.result.as_ref()?;
        (self
            .preview
            .current
            .is_some_and(|current| request.compatible_with(current))
            && self.options == request.options
            && self.through_point == request.through_point
            && self.menu.is_none()
            && self.current(document))
        .then_some(edges.as_slice())
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_ribbon_preview(
        &mut self,
        point: Option<Point3>,
        ctx: &egui::Context,
    ) {
        let Some(session) = &mut self.ribbon_session else {
            return;
        };
        let input = (|| {
            let point = point?;
            if session.menu.is_some() || !session.current(&self.document) {
                return None;
            }
            let options = session.options;
            let curve = session
                .portion
                .as_ref()
                .map(|portion| portion.curve.as_ref())
                .or_else(|| session.source.geometry().curve_ref())?;
            let source = session.preview.source(
                curve,
                options.in_cplane,
                session.plane,
                session.tolerance,
            )?;
            let through_point = session.through_point;
            let distance = source.signed_distance(options, point, through_point).ok()?;
            Some((
                Request {
                    options,
                    distance,
                    through_point,
                    side_point: source.preview_pick(options, distance, point),
                },
                source,
            ))
        })();
        session.preview.update(input, ctx);
    }
}

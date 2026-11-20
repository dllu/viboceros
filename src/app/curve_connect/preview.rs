//! Read-only previews of the current two-end connection.
use super::*;
use std::sync::mpsc;

struct Request {
    second: connect::Source,
    options: connect::Options,
}

impl Request {
    fn matches(&self, other: &Self, document: &Document) -> bool {
        self.options == other.options
            && self.second.id() == other.second.id()
            && self.second.edits_start() == other.second.edits_start()
            && self.second.is_current(document)
    }
}

#[derive(Default)]
pub(super) struct Preview {
    job: Option<(Request, mpsc::Receiver<Vec<NurbsCurve>>)>,
    result: Option<(Request, Vec<NurbsCurve>)>,
}

impl Preview {
    pub(super) fn curves(
        &self,
        doc: &Document,
        options: connect::Options,
    ) -> Option<&[NurbsCurve]> {
        let (request, curves) = self.result.as_ref()?;
        (request.options == options && request.second.is_current(doc)).then_some(curves.as_slice())
    }

    fn update(
        &mut self,
        doc: &Document,
        input: Option<(&connect::Source, Request)>,
        ctx: &egui::Context,
    ) {
        let matches = |previous: &Request| {
            input
                .as_ref()
                .is_some_and(|(_, next)| previous.matches(next, doc))
        };
        if self
            .result
            .as_ref()
            .is_some_and(|(request, _)| !matches(request))
        {
            self.result = None;
            ctx.request_repaint();
        }
        if let Some((request, receiver)) = &self.job {
            let curves = match receiver.try_recv() {
                Ok(curves) => curves,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
            };
            if matches(request) {
                let (request, _) = self.job.take().unwrap();
                self.result = Some((request, curves));
                ctx.request_repaint();
            }
            self.job = None;
        }
        let Some((first, request)) = input else {
            return;
        };
        if self.result.is_some() {
            return;
        }
        let tolerance = doc.tolerance();
        let sources = [first.clone(), request.second.clone()];
        let options = request.options;
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        if std::thread::Builder::new()
            .name("connect-preview".into())
            .spawn(move || {
                let curves = connect::preview(sources.each_ref(), options, tolerance)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|g| g.nurbs_curve_representation().ok().flatten())
                    .collect();
                let _ = sender.send(curves);
                ctx.request_repaint();
            })
            .is_ok()
        {
            self.job = Some((request, receiver));
        } else {
            self.result = Some((request, Vec::new()));
        }
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_curve_connect_preview(
        &mut self,
        hover: Option<(ObjectId, Point3)>,
        ctx: &egui::Context,
    ) {
        let picking = self.picking_curve_connect();
        let Some(session) = &mut self.curve_connect_session else {
            return;
        };
        let doc = &self.document;
        let input = (|| {
            if !picking {
                return None;
            }
            let first = session.first.as_ref()?;
            let (id, point) = hover?;
            if id == first.id() || !first.is_current(doc) {
                return None;
            }
            let second = connect::Source::capture(doc, id, Some(point), None).ok()?;
            Some((
                first,
                Request {
                    second,
                    options: session.options,
                },
            ))
        })();
        session.preview.update(doc, input, ctx);
    }
}

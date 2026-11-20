//! Keep cursor tracking responsive while the kernel computes a curve draft.
use super::*;
use std::sync::mpsc;
use viboceros_geometry::NurbsCurve;

type Key = (Target, Options);
type Draft = (curve_extend::Prepared, Vec<NurbsCurve>);

#[derive(Default)]
pub(super) struct Preview {
    pending: Option<(Key, mpsc::Receiver<Option<Draft>>)>,
    latest: Option<(Key, Option<Draft>)>,
}

impl Preview {
    pub(super) fn curves(&self) -> Option<&[NurbsCurve]> {
        self.latest
            .as_ref()?
            .1
            .as_ref()
            .map(|(_, curves)| curves.as_slice())
    }

    pub(super) fn take(
        &mut self,
        target: Target,
        options: Options,
    ) -> Option<curve_extend::Prepared> {
        let key = (target, options);
        if self.latest.as_ref()?.0 == key {
            self.latest.take()?.1.map(|(prepared, _)| prepared)
        } else {
            None
        }
    }

    fn update(&mut self, source: &Source, input: Option<Key>, ctx: &egui::Context) {
        let Some(key) = input else {
            if self.latest.take().is_some() {
                ctx.request_repaint();
            }
            self.pending = None;
            return;
        };
        if let Some((computed, channel)) = &self.pending {
            let draft = match channel.try_recv() {
                Ok(value) => value,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => None,
            };
            self.latest = Some((*computed, draft));
            self.pending = None;
            ctx.request_repaint();
        }
        if self
            .latest
            .as_ref()
            .is_some_and(|(computed, _)| *computed == key)
        {
            return;
        }
        let source = source.clone();
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        if std::thread::Builder::new()
            .name("extend-preview".into())
            .spawn(move || {
                let draft = curve_extend::prepare(&source, &[], key.0, key.1)
                    .ok()
                    .and_then(|prepared| {
                        let curve = prepared
                            .geometry()
                            .nurbs_curve_representation()
                            .ok()
                            .flatten()?;
                        Some((prepared, vec![curve]))
                    });
                let _ = sender.send(draft);
                ctx.request_repaint();
            })
            .is_ok()
        {
            self.pending = Some((key, receiver));
        }
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_curve_extend_preview(
        &mut self,
        hover: Option<Point3>,
        ctx: &egui::Context,
    ) {
        let Some(session) = &mut self.curve_extend_session else {
            return;
        };
        let Step::End(source, input) = &session.step else {
            return;
        };
        let target = hover
            .filter(|_| session.getter.is_none())
            .and_then(|point| input.target(point));
        session.preview.update(
            source,
            target.map(|target| (target, session.prefs.options)),
            ctx,
        );
    }
}

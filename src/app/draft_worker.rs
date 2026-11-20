//! Keep at most one preview solve in flight per interactive stage.
use super::egui;
use std::sync::mpsc;
use viboceros_geometry::{Brep, NurbsCurve, Tolerance};

pub(super) fn wireframe(brep: &Brep, tolerance: Tolerance) -> Vec<NurbsCurve> {
    brep.wireframe_curves(1, tolerance).unwrap_or_else(|_| {
        brep.edges()
            .iter()
            .map(|edge| edge.curve().clone())
            .collect()
    })
}

#[derive(Debug)]
pub(super) struct Worker<K, T> {
    job: Option<(K, mpsc::Receiver<Option<T>>)>,
    result: Option<(K, Option<T>)>,
}

impl<K, T> Default for Worker<K, T> {
    fn default() -> Self {
        Self {
            job: None,
            result: None,
        }
    }
}

impl<K: PartialEq, T: Send + 'static> Worker<K, T> {
    pub(super) fn latest(&self) -> Option<&T> {
        self.result.as_ref()?.1.as_ref()
    }

    pub(super) fn clear_result(&mut self) {
        self.result = None;
    }

    pub(super) fn retain_as(&mut self, previous: &K, accepted: K)
    where
        K: Clone,
    {
        if let Some((request, _)) = &mut self.job
            && request == previous
        {
            *request = accepted.clone();
        }
        if let Some((request, _)) = &mut self.result
            && request == previous
        {
            *request = accepted;
        } else {
            self.clear_result();
        }
    }

    pub(super) fn take(&mut self, request: &K) -> Option<T> {
        if self
            .result
            .as_ref()
            .is_some_and(|(previous, _)| previous == request)
        {
            return self.result.take()?.1;
        }
        if let Some((pending, receiver)) = &self.job
            && pending == request
            && let Ok(draft) = receiver.try_recv()
        {
            self.job = None;
            return draft;
        }
        None
    }

    pub(super) fn update<F>(
        &mut self,
        request: Option<K>,
        context: &egui::Context,
        compatible: impl Fn(&K, &K) -> bool,
        prepare: impl FnOnce(&K) -> F,
    ) where
        F: FnOnce() -> Option<T> + Send + 'static,
    {
        let matches = |previous: &K| {
            request
                .as_ref()
                .is_some_and(|current| compatible(previous, current))
        };
        if self
            .result
            .as_ref()
            .is_some_and(|(previous, _)| !matches(previous))
        {
            self.result = None;
            context.request_repaint();
        }
        if let Some((pending, receiver)) = self.job.take() {
            let draft = match receiver.try_recv() {
                Ok(draft) => draft,
                Err(mpsc::TryRecvError::Disconnected) => None,
                Err(mpsc::TryRecvError::Empty) => {
                    self.job = Some((pending, receiver));
                    return;
                }
            };
            if matches(&pending) {
                // Show completed positions during motion and coalesce
                // intervening edits into the next solve.
                self.result = Some((pending, draft));
                context.request_repaint();
            }
        }
        let Some(request) = request else { return };
        if self
            .result
            .as_ref()
            .is_some_and(|(previous, _)| *previous == request)
        {
            return;
        }
        let solve = prepare(&request);
        let worker_context = context.clone();
        let (send, receive) = mpsc::channel();
        if std::thread::Builder::new()
            .name("surface-preview".into())
            .spawn(move || {
                if send.send(solve()).is_ok() {
                    worker_context.request_repaint();
                }
            })
            .is_ok()
        {
            self.job = Some((request, receive));
        } else {
            self.result = Some((request, None));
            context.request_repaint();
        }
    }
}

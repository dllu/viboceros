//! One native boundary solve at a time, using the frozen source faces.
use super::*;
use std::sync::{Arc, mpsc};
use viboceros_command::extrude_surface_path::{prepare_surface_path_range, preview_boundary_edges};
use viboceros_geometry::Brep;

type Parameters = (f64, Tolerance);

pub(super) struct Preview {
    sources: Arc<[Brep]>,
    cutter: Arc<Brep>,
    job: Option<(Parameters, mpsc::Receiver<Vec<NurbsCurve>>)>,
    result: Option<(Parameters, Vec<NurbsCurve>)>,
}

impl Preview {
    pub(super) fn capture(
        doc: &Document,
        targets: &[(ObjectId, usize)],
        cutter: &Brep,
    ) -> Option<Self> {
        let tolerance = doc.tolerance();
        let sources = targets
            .iter()
            .map(|&(id, face)| match doc.object(id)?.geometry() {
                Geometry::Brep(brep) => brep.duplicate_faces(&[face], tolerance).ok(),
                Geometry::NurbsSurface(surface) => {
                    Brep::try_surface_face(surface.clone(), tolerance).ok()
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            sources: sources.into(),
            cutter: Arc::new(cutter.clone()),
            job: None,
            result: None,
        })
    }

    fn update(
        &mut self,
        parameters: Parameters,
        curve: &NurbsCurve,
        start: f64,
        reverse: bool,
        split: bool,
        context: &egui::Context,
    ) {
        if let Some((pending, receiver)) = &self.job {
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Vec::new(),
            };
            let pending = *pending;
            self.result = Some((pending, result));
            self.job = None;
            if pending == parameters {
                context.request_repaint();
            }
        }
        if self
            .result
            .as_ref()
            .is_some_and(|(previous, _)| *previous == parameters)
        {
            return;
        }
        let (end, tolerance) = parameters;
        let source = self.sources.clone();
        let cutter = self.cutter.clone();
        let curve = curve.clone();
        let context = context.clone();
        let (sender, receiver) = mpsc::channel();
        if std::thread::Builder::new()
            .name("surface-path-preview".into())
            .spawn(move || {
                let result = prepare_surface_path_range(&curve, [start, end], reverse)
                    .and_then(|path| {
                        preview_boundary_edges(&source, &path, &cutter, split, tolerance)
                    })
                    .unwrap_or_default();
                let _ = sender.send(result);
                context.request_repaint();
            })
            .is_ok()
        {
            self.job = Some((parameters, receiver));
        } else {
            self.result = Some((parameters, Vec::new()));
        }
    }
}

impl SurfacePathSession {
    pub(in crate::app) fn boundary_preview(&self, doc: &Document) -> Option<(f64, &[NurbsCurve])> {
        let points = self.points.as_ref()?;
        if !self.sources.is_current(doc)
            || !self.boundary.is_current(doc)
            || !points.is_current(doc)
        {
            return None;
        }
        let ((end, tolerance), edges) = points.boundary_preview.as_ref()?.result.as_ref()?;
        (*tolerance == doc.tolerance()).then_some((*end, edges))
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_surface_path_boundary_preview(
        &mut self,
        end: Option<f64>,
        context: &egui::Context,
    ) {
        let Some((session, end)) = self.surface_path_prompt.as_mut().zip(end) else {
            return;
        };
        let Some(points) = session.points.as_mut() else {
            return;
        };
        let Some(start) = points.start else { return };
        if start == end
            || !session.sources.is_current(&self.document)
            || !session.boundary.is_current(&self.document)
            || !points.is_current(&self.document)
        {
            return;
        }
        if let Some(preview) = &mut points.boundary_preview {
            preview.update(
                (end, self.document.tolerance()),
                &points.curve,
                start,
                points.reverse,
                session.options.split_at_tangents,
                context,
            );
        }
    }
}

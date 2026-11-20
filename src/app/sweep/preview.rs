//! Sweep drafts use the normal command solver and keep the document immutable.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{Brep, NurbsCurve};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    revision: u64,
    tolerance: Tolerance,
    pole: Option<(End, Point3)>,
}

#[derive(Debug)]
struct Draft {
    brep: Brep,
    sections: usize,
    wires: Vec<NurbsCurve>,
}

#[derive(Debug, Default)]
pub(super) struct Preview {
    revision: u64,
    source: Option<Option<Arc<Source>>>,
    worker: draft_worker::Worker<Request, Draft>,
}

impl Preview {
    pub(super) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.source = None;
        self.clear_wires();
    }

    pub(super) fn clear_wires(&mut self) {
        self.worker.clear_result();
    }
}

impl Session {
    pub(in crate::app) fn preview_wires(&self) -> &[NurbsCurve] {
        self.preview
            .worker
            .latest()
            .map_or(&[], |draft| &draft.wires)
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_sweep_preview(
        &mut self,
        point: Option<Point3>,
        context: &egui::Context,
    ) {
        if !self
            .sweep_session
            .as_ref()
            .is_some_and(|p| p.profiles() != 0)
        {
            return;
        }
        if !self.validate_sweep_sources() {
            context.request_repaint();
            return;
        }
        let p = self.sweep_session.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let pole = p.picking.zip(point);
        // During a point getter, dropping the resolved cursor also hides the
        // provisional shape. Outside the getter, show the accepted settings.
        let request = (p.picking.is_none() || pole.is_some()).then_some(Request {
            revision: p.preview.revision,
            tolerance,
            pole,
        });
        p.preview.worker.update(
            request,
            context,
            |previous, current| {
                previous.revision == current.revision
                    && previous.tolerance == current.tolerance
                    && previous.pole.map(|(end, _)| end) == current.pole.map(|(end, _)| end)
            },
            |&request| {
                let source = p
                    .preview
                    .source
                    .get_or_insert_with(|| {
                        p.state.capture_source(&self.document).ok().map(Arc::new)
                    })
                    .clone();
                let mut options = p.state.options.clone();
                if let Some((end, point)) = request.pole {
                    end.set_point(&mut options, point);
                }
                move || {
                    let (brep, sections) = source?.build(&options, tolerance).ok()?;
                    let wires = draft_worker::wireframe(&brep, tolerance);
                    Some(Draft {
                        brep,
                        sections,
                        wires,
                    })
                }
            },
        );
    }

    pub(super) fn commit_sweep_preview(&mut self) {
        let session = self.sweep_session.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let request = Request {
            revision: session.preview.revision,
            tolerance,
            pole: None,
        };
        let result =
            if let Some(Draft { brep, sections, .. }) = session.preview.worker.take(&request) {
                Ok((brep, sections))
            } else {
                session
                    .state
                    .capture_source(&self.document)
                    .and_then(|source| source.build(&session.state.options, tolerance))
            };
        let (brep, sections) = match result {
            Ok(result) => result,
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        let faces = brep.faces().len();
        match edit_document_transaction(&mut self.document, "Sweep1", |document| {
            document.add_geometry(Geometry::Brep(brep))
        }) {
            Ok(_) => {
                self.sweep_session = None;
                self.object_prompt = None;
                self.push_log(format!(
                    "Swept {sections} sections into one BRep with {faces} faces"
                ));
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
            }
        }
    }
}

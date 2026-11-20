//! Sweep drafts use the normal command solver and keep the document immutable.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{Brep, NurbsCurve, Real};

#[derive(Clone, Debug, PartialEq)]
struct Request {
    revision: u64,
    tolerance: Tolerance,
    change: Option<DraftChange>,
    seams: Vec<(Real, bool)>,
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

    pub(super) fn accept_change(
        &mut self,
        change: DraftChange,
        tolerance: Tolerance,
        stage: &ProfileStage,
    ) {
        let previous = self.request(tolerance, Some(change), stage);
        self.revision = self.revision.wrapping_add(1);
        self.worker
            .retain_as(&previous, self.request(tolerance, None, stage));
    }

    fn request(
        &self,
        tolerance: Tolerance,
        change: Option<DraftChange>,
        stage: &ProfileStage,
    ) -> Request {
        Request {
            revision: self.revision,
            tolerance,
            change,
            seams: stage
                .seams()
                .map_or_else(Vec::new, |editor| editor.placements().collect()),
        }
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
        let change = p.picking.zip(point).and_then(|(pick, point)| match pick {
            Pick::Pole(end) => Some(DraftChange::Pole(end, point)),
            Pick::AxisEnd(start) => axis::between(start, point)
                .ok()
                .map(|axis| DraftChange::Axis(Some(axis))),
            Pick::AxisStart => None,
        });
        // Keep the accepted draft until the base point is picked. A provisional
        // draft requires a resolved cursor and a usable direction.
        let ready = matches!(p.picking, None | Some(Pick::AxisStart)) || change.is_some();
        let request = ready.then(|| p.preview.request(tolerance, change, &p.state.stage));
        p.preview.worker.update(
            request,
            context,
            |previous, current| {
                previous.revision == current.revision
                    && previous.tolerance == current.tolerance
                    && previous.seams.len() == current.seams.len()
                    && match (previous.change, current.change) {
                        (None, None) | (Some(DraftChange::Axis(_)), Some(DraftChange::Axis(_))) => {
                            true
                        }
                        (Some(DraftChange::Pole(a, _)), Some(DraftChange::Pole(b, _))) => a == b,
                        _ => false,
                    }
            },
            |request| {
                let source = p
                    .preview
                    .source
                    .get_or_insert_with(|| {
                        p.state.capture_source(&self.document).ok().map(Arc::new)
                    })
                    .clone();
                let mut options = p.state.options.clone();
                if let Some(change) = request.change {
                    change.apply(&mut options);
                }
                let editor = p.state.stage.seams().cloned();
                move || {
                    let source =
                        seams::with_seam_profiles(source?, editor.as_ref(), tolerance).ok()?;
                    let (brep, sections) = source.build(&options, tolerance).ok()?;
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
        let request = session
            .preview
            .request(tolerance, None, &session.state.stage);
        let result =
            if let Some(Draft { brep, sections, .. }) = session.preview.worker.take(&request) {
                Ok((brep, sections))
            } else {
                session
                    .state
                    .capture_source(&self.document)
                    .and_then(|source| {
                        seams::with_seam_profiles(
                            Arc::new(source),
                            session.state.stage.seams(),
                            tolerance,
                        )
                    })
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

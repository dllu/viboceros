use super::*;
use viboceros_geometry::{Brep, NurbsCurve};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    revision: u64,
    tolerance: Tolerance,
    provisional: Option<(End, Point3)>,
}

#[derive(Debug)]
struct Draft {
    brep: Brep,
    wires: Vec<NurbsCurve>,
}

#[derive(Debug, Default)]
pub(super) struct Preview {
    revision: u64,
    worker: draft_worker::Worker<Request, Result<Draft, String>>,
    reported: Option<Request>,
}

impl Preview {
    pub(super) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.worker.clear_result();
    }

    pub(super) fn clear_wires(&mut self) {
        self.worker.clear_result();
    }

    pub(super) fn accept_point(&mut self, end: End, point: Point3, tolerance: Tolerance) {
        let previous = self.request(tolerance, Some((end, point)));
        self.revision = self.revision.wrapping_add(1);
        self.worker
            .retain_as(&previous, self.request(tolerance, None));
    }

    fn request(&self, tolerance: Tolerance, provisional: Option<(End, Point3)>) -> Request {
        Request {
            revision: self.revision,
            tolerance,
            provisional,
        }
    }
}

impl Session {
    pub(in crate::app) fn preview_wires(&self) -> &[NurbsCurve] {
        self.preview
            .worker
            .latest()
            .and_then(|result| result.as_ref().ok())
            .map_or(&[], |draft| draft.wires.as_slice())
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_sweep_two_rail_preview(
        &mut self,
        point: Option<Point3>,
        context: &egui::Context,
    ) {
        let Some(session) = &self.sweep_two_rails else {
            return;
        };
        if session.rail_count() < 2 || session.section_count() == 0 {
            return;
        }
        let provisional = session.picking.zip(point);
        let mut ends = session.state.ends;
        if let Some((end, point)) = provisional {
            end.set(&mut ends, point);
        }
        let ready = session.picking.is_none() || provisional.is_some();
        let session = self.sweep_two_rails.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let request = session.preview.request(tolerance, provisional);
        let fit = session.state.fit;
        let maintain_height = session.state.maintain_height;
        session.preview.worker.update(
            ready.then_some(request),
            context,
            |previous, current| {
                previous.revision == current.revision
                    && previous.tolerance == current.tolerance
                    && previous.provisional.map(|(end, _)| end)
                        == current.provisional.map(|(end, _)| end)
            },
            |_| {
                let source = session.state.capture(&self.document);
                move || {
                    Some(
                        source
                            .and_then(|source| {
                                if provisional.is_some() {
                                    source.validate_point_ends(ends, tolerance)?;
                                }
                                source.build(fit, ends, maintain_height, tolerance)
                            })
                            .map(|brep| {
                                let wires = draft_worker::wireframe(&brep, tolerance);
                                Draft { brep, wires }
                            })
                            .map_err(|error| error.to_string()),
                    )
                }
            },
        );
        if session.state.review
            && session.picking.is_none()
            && session.preview.reported != Some(request)
            && let Some(Err(error)) = session.preview.worker.latest()
        {
            session.preview.reported = Some(request);
            let message = format!("Sweep2 preview: {error}");
            self.push_log(message);
        }
    }

    pub(super) fn commit_sweep_two_rails(&mut self) {
        if !self.validate_sweep_two_rails() {
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let request = session.preview.request(tolerance, None);
        let result = if let Some(draft) = session.preview.worker.take(&request) {
            draft.map(|draft| draft.brep)
        } else {
            session
                .state
                .capture(&self.document)
                .and_then(|source| {
                    source.build(
                        session.state.fit,
                        session.state.ends,
                        session.state.maintain_height,
                        tolerance,
                    )
                })
                .map_err(|error| error.to_string())
        };
        let brep = match result {
            Ok(brep) => brep,
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        let message = viboceros_command::sweep_two_rails::result_message(
            session.section_count(),
            brep.faces().len(),
        );
        match edit_document_transaction(&mut self.document, "Sweep2", |document| {
            document.add_geometry(Geometry::Brep(brep))
        }) {
            Ok(_) => {
                self.sweep_two_rails = None;
                self.object_prompt = None;
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}

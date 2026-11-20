use super::*;
use viboceros_geometry::{Brep, NurbsCurve, Real};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Provisional {
    End(End, Point3),
    Slash([Point3; 2]),
}

impl Provisional {
    fn point_end(self) -> Option<End> {
        match self {
            Self::End(end, _) => Some(end),
            Self::Slash(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Request {
    revision: u64,
    tolerance: Tolerance,
    provisional: Option<Provisional>,
    seams: Vec<(Real, bool)>,
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

    pub(super) fn accept_provisional(
        &mut self,
        provisional: Provisional,
        tolerance: Tolerance,
        editor: Option<&Editor>,
    ) {
        let previous = self.request(tolerance, Some(provisional), editor);
        self.revision = self.revision.wrapping_add(1);
        self.worker
            .retain_as(&previous, self.request(tolerance, None, editor));
    }

    fn request(
        &self,
        tolerance: Tolerance,
        provisional: Option<Provisional>,
        editor: Option<&Editor>,
    ) -> Request {
        Request {
            revision: self.revision,
            tolerance,
            provisional,
            seams: editor.map_or_else(Vec::new, |editor| editor.placements().collect()),
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
        slash_point: Option<(usize, Point3)>,
        context: &egui::Context,
    ) {
        let Some(session) = &self.sweep_two_rails else {
            return;
        };
        if session.rail_count() < 2 || session.section_count() == 0 {
            return;
        }
        let provisional = session
            .picking
            .zip(point)
            .map(|(end, point)| Provisional::End(end, point))
            .or_else(|| {
                slash_point
                    .and_then(|pick| session.provisional_slash(pick))
                    .map(Provisional::Slash)
            });
        let mut shape = session.state.shape;
        if let Some(Provisional::End(end, point)) = provisional {
            end.set(&mut shape.ends, point);
        }
        let ready = session.picking.is_none() || provisional.is_some();
        let session = self.sweep_two_rails.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let request = session
            .preview
            .request(tolerance, provisional, session.state.seams.as_ref());
        session.preview.worker.update(
            ready.then_some(request.clone()),
            context,
            |previous, current| {
                previous.revision == current.revision
                    && previous.tolerance == current.tolerance
                    && previous.seams.len() == current.seams.len()
                    && previous.provisional.and_then(Provisional::point_end)
                        == current.provisional.and_then(Provisional::point_end)
            },
            |_| {
                let source = session.state.capture(&self.document);
                let editor = session.state.seams.clone();
                let mut slashes = session.state.slashes.clone();
                if let Some(Provisional::Slash(pair)) = provisional {
                    slashes.push(pair);
                }
                move || {
                    Some(
                        source
                            .and_then(|source| {
                                super::seams::with_seam_profiles(source, editor.as_ref(), tolerance)
                            })
                            .and_then(|source| {
                                if matches!(provisional, Some(Provisional::End(..))) {
                                    source.validate_point_ends(shape.ends, tolerance)?;
                                }
                                source.build(shape, &slashes, tolerance)
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
            && provisional.is_none()
            && session.preview.reported.as_ref() != Some(&request)
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
        let request = session
            .preview
            .request(tolerance, None, session.state.seams.as_ref());
        let result = if let Some(draft) = session.preview.worker.take(&request) {
            draft.map(|draft| draft.brep)
        } else {
            session
                .state
                .capture_edited(&self.document)
                .and_then(|source| {
                    source.build(session.state.shape, &session.state.slashes, tolerance)
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

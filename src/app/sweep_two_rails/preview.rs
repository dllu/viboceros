use super::*;
use viboceros_geometry::{Brep, NurbsCurve};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    revision: u64,
    tolerance: Tolerance,
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

    fn request(&self, tolerance: Tolerance) -> Request {
        Request {
            revision: self.revision,
            tolerance,
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
    pub(in crate::app) fn update_sweep_two_rail_preview(&mut self, context: &egui::Context) {
        if !self
            .sweep_two_rails
            .as_ref()
            .is_some_and(|session| session.section_count() >= 2)
        {
            return;
        }
        if !self.validate_sweep_two_rails() {
            context.request_repaint();
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let request = session.preview.request(tolerance);
        let fit = session.state.fit;
        session.preview.worker.update(
            Some(request),
            context,
            |previous, current| previous == current,
            |_| {
                let ids = session
                    .state
                    .picks
                    .iter()
                    .map(|pick| pick.id)
                    .collect::<Vec<_>>();
                let source = Source::capture(&self.document, &ids, &session.options);
                move || {
                    Some(
                        source
                            .and_then(|source| source.build(fit, tolerance))
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
        let request = session.preview.request(tolerance);
        let result = if let Some(draft) = session.preview.worker.take(&request) {
            draft.map(|draft| draft.brep)
        } else {
            Source::capture(&self.document, &session.ids(), &session.options)
                .and_then(|source| source.build(session.state.fit, tolerance))
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

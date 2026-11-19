//! The temporary wireframe and confirmation share one staged native result.
use super::*;
use std::sync::mpsc;
use viboceros_command::EdgeBlendSelection;
use viboceros_geometry::NurbsCurve;

struct Solved {
    selection: EdgeBlendSelection,
    curves: Vec<NurbsCurve>,
}

type PreviewResult = Result<Solved, String>;

struct Job {
    plan: EdgeBlendPlan,
    result: mpsc::Receiver<PreviewResult>,
}

#[derive(Default)]
pub(in crate::app) struct Preview {
    job: Option<Job>,
    result: Option<(EdgeBlendPlan, PreviewResult)>,
}

impl Preview {
    pub(super) fn clear(&mut self) {
        self.result = None;
    }

    pub(in crate::app) fn curves(&self) -> &[NurbsCurve] {
        self.result
            .as_ref()
            .and_then(|(_, result)| result.as_ref().ok())
            .map_or(&[], |solved| &solved.curves)
    }

    fn update(
        &mut self,
        plan: Option<EdgeBlendPlan>,
        moving: bool,
        context: &egui::Context,
    ) -> Option<String> {
        let compatible = |previous: &EdgeBlendPlan| {
            plan.as_ref().is_some_and(|current| {
                previous == current || moving && previous.matches_except_size(current)
            })
        };
        if self
            .result
            .as_ref()
            .is_some_and(|(previous, _)| !compatible(previous))
        {
            self.clear();
        }
        if let Some(job) = &self.job {
            let result = match job.result.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => Err("edge blend stopped".into()),
            };
            let job = self.job.take().unwrap();
            if compatible(&job.plan) {
                let error = if moving {
                    None
                } else {
                    result.as_ref().err().cloned()
                };
                self.result = Some((job.plan, result));
                if !moving {
                    return error;
                }
            }
        }
        let plan = plan.filter(|current| {
            self.result
                .as_ref()
                .is_none_or(|(previous, _)| previous != current)
        })?;
        let request = plan.clone();
        let context = context.clone();
        let (sender, result) = mpsc::channel();
        match std::thread::Builder::new()
            .name("edge-blend-preview".into())
            .spawn(move || {
                let solved = request
                    .solve()
                    .map_err(|error| error.to_string())
                    .map(|selection| {
                        let curves = selection
                            .replacements()
                            .iter()
                            .filter_map(|(_, geometry)| match geometry {
                                Geometry::Brep(brep) => Some(brep.edges()),
                                _ => None,
                            })
                            .flatten()
                            .map(|edge| edge.curve().clone())
                            .collect();
                        Solved { selection, curves }
                    });
                let _ = sender.send(solved);
                context.request_repaint();
            }) {
            Ok(_) => {
                self.job = Some(Job { plan, result });
                None
            }
            Err(error) => {
                let error = error.to_string();
                self.result = Some((plan, Err(error.clone())));
                Some(error)
            }
        }
    }

    pub(super) fn selection(&mut self, plan: EdgeBlendPlan) -> Result<EdgeBlendSelection, String> {
        if let Some((_, result)) = self
            .result
            .as_ref()
            .filter(|(previous, _)| *previous == plan)
        {
            if let Err(error) = result {
                return Err(error.clone());
            }
            return self.result.take().unwrap().1.map(|solved| solved.selection);
        }
        if self.job.as_ref().is_some_and(|job| job.plan == plan)
            && let Ok(result) = self.job.take().unwrap().result.recv()
        {
            return result.map(|solved| solved.selection);
        }
        plan.solve().map_err(|error| error.to_string())
    }
}

impl VibocerosApp {
    pub(in crate::app) fn update_edge_blend_preview(&mut self, context: &egui::Context) {
        let hover = self.edge_blend_size_hover();
        let plan = self
            .edge_fillet_prompt
            .filter(|prompt| prompt.preview)
            .and_then(|mut prompt| {
                prompt.radius = hover.unwrap_or(prompt.radius);
                self.capture_edge_blend(prompt).ok()
            });
        if let Some(error) = self
            .edge_blend_preview
            .update(plan, hover.is_some(), context)
        {
            self.push_log(format!("Preview unavailable. {error}"));
        }
    }
}

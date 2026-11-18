//! A single background intersection job follows the latest cutting guide.
use super::*;
use crate::viewport::{CutDraft, CutPlan, CutResult};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::BoundingBox3;

pub(super) struct Sources {
    geometry: Vec<GeometrySnapshot>,
    pub(super) bounds: Vec<BoundingBox3>,
    pub(super) extent: BoundingBox3,
}

impl Sources {
    fn matches(&self, other: &Self) -> bool {
        self.geometry.len() == other.geometry.len()
            && self
                .geometry
                .iter()
                .zip(&other.geometry)
                .all(|(a, b)| a.shares_storage_with(b))
    }
}

struct Request {
    plan: CutPlan,
    sources: Sources,
    tolerance: Tolerance,
}

struct Job {
    receiver: mpsc::Receiver<CutResult>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct Preview {
    request: Option<Request>,
    job: Option<Job>,
    result: Option<CutResult>,
}

impl Preview {
    pub(super) fn result(
        &self,
        sources: &Sources,
        tolerance: Tolerance,
    ) -> Option<(&CutPlan, &CutResult)> {
        let request = self.request.as_ref()?;
        if request.tolerance != tolerance || !request.sources.matches(sources) {
            return None;
        }
        Some((&request.plan, self.result.as_ref()?))
    }

    pub(super) fn poll(&mut self) {
        let Some(job) = &self.job else { return };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => CutResult::default(),
        };
        if !job.cancel.load(Ordering::Relaxed) {
            self.result = Some(result);
        }
        self.job = None;
    }

    pub(super) fn update(
        &mut self,
        plan: Option<CutPlan>,
        sources: Option<Sources>,
        tolerance: Tolerance,
        context: &egui::Context,
    ) {
        let request = plan.zip(sources).map(|(plan, sources)| Request {
            plan,
            sources,
            tolerance,
        });
        let changed = match (&self.request, &request) {
            (Some(before), Some(after)) => {
                before.plan != after.plan
                    || before.tolerance != after.tolerance
                    || !before.sources.matches(&after.sources)
            }
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.request = request;
            self.result = None;
            if let Some(job) = &self.job {
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
        if self.job.is_some() || self.result.is_some() {
            return;
        }
        let Some(request) = &self.request else { return };
        let plan = request.plan.clone();
        let sources = request.sources.geometry.clone();
        let tolerance = request.tolerance;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let context = context.clone();
        let (sender, receiver) = mpsc::channel();
        if std::thread::Builder::new()
            .name("cut-preview".into())
            .spawn(move || {
                let result = intersections(&plan, &sources, tolerance, &worker_cancel);
                let _ = sender.send(result);
                context.request_repaint();
            })
            .is_ok()
        {
            self.job = Some(Job { receiver, cancel });
        } else {
            self.result = Some(CutResult::default());
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn intersections(
    plan: &CutPlan,
    sources: &[GeometrySnapshot],
    tolerance: Tolerance,
    cancel: &AtomicBool,
) -> CutResult {
    use viboceros_command::{contour, section};
    let mut result = CutResult::default();
    let mut append = |geometry| match geometry {
        Geometry::Point(point) => result.points.push(point),
        Geometry::Line(line) => result.lines.push([line.start(), line.end()]),
        Geometry::Polyline(polyline) => result
            .lines
            .extend(polyline.vertices().windows(2).map(|p| [p[0], p[1]])),
        curve => {
            if let Ok(Some(curve)) = curve.nurbs_curve_representation() {
                result.curves.push(curve);
            }
        }
    };
    match plan {
        CutPlan::Section {
            frame,
            length,
            extend,
        } => {
            for source in sources {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(parts) =
                    section::section_geometry(source, *frame, *length, *extend, tolerance)
                {
                    parts.into_iter().for_each(&mut append);
                }
            }
        }
        CutPlan::Contour(frames) => {
            'planes: for &frame in frames {
                for source in sources {
                    if cancel.load(Ordering::Relaxed) {
                        break 'planes;
                    }
                    if let Ok(parts) = contour::contour_geometry(source, frame, tolerance) {
                        parts.into_iter().for_each(&mut append);
                    }
                }
            }
        }
    }
    result
}

impl VibocerosApp {
    pub(super) fn cutting_draft(&self) -> Option<CutDraft> {
        if self.plane_prompt.is_some() || self.set_view_prompt.is_some() {
            return None;
        }
        match self.active_command? {
            InteractiveCommand::Section {
                start: Some(start),
                options,
            } => Some(CutDraft::Section {
                start,
                extend: options.extend,
            }),
            InteractiveCommand::Contour {
                base: Some(base),
                direction,
                spacing_start,
                options,
            } => Some(CutDraft::Contour {
                base,
                direction,
                spacing_start,
                spacing: self.commands.contour_preferences().spacing,
                range: options.range,
            }),
            _ => None,
        }
    }

    pub(super) fn cutting_sources(&self, draft: CutDraft) -> Option<Sources> {
        let mut geometry = Vec::new();
        let mut bounds = Vec::new();
        let mut add = |object: &viboceros_document::Object| {
            if let Some(b) = object.geometry().bounds() {
                bounds.push(b);
                geometry.push(object.geometry_snapshot().clone());
            }
        };
        match draft {
            CutDraft::Section { .. } => {
                let ids = self
                    .section_session
                    .as_ref()?
                    .sources
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>();
                for object in self
                    .document
                    .objects()
                    .filter(|object| ids.contains(&object.id()))
                {
                    add(object);
                }
            }
            CutDraft::Contour { .. } => {
                for object in self
                    .document
                    .selected_objects()
                    .filter(|o| viboceros_command::ObjectSelectionFilter::Section.accepts_object(o))
                {
                    add(object);
                }
            }
        }
        let extent = bounds
            .iter()
            .copied()
            .try_fold(*bounds.first()?, BoundingBox3::union)
            .ok()?;
        Some(Sources {
            geometry,
            bounds,
            extent,
        })
    }
}

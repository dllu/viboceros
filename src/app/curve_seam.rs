//! Stage closed-curve seam edits and commit the batch in one undo step.
use super::*;
use crate::viewport::curve_seam::{Drag, Event, Marker, Overlay};
use std::collections::BTreeSet;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::{CommandError, ObjectSelectionFilter, prepare_curve_seam};
use viboceros_geometry::{CurveRef, Real, wrap_closed_curve_parameter};

mod input;

struct Seam {
    source: CapturedCurve,
    knots: Vec<(Real, Point3)>,
    parameter: Real,
    reversed: bool,
}

impl Seam {
    fn capture(document: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let source = CapturedCurve::capture(document, id, None)?;
        let curve = source.curve_ref();
        let domain = curve.domain();
        let knots = match curve {
            CurveRef::NurbsCurve(_) | CurveRef::Ellipse(_) => source
                .curve()
                .knots()
                .chunk_by(|a, b| a == b)
                .filter_map(|knot| {
                    let parameter = curve.parameter_from_nurbs(knot[0]).ok()?;
                    if parameter == *domain.end() || !domain.contains(&parameter) {
                        return None;
                    }
                    Some((parameter, curve.evaluate(parameter).ok()?))
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(Self {
            source,
            knots,
            parameter: *domain.start(),
            reversed: false,
        })
    }

    fn curve(&self) -> CurveRef<'_> {
        self.source.curve_ref()
    }

    fn start(&self) -> Real {
        *self.curve().domain().start()
    }

    fn normalize(&self, parameter: Real) -> Real {
        if parameter == *self.curve().domain().end() {
            self.start()
        } else {
            parameter
        }
    }

    fn marker(&self, index: usize) -> Option<Marker> {
        let curve = self.curve();
        let parameter = wrap_closed_curve_parameter(self.parameter, &curve.domain()).ok()?;
        Marker::at(index, curve, parameter, self.reversed)
    }

    fn natural(&mut self) {
        self.parameter = self.start();
        self.reversed = false;
    }

    fn align_to(&mut self, previous: Marker, snap_to_knots: bool, tolerance: Tolerance) {
        let knot = if snap_to_knots {
            self.knots
                .iter()
                .filter_map(|&(t, point)| Some((t, previous.point.distance_to(point).ok()?)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(t, _)| t)
        } else {
            None
        };
        let Some(parameter) = knot.or_else(|| {
            self.curve()
                .closest_parameter(previous.point, tolerance)
                .ok()
        }) else {
            return;
        };
        // The native getter reports parameters in the current traversal direction.
        let parameter = self.normalize(parameter);
        self.parameter = if self.reversed { -parameter } else { parameter };
        if let Some(tangent) = previous.tangent
            && let Ok(sample) = self.curve().evaluate_with_tangent(parameter)
            && let Ok(dot) = sample.tangent().as_vector().dot(tangent)
        {
            self.reversed = dot < 0.;
        }
    }

    fn output(&self, tolerance: Tolerance) -> Result<Option<(ObjectId, Geometry)>, CommandError> {
        if self.parameter == self.start() && !self.reversed {
            return Ok(None);
        }
        let mut curve = self.curve().to_owned();
        if self.parameter != self.start() {
            curve = prepare_curve_seam(&curve, self.parameter)?;
        }
        if self.reversed {
            curve = curve.reversed(tolerance)?;
        }
        Ok(Some((self.source.object_id(), Geometry::from(curve))))
    }
}

struct Move {
    viewport: usize,
    index: usize,
    pointer: egui::Pos2,
    parameter: Real,
}

pub(super) struct Session {
    before: Vec<ObjectId>,
    seams: Vec<Seam>,
    moving: Option<Move>,
    target: Option<usize>,
    flipping: bool,
    id_pending: bool,
    snap_to_knots: bool,
}

impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.seams.is_empty()
    }

    pub(super) fn hint(&self) -> &'static str {
        match (self.id_pending, self.selecting(), self.flipping) {
            (true, _, _) => "Enter the closed curve object ID",
            (_, true, _) => "Select closed curves for seam adjustment. Press Enter when done",
            (_, _, true) => "Select seam to flip",
            _ if self.snap_to_knots => {
                "Drag seam points. Press Enter when done (Flip Automatic Natural SnapToKnots=Yes)"
            }
            _ => "Drag seam points. Press Enter when done (Flip Automatic Natural SnapToKnots=No)",
        }
    }

    pub(super) fn markers(&self) -> Vec<Marker> {
        self.seams
            .iter()
            .enumerate()
            .filter_map(|(index, seam)| seam.marker(index))
            .collect()
    }

    pub(super) fn overlay<'a>(&'a self, markers: &'a [Marker], viewport: usize) -> Overlay<'a> {
        Overlay {
            markers,
            active: self.moving.as_ref().map(|moving| moving.index),
            flipping: self.flipping,
            drag: self
                .moving
                .as_ref()
                .filter(|moving| moving.viewport == viewport)
                .map(|moving| {
                    let seam = &self.seams[moving.index];
                    Drag {
                        index: moving.index,
                        origin: moving.pointer,
                        origin_parameter: moving.parameter,
                        curve: seam.curve(),
                        display: seam.source.curve(),
                        knots: if self.snap_to_knots { &seam.knots } else { &[] },
                        reversed: seam.reversed,
                    }
                }),
        }
    }

    fn automatic(&mut self, tolerance: Tolerance) {
        let mut previous: Option<Marker> = None;
        for (index, seam) in self.seams.iter_mut().enumerate() {
            if let Some(marker) = previous {
                seam.align_to(marker, self.snap_to_knots, tolerance);
            }
            previous = seam.marker(index);
        }
    }
}

impl VibocerosApp {
    pub(super) fn start_curve_seam(&mut self, input: &str) -> bool {
        if !input
            .trim()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("CrvSeam")
        {
            return false;
        }
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let selected = before
            .iter()
            .copied()
            .filter(|id| self.seam_eligible(*id))
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.restore_curve_selection(selected);
        self.curve_seam_session = Some(Session {
            before,
            seams: Vec::new(),
            moving: None,
            target: None,
            flipping: false,
            id_pending: false,
            snap_to_knots: self.curve_seam_snap_to_knots,
        });
        self.push_log(format!("> {input}"));
        if self.document.selected_object_ids().len() == 0 {
            self.log_curve_seam();
        } else {
            self.begin_curve_seam_adjustment();
        }
        true
    }

    fn seam_eligible(&self, id: ObjectId) -> bool {
        self.document.is_object_selectable(id)
            && self
                .document
                .object(id)
                .is_some_and(|object| ObjectSelectionFilter::ClosedCurves.accepts_object(object))
    }

    pub(super) fn selecting_curve_seams(&self) -> bool {
        self.curve_seam_session
            .as_ref()
            .is_some_and(Session::selecting)
    }

    pub(super) fn select_curve_seams(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) -> bool {
        if self.curve_seam_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if !self.selecting_curve_seams() {
            return true;
        }
        let mut selected = self.document.selected_object_ids().collect::<Vec<_>>();
        let mut membership: BTreeSet<_> = selected.iter().copied().collect();
        for id in ids {
            if membership.contains(&id) {
                if matches!(mode, SelectionMode::Remove | SelectionMode::Toggle) {
                    membership.remove(&id);
                }
            } else if mode != SelectionMode::Remove && self.seam_eligible(id) {
                membership.insert(id);
                selected.push(id);
            }
        }
        selected.retain(|id| membership.remove(id));
        self.restore_curve_selection(selected);
        true
    }

    fn log_curve_seam(&mut self) {
        if let Some(session) = &self.curve_seam_session {
            self.push_log(session.hint().into());
        }
    }

    fn begin_curve_seam_adjustment(&mut self) {
        let seams = self
            .document
            .selected_object_ids()
            .filter(|&id| self.seam_eligible(id))
            .map(|id| Seam::capture(&self.document, id))
            .collect::<Result<Vec<_>, _>>();
        match seams {
            Ok(seams) => {
                let session = self.curve_seam_session.as_mut().expect("CrvSeam session");
                session.target = (seams.len() == 1).then_some(0);
                session.seams = seams;
                session.automatic(self.document.tolerance());
            }
            Err(error) => self.push_log(format!("Unable to read closed curves: {error}")),
        }
        self.log_curve_seam();
    }

    pub(super) fn validate_curve_seams(&mut self) {
        if self.curve_seam_session.as_ref().is_some_and(|session| {
            session
                .seams
                .iter()
                .any(|seam| !seam.source.current(&self.document))
        }) {
            self.cancel_curve_seam(false);
            self.push_log("A CrvSeam source or document tolerance changed".into());
        }
    }

    pub(super) fn cancel_curve_seam(&mut self, announce: bool) {
        if let Some(session) = self.curve_seam_session.take() {
            self.restore_curve_selection(session.before);
            if announce {
                self.push_log("Cancelled CrvSeam".into());
            }
        }
    }

    pub(super) fn apply_curve_seam_event(&mut self, viewport: usize, event: Event) -> bool {
        let Some(session) = &mut self.curve_seam_session else {
            return false;
        };
        match event {
            Event::Grab(index, pointer) => {
                session.target = Some(index);
                session.moving = Some(Move {
                    viewport,
                    index,
                    pointer,
                    parameter: session.seams[index].parameter,
                });
                true
            }
            Event::Move { parameter, done } => {
                let mut changed = done;
                if let Some(moving) = &session.moving
                    && let Some(parameter) = parameter
                {
                    let seam = &mut session.seams[moving.index];
                    changed |= seam.parameter != parameter;
                    seam.parameter = parameter;
                }
                if done {
                    session.moving = None;
                }
                changed
            }
            Event::Flip(index) => {
                session.seams[index].reversed ^= true;
                session.flipping = false;
                session.target = Some(index);
                self.log_curve_seam();
                true
            }
        }
    }

    fn finish_curve_seam(&mut self) {
        let session = self.curve_seam_session.take().expect("CrvSeam session");
        self.restore_curve_selection(session.before);
        let output = session
            .seams
            .iter()
            .filter_map(|seam| seam.output(self.document.tolerance()).transpose())
            .collect::<Result<Vec<_>, _>>();
        let result = output.and_then(|output| {
            let count = output.len();
            if count > 0 {
                edit_document_transaction(&mut self.document, "CrvSeam", |document| {
                    document.replace_object_geometries(output)?;
                    Ok(())
                })?;
            }
            Ok(count)
        });
        self.push_log(match result {
            Ok(0) => "Curve seams are unchanged".into(),
            Ok(1) => "Adjusted 1 curve seam".into(),
            Ok(count) => format!("Adjusted {count} curve seams"),
            Err(error) => format!("Unable to adjust curve seams: {error}"),
        });
    }
}

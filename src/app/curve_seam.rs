//! Stage closed-curve seam edits and commit the batch in one undo step.
use super::*;
use crate::viewport::curve_seam::{Drag, Event, Marker, Overlay};
use std::collections::BTreeSet;
use std::sync::Arc;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::{CommandError, ObjectSelectionFilter, prepare_curve_seam};
use viboceros_geometry::{Curve3, CurveRef, NurbsCurve, Real, wrap_closed_curve_parameter};

mod input;

#[derive(Clone, Debug)]
struct Seam {
    source: CapturedCurve,
    knots: Arc<[(Real, Point3)]>,
    parameter: Real,
    reversed: bool,
}

impl Seam {
    fn from_source(source: CapturedCurve) -> Self {
        let curve = source.curve_ref();
        let domain = curve.domain();
        let knots: Vec<_> = match curve {
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
        Self {
            source,
            knots: knots.into(),
            parameter: *domain.start(),
            reversed: false,
        }
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

    fn relocated(&self, tolerance: Tolerance) -> Result<Option<Curve3>, CommandError> {
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
        Ok(Some(curve))
    }

    fn output(&self, tolerance: Tolerance) -> Result<Option<(ObjectId, Geometry)>, CommandError> {
        Ok(self
            .relocated(tolerance)?
            .map(|curve| (self.source.object_id(), Geometry::from(curve))))
    }
}

#[derive(Clone, Debug)]
struct Move {
    viewport: usize,
    index: usize,
    pointer: egui::Pos2,
    parameter: Real,
}

#[derive(Clone, Debug)]
pub(super) struct Editor {
    seams: Vec<Seam>,
    moving: Option<Move>,
    target: Option<usize>,
    flipping: bool,
    snap_to_knots: bool,
}

impl Editor {
    fn empty(snap_to_knots: bool) -> Self {
        Self {
            seams: Vec::new(),
            moving: None,
            target: None,
            flipping: false,
            snap_to_knots,
        }
    }

    pub(super) fn capture(
        document: &Document,
        ids: impl IntoIterator<Item = ObjectId>,
        snap_to_knots: bool,
        natural: bool,
    ) -> Result<Self, CommandError> {
        let sources = ids
            .into_iter()
            .map(|id| CapturedCurve::capture(document, id, None))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_sources(
            sources,
            document.tolerance(),
            snap_to_knots,
            natural,
        ))
    }

    pub(super) fn from_sources(
        sources: impl IntoIterator<Item = CapturedCurve>,
        tolerance: Tolerance,
        snap_to_knots: bool,
        natural: bool,
    ) -> Self {
        let seams = sources
            .into_iter()
            .map(Seam::from_source)
            .collect::<Vec<_>>();
        let target = (seams.len() == 1).then_some(0);
        let mut editor = Self {
            seams,
            target,
            ..Self::empty(snap_to_knots)
        };
        if !natural {
            editor.automatic(tolerance);
        }
        editor
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.flipping {
            "Select seam to flip"
        } else if self.snap_to_knots {
            "Drag seam points. Press Enter when done (Flip Automatic Natural SnapToKnots=Yes)"
        } else {
            "Drag seam points. Press Enter when done (Flip Automatic Natural SnapToKnots=No)"
        }
    }

    pub(super) fn snap_to_knots(&self) -> bool {
        self.snap_to_knots
    }

    pub(super) fn sources(&self) -> impl Iterator<Item = &CapturedCurve> {
        self.seams.iter().map(|seam| &seam.source)
    }

    pub(super) fn placements(&self) -> impl Iterator<Item = (Real, bool)> + '_ {
        self.seams
            .iter()
            .map(|seam| (seam.parameter, seam.reversed))
    }

    pub(super) fn profiles(&self, tolerance: Tolerance) -> Result<Vec<NurbsCurve>, CommandError> {
        self.seams
            .iter()
            .map(|seam| {
                Ok(match seam.relocated(tolerance)? {
                    Some(curve) => curve.as_ref().to_nurbs()?,
                    None => seam.source.curve().clone(),
                })
            })
            .collect()
    }

    pub(super) fn option(
        &mut self,
        command: &str,
        tolerance: Tolerance,
    ) -> Option<Result<(), &'static str>> {
        if self.flipping {
            return None;
        }
        let (option, value) = command
            .split_once(|c: char| c.is_whitespace() || c == '=')
            .map_or((command, ""), |(name, value)| (name, value.trim()));
        if option.eq_ignore_ascii_case("Flip") && value.is_empty() {
            self.flipping = true;
        } else if option.eq_ignore_ascii_case("Natural") && value.is_empty() {
            for seam in &mut self.seams {
                seam.natural();
            }
        } else if option.eq_ignore_ascii_case("Automatic") && value.is_empty() {
            self.automatic(tolerance);
        } else if option.eq_ignore_ascii_case("SnapToKnots") {
            self.snap_to_knots = match value.trim_start_matches('_') {
                "" => !self.snap_to_knots,
                yes if yes.eq_ignore_ascii_case("Yes") => true,
                no if no.eq_ignore_ascii_case("No") => false,
                _ => return Some(Err("Use SnapToKnots=Yes or SnapToKnots=No")),
            };
        } else {
            return None;
        }
        self.moving = None;
        Some(Ok(()))
    }

    pub(super) fn select_id(&mut self, viewport: usize, id: ObjectId) -> bool {
        self.select_source(viewport, (id, None))
    }

    pub(super) fn select_source(
        &mut self,
        viewport: usize,
        source: (ObjectId, Option<usize>),
    ) -> bool {
        let Some(index) = self
            .seams
            .iter()
            .position(|seam| (seam.source.object_id(), seam.source.source().1) == source)
        else {
            return false;
        };
        if self.flipping {
            self.apply(viewport, Event::Flip(index));
        } else {
            self.target = Some(index);
        }
        true
    }

    pub(super) fn relocate_target(
        &mut self,
        parameter: Option<Real>,
        point: Option<Point3>,
        tolerance: Tolerance,
    ) -> bool {
        if self.flipping {
            return false;
        }
        let Some(seam) = self.target.and_then(|index| self.seams.get_mut(index)) else {
            return false;
        };
        let parameter =
            parameter.or_else(|| seam.curve().closest_parameter(point?, tolerance).ok());
        let Some(parameter) = parameter else {
            return false;
        };
        seam.parameter = parameter;
        self.moving = None;
        true
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
            connect: true,
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
    pub(super) fn apply(&mut self, viewport: usize, event: Event) -> bool {
        match event {
            Event::Grab(index, pointer) => {
                self.target = Some(index);
                self.moving = Some(Move {
                    viewport,
                    index,
                    pointer,
                    parameter: self.seams[index].parameter,
                });
                true
            }
            Event::Move { parameter, done } => {
                let mut changed = done;
                if let Some(moving) = &self.moving
                    && let Some(parameter) = parameter
                {
                    let seam = &mut self.seams[moving.index];
                    changed |= seam.parameter != parameter;
                    seam.parameter = parameter;
                }
                if done {
                    self.moving = None;
                }
                changed
            }
            Event::Flip(index) => {
                self.seams[index].reversed ^= true;
                self.flipping = false;
                self.target = Some(index);
                true
            }
        }
    }
}

pub(super) struct Session {
    before: Vec<ObjectId>,
    id_pending: bool,
    pub(super) editor: Editor,
}

impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.editor.seams.is_empty()
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.id_pending {
            "Enter the closed curve object ID"
        } else if self.selecting() {
            "Select closed curves for seam adjustment. Press Enter when done"
        } else {
            self.editor.hint()
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
            id_pending: false,
            editor: Editor::empty(self.curve_seam_snap_to_knots),
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
        let ids = self
            .document
            .selected_object_ids()
            .filter(|&id| self.seam_eligible(id));
        match Editor::capture(&self.document, ids, self.curve_seam_snap_to_knots, false) {
            Ok(editor) => {
                self.curve_seam_session
                    .as_mut()
                    .expect("CrvSeam session")
                    .editor = editor;
            }
            Err(error) => self.push_log(format!("Unable to read closed curves: {error}")),
        }
        self.log_curve_seam();
    }

    pub(super) fn validate_curve_seams(&mut self) {
        if self.curve_seam_session.as_ref().is_some_and(|session| {
            session
                .editor
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
            return self.apply_loft_seam_event(viewport, event);
        };
        let changed = session.editor.apply(viewport, event);
        if matches!(event, Event::Flip(_)) {
            self.log_curve_seam();
        }
        changed
    }

    fn finish_curve_seam(&mut self) {
        let session = self.curve_seam_session.take().expect("CrvSeam session");
        self.restore_curve_selection(session.before);
        let output = session
            .editor
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

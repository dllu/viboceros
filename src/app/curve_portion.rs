//! Shared transient SubCrv getter for blend commands; model history stays untouched.
use super::*;
use viboceros_command::{
    curve_reference::{CapturedCurve, PortionBinding},
    subcurve_input,
};
mod input;
#[derive(Clone, Default)]
pub(in crate::app) struct Editor {
    pub(in crate::app) reference: Option<CapturedCurve>,
    start: Option<f64>,
    length: Option<f64>,
    hover: Option<f64>,
    locked: Option<bool>,
    preview: super::subcurve_preview::Cache,
}
pub(in crate::app) enum Action {
    Pending,
    Discard,
    Capture(ObjectId, Option<usize>),
    Selected(PortionBinding),
}
impl Editor {
    pub(in crate::app) fn hint(&self) -> &'static str {
        if self.reference.is_none() {
            "Select curve to temporarily shorten"
        } else if self.start.is_none() {
            "Start of temporary curve"
        } else if self.locked.is_some() {
            "Select locked-side end or enter curve length; Direction=Free unlocks"
        } else if self.length.is_some() {
            "Confirm the length direction on the curve"
        } else {
            "Select end location or enter curve length; Direction=Free"
        }
    }
    pub(in crate::app) fn capture(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> Result<(), String> {
        let reference = CapturedCurve::capture(doc, id, edge).map_err(|e| e.to_string())?;
        self.hover = pick.and_then(|p| {
            reference
                .curve_ref()
                .closest_parameter(p, doc.tolerance())
                .ok()
        });
        self.reference = Some(reference);
        Ok(())
    }
    pub(in crate::app) fn hover(&self) -> Option<f64> {
        self.hover
    }
    pub(in crate::app) fn hover_parameter(&mut self, t: f64) {
        if let Some(source) = &self.reference {
            self.hover = source.curve_ref().parameter_from_nurbs(t).ok();
        }
    }
    pub(in crate::app) fn accept_parameter(
        &mut self,
        t: f64,
        tolerance: Tolerance,
    ) -> Result<Action, String> {
        let source = self
            .reference
            .as_ref()
            .ok_or("Select curve to temporarily shorten")?;
        let parameter = source
            .curve_ref()
            .parameter_from_nurbs(t)
            .map_err(|e| e.to_string())?;
        self.advance(parameter, tolerance)
    }
    fn advance(&mut self, t: f64, tolerance: Tolerance) -> Result<Action, String> {
        let Some(start) = self.start else {
            self.start = Some(t);
            return Ok(Action::Pending);
        };
        if self.length.is_none() && start == t {
            return Ok(Action::Discard);
        }
        let source = self.reference.as_ref().unwrap();
        let c = source.curve_ref();
        let range = if let Some(length) = self.length {
            if let Some(forward) = self.locked {
                subcurve_input::interval_in_direction(c, start, length, forward, tolerance)
            } else {
                subcurve_input::interval(c, start, t, length, tolerance)
            }
        } else {
            subcurve_input::point_parameters(c, start, t, self.locked, false).map_err(Into::into)
        }
        .map_err(|e| e.to_string())?;
        Ok(range.map_or(Action::Discard, |parameters| {
            Action::Selected(PortionBinding {
                object: source.object_id(),
                edge: source.source().1,
                parameters,
                wrap: self.length.is_some(),
            })
        }))
    }
    pub(in crate::app) fn preview(
        &mut self,
        doc: &Document,
    ) -> Option<super::subcurve_preview::Preview> {
        let source = self.reference.as_ref()?;
        if !source.current(doc) {
            return None;
        }
        self.preview.temporary_bounded(
            Some(source.source().0),
            source.source().1,
            [self.start?, self.hover?],
            self.length,
            self.locked,
            doc.tolerance(),
        )
    }
}

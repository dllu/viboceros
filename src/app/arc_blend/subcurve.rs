//! Ephemeral curve portions retain their original parent and shared preview policy.
use super::*;
use viboceros_command::{curve_reference::CapturedCurve, subcurve_input};
mod input;
#[derive(Clone, Default)]
pub(in crate::app) struct Editor {
    pub(super) reference: Option<CapturedCurve>,
    start: Option<f64>,
    length: Option<f64>,
    hover: Option<f64>,
    locked: Option<bool>,
    preview: crate::app::subcurve_preview::Cache,
}
impl Editor {
    pub(super) fn hint(&self) -> &'static str {
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
}
impl Session {
    pub(in crate::app) fn subcurve_hover(&self) -> Option<f64> {
        self.subcurve.as_ref()?.hover
    }
    pub(in crate::app) fn subcurve_preview(
        &mut self,
        doc: &Document,
    ) -> Option<crate::app::subcurve_preview::Preview> {
        let p = self.subcurve.as_mut()?;
        let source = p.reference.as_ref()?;
        if !source.current(doc) {
            return None;
        }
        p.preview.temporary_bounded(
            Some(source.source().0),
            source.source().1,
            [p.start?, p.hover?],
            p.length,
            p.locked,
            doc.tolerance(),
        )
    }
}
impl VibocerosApp {
    pub(in crate::app) fn discard_arc_subcurve(&mut self) -> bool {
        let Some(s) = self.arc_blend_session.as_mut() else {
            return false;
        };
        if s.subcurve.take().is_none() {
            return false;
        }
        s.binding = None;
        s.candidate = None;
        self.command_input.clear();
        self.arc_blend_prompt();
        true
    }
    pub(super) fn pick_arc_subcurve_reference(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> bool {
        let Some(p) = self
            .arc_blend_session
            .as_ref()
            .and_then(|s| s.subcurve.as_ref())
        else {
            return false;
        };
        if p.reference.is_some() {
            return true;
        }
        match CapturedCurve::capture(&self.document, id, edge) {
            Ok(reference) => {
                let hover = pick.and_then(|p| {
                    reference
                        .curve_ref()
                        .closest_parameter(p, self.document.tolerance())
                        .ok()
                });
                let p = self
                    .arc_blend_session
                    .as_mut()
                    .unwrap()
                    .subcurve
                    .as_mut()
                    .unwrap();
                p.reference = Some(reference);
                p.hover = hover;
                self.arc_blend_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn hover_arc_subcurve_parameter(&mut self, t: f64) -> bool {
        let Some(p) = self
            .arc_blend_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
        else {
            return false;
        };
        if let Some(source) = &p.reference {
            p.hover = source.curve_ref().parameter_from_nurbs(t).ok();
        }
        true
    }
    pub(super) fn accept_arc_subcurve_parameter(&mut self, t: f64) -> bool {
        self.validate_arc_blend();
        let Some(source) = self
            .arc_blend_session
            .as_ref()
            .and_then(|s| s.subcurve.as_ref())
            .and_then(|p| p.reference.as_ref())
        else {
            return false;
        };
        let result = source.curve_ref().parameter_from_nurbs(t);
        match result {
            Ok(t) => self.accept_arc_subcurve_native_parameter(t),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    fn accept_arc_subcurve_native_parameter(&mut self, t: f64) {
        let s = self.arc_blend_session.as_mut().unwrap();
        let p = s.subcurve.as_mut().unwrap();
        let Some(start) = p.start else {
            p.start = Some(t);
            self.arc_blend_prompt();
            return;
        };
        let source = p.reference.as_ref().unwrap();
        let c = source.curve_ref();
        if p.length.is_none() && start == t {
            self.discard_arc_subcurve();
            return;
        }
        let range = if let Some(length) = p.length {
            if let Some(forward) = p.locked {
                subcurve_input::interval_in_direction(
                    c,
                    start,
                    length,
                    forward,
                    self.document.tolerance(),
                )
            } else {
                subcurve_input::interval(c, start, t, length, self.document.tolerance())
            }
        } else {
            subcurve_input::point_parameters(c, start, t, p.locked, false).map_err(Into::into)
        };
        let result = range.and_then(|range| {
            if let Some(range) = range {
                s.input.pick_subcurve(
                    &self.document,
                    source.object_id(),
                    source.source().1,
                    range,
                    p.length.is_some(),
                )?;
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                self.discard_arc_subcurve();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
    }
}

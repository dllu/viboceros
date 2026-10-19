//! Nonmutating temporary ranges use the shared length, direction and preview policies.
use super::*;
use viboceros_command::subcurve_input;
impl Prompt {
    fn empty_locked_side(&self) -> bool {
        self.input
            .start
            .zip(self.input.locked_forward)
            .is_some_and(|(start, forward)| {
                let curve = self.draft.as_ref().unwrap().geometry().curve_ref().unwrap();
                let domain = curve.domain();
                curve.is_closed().unwrap_or(false)
                    && start
                        == if forward {
                            *domain.end()
                        } else {
                            *domain.start()
                        }
            })
    }
    pub(in crate::app) fn subcurve_hover(&self) -> Option<f64> {
        self.shortening
            .then_some(self.input.hover_parameter)
            .flatten()
    }
    pub(in crate::app) fn subcurve_preview(
        &mut self,
        doc: &Document,
    ) -> Option<super::super::subcurve_preview::Preview> {
        if !self.shortening || self.empty_locked_side() || self.full_closed_length {
            return None;
        }
        let source = doc
            .object(self.draft.as_ref()?.object())?
            .geometry_snapshot();
        self.input.preview.temporary(
            Some(source),
            None,
            [self.input.start?, self.input.hover_parameter?],
            self.input.length,
            self.input.locked_forward,
            doc.tolerance(),
        )
    }
}
impl VibocerosApp {
    pub(super) fn discard_insert_kink_subcurve(&mut self) {
        let p = self.insert_kink_prompt.as_mut().unwrap();
        p.draft = None;
        p.cursor = None;
        p.shortening = false;
        p.subcurve = false;
        p.input = Default::default();
        p.locked_side = None;
        p.full_closed_length = false;
        self.point_filter = None;
        self.point_constraint = None;
        self.snaps.model_override = None;
        self.command_input.clear();
        self.log_insert_kink();
    }
    pub(in crate::app) fn update_insert_kink_subcurve_hover(&mut self, point: Point3) -> bool {
        let Some(p) = self
            .insert_kink_prompt
            .as_mut()
            .filter(|p| p.shortening && p.input.start.is_some())
        else {
            return false;
        };
        if p.empty_locked_side() {
            p.input.hover_parameter = p.input.start;
            return true;
        }
        let source = p.locked_side.as_ref().or_else(|| {
            p.draft
                .as_ref()
                .and_then(|d| self.document.object(d.object()))
                .map(|o| o.geometry_snapshot())
        });
        p.input.hover_parameter =
            p.input
                .preview
                .parameter(source, None, point, self.document.tolerance());
        true
    }
    pub(super) fn continue_insert_kink_subcurve(&mut self, text: &str) -> bool {
        let Some(p) = self.insert_kink_prompt.as_mut().filter(|p| p.shortening) else {
            return false;
        };
        if text.is_empty() {
            self.discard_insert_kink_subcurve();
            return true;
        }
        let Some(start) = p.input.start else {
            return false;
        };
        let curve = p.draft.as_ref().unwrap().geometry().curve_ref().unwrap();
        if let Some(answer) = subcurve_input::direction_option(text) {
            match answer {
                Ok(false) => {
                    p.input.locked_forward = None;
                    p.locked_side = None;
                }
                Ok(true)
                    if text.trim_start_matches('_').eq_ignore_ascii_case("D")
                        && p.input.locked_forward.is_some() =>
                {
                    p.input.locked_forward = None;
                    p.locked_side = None;
                }
                Ok(true) => {
                    let Some(hover) = p.input.hover_parameter else {
                        self.push_log("Move along the curve before locking direction".into());
                        return true;
                    };
                    match subcurve_input::cursor_forward(curve, start, hover) {
                        Ok(forward) => {
                            let domain = curve.domain();
                            let end = if forward {
                                *domain.end()
                            } else {
                                *domain.start()
                            };
                            let side = if curve.is_closed().unwrap_or(false) && start != end {
                                let range = if forward { [start, end] } else { [end, start] };
                                match viboceros_command::prepare_directed_subcurve(
                                    &curve.to_owned(),
                                    range[0],
                                    range[1],
                                ) {
                                    Ok(c) => Some(Geometry::from(c).into()),
                                    Err(e) => {
                                        self.push_log(format!("Error: {e}"));
                                        return true;
                                    }
                                }
                            } else {
                                None
                            };
                            p.locked_side = side;
                            p.input.locked_forward = Some(forward);
                        }
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            return true;
                        }
                    }
                }
                Err(e) => {
                    self.push_log(e.into());
                    return true;
                }
            }
            self.log_insert_kink();
            return true;
        }
        let Some(quantity) =
            viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
        else {
            return false;
        };
        let length = quantity.map_err(|e| e.to_string()).and_then(|length| {
            let total = curve
                .length(self.document.tolerance())
                .map_err(|e| e.to_string())?;
            // Rhino's reported whole length has a default fractional accuracy
            // of 1e-8. Admit an upper endpoint within that reporting precision;
            // the exact extractor still clamps to the source's valid domain.
            if length.abs() > total && length.abs() - total > total * 1e-8 {
                Err("Curve length exceeds the whole source curve".into())
            } else {
                Ok((
                    length.abs().min(total),
                    length.abs() >= total && curve.is_closed().map_err(|e| e.to_string())?,
                ))
            }
        });
        match length {
            Ok((0., _)) => self.discard_insert_kink_subcurve(),
            Ok((length, full_closed)) => {
                p.input.length = Some(length);
                p.full_closed_length = full_closed;
                if let Some(forward) = p.input.locked_forward {
                    match subcurve_input::locked_numeric_requires_confirmation(
                        curve,
                        start,
                        length,
                        forward,
                        self.document.tolerance(),
                    ) {
                        Ok(true) => {
                            p.input.locked_forward = None;
                            p.locked_side = None;
                            self.log_insert_kink();
                        }
                        Ok(false) => match curve.evaluate(start) {
                            Ok(point) => {
                                self.insert_kink_subcurve_point(point);
                            }
                            Err(e) => self.push_log(format!("Error: {e}")),
                        },
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                } else {
                    self.log_insert_kink();
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn insert_kink_subcurve_point(&mut self, point: Point3) -> bool {
        if self
            .insert_kink_prompt
            .as_ref()
            .unwrap()
            .empty_locked_side()
        {
            self.discard_insert_kink_subcurve();
            return true;
        }
        let p = self.insert_kink_prompt.as_mut().unwrap();
        let d = p.draft.as_ref().unwrap();
        let curve = d.geometry().curve_ref().unwrap();
        // A closed locked getter projects onto the captured side of the
        // original parameter chart. It does not wrap an opposite-side pick.
        let pick_curve = p
            .locked_side
            .as_ref()
            .and_then(|g| g.curve_ref())
            .unwrap_or(curve);
        let parameter = match pick_curve.closest_parameter(point, self.document.tolerance()) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let Some(start) = p.input.start else {
            p.input.start = Some(parameter);
            self.log_insert_kink();
            return true;
        };
        let interval = if let Some(length) = p.input.length {
            if let Some(forward) = p.input.locked_forward {
                subcurve_input::interval_in_direction(
                    curve,
                    start,
                    length,
                    forward,
                    self.document.tolerance(),
                )
            } else {
                subcurve_input::interval(curve, start, parameter, length, self.document.tolerance())
            }
        } else {
            subcurve_input::point_parameters(curve, start, parameter, p.input.locked_forward, false)
                .map_err(Into::into)
        };
        let range = match interval {
            Ok(Some(range)) => range,
            Ok(None) => {
                self.discard_insert_kink_subcurve();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        match Draft::new(&self.document, d.object(), Some(range))
            .and_then(|draft| Ok((draft.geometry().curve_ref().unwrap().to_nurbs()?, draft)))
        {
            Ok((cursor, draft)) => {
                let p = self.insert_kink_prompt.as_mut().unwrap();
                p.cursor = Some(cursor);
                p.draft = Some(draft);
                p.shortening = false;
                p.input = Default::default();
                p.locked_side = None;
                p.full_closed_length = false;
                self.log_insert_kink();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
}

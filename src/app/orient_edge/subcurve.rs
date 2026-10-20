//! Temporary source portions reuse the directed SubCrv solver and preview cache.
use super::*;
use viboceros_command::subcurve_input;

impl Session {
    pub(in crate::app) fn subcurve_hover(&self) -> Option<f64> {
        self.subcurve.as_ref()?.hover_parameter
    }
    pub(in crate::app) fn subcurve_preview(
        &mut self,
        doc: &Document,
    ) -> Option<super::super::subcurve_preview::Preview> {
        let p = self.subcurve.as_mut()?;
        let (source, edge) = self.subcurve_source.as_ref()?.source();
        p.preview.temporary(
            Some(source),
            edge,
            [p.start?, p.hover_parameter?],
            p.length,
            p.locked_forward,
            doc.tolerance(),
        )
    }
}

impl VibocerosApp {
    fn discard_edge_subcurve(&mut self) {
        let s = self.orient_edge_session.as_mut().unwrap();
        s.subcurve = None;
        s.subcurve_source = None;
        s.sources.clear();
        s.cursor = None;
        s.pick = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.orient_edge_prompt();
    }

    pub(in crate::app) fn continue_edge_subcurve(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_edge_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("SubCrv") && s.source.is_none() && s.subcurve.is_none() {
            self.orient_edge_session.as_mut().unwrap().subcurve = Some(Default::default());
            self.orient_edge_prompt();
            return true;
        }
        let Some(p) = &s.subcurve else { return false };
        if input.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("Cancel")
        {
            self.discard_edge_subcurve();
            return true;
        }
        if p.start.is_none() {
            return false;
        }
        let Some(quantity) =
            viboceros_drafting::PointInput::parse_length_with_units(input, self.document.units())
        else {
            return false;
        };
        let result = quantity.map_err(|e| e.to_string()).and_then(|length| {
            let total = s
                .subcurve_source
                .as_ref()
                .unwrap()
                .curve_ref()
                .length(self.document.tolerance())
                .map_err(|e| e.to_string())?;
            if length.abs() > total {
                return Err("Curve length exceeds the whole source curve".into());
            }
            Ok(length.abs())
        });
        match result {
            Ok(0.) => self.discard_edge_subcurve(),
            Ok(length) => {
                let s = self.orient_edge_session.as_mut().unwrap();
                let p = s.subcurve.as_mut().unwrap();
                p.length = Some(length);
                if let Some(forward) = p.locked_forward {
                    let c = s.subcurve_source.as_ref().unwrap().curve_ref();
                    match subcurve_input::locked_numeric_requires_confirmation(
                        c,
                        p.start.unwrap(),
                        length,
                        forward,
                        self.document.tolerance(),
                    ) {
                        Ok(false) => {
                            if let Ok(point) = c.evaluate(p.start.unwrap()) {
                                self.accept_edge_subcurve_point(point);
                            }
                        }
                        Ok(true) => {
                            p.locked_forward = None;
                            self.orient_edge_prompt();
                        }
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                } else {
                    self.orient_edge_prompt();
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }

    pub(super) fn accept_edge_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.orient_edge_session.as_ref()?;
        let captured = s.subcurve_source.as_ref()?;
        let c = captured.curve_ref();
        let p = s.subcurve.as_ref()?;
        let parameter = match c.closest_parameter(point, self.document.tolerance()) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        };
        let Some(start) = p.start else {
            self.orient_edge_session
                .as_mut()
                .unwrap()
                .subcurve
                .as_mut()
                .unwrap()
                .start = Some(parameter);
            self.orient_edge_prompt();
            return Some(true);
        };
        if p.length.is_none() && start == parameter {
            self.discard_edge_subcurve();
            return Some(true);
        }
        let interval = if let Some(length) = p.length {
            if let Some(forward) = p.locked_forward {
                subcurve_input::interval_in_direction(
                    c,
                    start,
                    length,
                    forward,
                    self.document.tolerance(),
                )
            } else {
                subcurve_input::interval(c, start, parameter, length, self.document.tolerance())
            }
        } else {
            subcurve_input::point_parameters(c, start, parameter, p.locked_forward, false)
                .map_err(Into::into)
        };
        let range = match interval {
            Ok(Some(range)) => range,
            Ok(None) => {
                self.discard_edge_subcurve();
                return Some(true);
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        };
        match Source::capture_range(&self.document, captured.object_id(), s.pick, Some(range)) {
            Ok(source) => {
                let s = self.orient_edge_session.as_mut().unwrap();
                s.source = Some(source);
                s.range = Some(range);
                s.subcurve = None;
                s.subcurve_source = None;
                s.cursor = None;
                self.point_filter = None;
                self.point_constraint = None;
                self.orient_edge_prompt();
                Some(true)
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                Some(false)
            }
        }
    }

    pub(in crate::app) fn update_edge_subcurve_hover(&mut self, point: Point3) -> bool {
        let Some(s) = self.orient_edge_session.as_mut() else {
            return false;
        };
        let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) else {
            return false;
        };
        let source = s.subcurve_source.as_ref().map(|c| c.source().0);
        p.hover_parameter = p
            .preview
            .parameter(source, None, point, self.document.tolerance());
        true
    }

    pub(in crate::app) fn continue_edge_subcurve_direction(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_edge_session.as_mut() else {
            return false;
        };
        let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) else {
            return false;
        };
        let Some(locked) = subcurve_input::direction_option(input) else {
            return false;
        };
        match locked {
            Ok(false) => p.locked_forward = None,
            Ok(true) => {
                let Some(hover) = p.hover_parameter else {
                    self.push_log("Move along the source curve before locking direction".into());
                    return true;
                };
                match subcurve_input::cursor_forward(
                    s.subcurve_source.as_ref().unwrap().curve_ref(),
                    p.start.unwrap(),
                    hover,
                ) {
                    Ok(forward) => p.locked_forward = Some(forward),
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
        self.orient_edge_prompt();
        true
    }
}

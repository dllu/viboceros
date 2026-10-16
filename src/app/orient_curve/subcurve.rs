//! A temporary guide uses the common directed trimming, length, and preview policies.
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
        let source = doc.object(p.object?)?.geometry_snapshot();
        p.preview.temporary(
            Some(source),
            [p.start?, p.hover_parameter?],
            p.length,
            p.locked_forward,
            doc.tolerance(),
        )
    }
}

impl VibocerosApp {
    fn discard_orient_subcurve(&mut self) {
        let s = self.orient_curve_session.as_mut().unwrap();
        s.subcurve = None;
        s.guide = None;
        s.phase = Phase::Guide;
        self.point_filter = None;
        self.point_constraint = None;
        self.orient_curve_prompt();
    }

    pub(super) fn continue_orient_subcurve(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_curve_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("SubCrv") && s.phase == Phase::Guide {
            self.orient_curve_session.as_mut().unwrap().subcurve = Some(Default::default());
            self.orient_curve_prompt();
            return true;
        }
        if s.subcurve.is_none() {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.discard_orient_subcurve();
            return true;
        }
        if s.phase != Phase::SubcurveEnd {
            return false;
        }
        let Some(quantity) =
            viboceros_drafting::PointInput::parse_length_with_units(input, self.document.units())
        else {
            return false;
        };
        let result = quantity.map_err(|e| e.to_string()).and_then(|length| {
            let curve = self
                .orient_curve_session
                .as_ref()
                .unwrap()
                .guide
                .as_ref()
                .unwrap()
                .original
                .as_ref();
            let total = curve
                .length(self.document.tolerance())
                .map_err(|e| e.to_string())?;
            if length.abs() > total {
                return Err("Curve length exceeds the whole source curve".into());
            }
            Ok(length.abs())
        });
        match result {
            Ok(0.) => self.discard_orient_subcurve(),
            Ok(length) => {
                let s = self.orient_curve_session.as_mut().unwrap();
                let p = s.subcurve.as_mut().unwrap();
                p.length = Some(length);
                if let Some(forward) = p.locked_forward {
                    match subcurve_input::locked_numeric_requires_confirmation(
                        s.guide.as_ref().unwrap().original.as_ref(),
                        p.start.unwrap(),
                        length,
                        forward,
                        self.document.tolerance(),
                    ) {
                        Ok(false) => {
                            let point = s
                                .guide
                                .as_ref()
                                .unwrap()
                                .original
                                .as_ref()
                                .evaluate(p.start.unwrap());
                            if let Ok(point) = point {
                                self.accept_orient_subcurve_point(point);
                            }
                        }
                        Ok(true) => {
                            p.locked_forward = None;
                            self.orient_curve_prompt();
                        }
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                } else {
                    self.orient_curve_prompt();
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }

    pub(super) fn accept_orient_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.orient_curve_session.as_ref()?;
        if !matches!(s.phase, Phase::SubcurveStart | Phase::SubcurveEnd) {
            return None;
        }
        let guide = s.guide.as_ref().unwrap();
        if self
            .document
            .object(guide.id)
            .and_then(|o| o.geometry().curve_ref())
            .is_none_or(|c| c.to_owned() != guide.original)
        {
            self.push_log("Error: orientation curve changed; restart OrientOnCrv".into());
            return Some(false);
        }
        let parameter = match guide
            .original
            .as_ref()
            .closest_parameter(point, self.document.tolerance())
        {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        };
        if s.phase == Phase::SubcurveStart {
            let s = self.orient_curve_session.as_mut().unwrap();
            s.subcurve.as_mut().unwrap().start = Some(parameter);
            s.phase = Phase::SubcurveEnd;
            self.orient_curve_prompt();
            return Some(true);
        }
        let p = s.subcurve.as_ref().unwrap();
        let curve = guide.original.as_ref();
        let start = p.start.unwrap();
        let interval = if let Some(length) = p.length {
            if let Some(forward) = p.locked_forward {
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
            subcurve_input::point_parameters(curve, start, parameter, p.locked_forward, false)
                .map_err(Into::into)
        };
        let range = match interval {
            Ok(Some(range)) => range,
            Ok(None) => {
                self.discard_orient_subcurve();
                return Some(true);
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        };
        let piece = viboceros_command::orient_curve::prepare_guide(&guide.original, Some(range))
            .and_then(|piece| Ok((piece.as_ref().to_nurbs()?, piece)));
        match piece {
            Ok((cursor, piece)) => {
                let s = self.orient_curve_session.as_mut().unwrap();
                let guide = s.guide.as_mut().unwrap();
                guide.range = Some(range);
                guide.working = piece;
                guide.cursor = cursor;
                s.subcurve = None;
                self.document.clear_selection();
                if s.options.on_curve {
                    s.phase = Phase::OnCurveBase;
                } else if !self.prepare_orient_curve() {
                    return Some(false);
                }
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        }
        self.orient_curve_prompt();
        Some(true)
    }

    pub(in crate::app) fn update_orient_subcurve_hover(&mut self, point: Point3) -> bool {
        let Some(s) = self.orient_curve_session.as_mut() else {
            return false;
        };
        let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) else {
            return false;
        };
        let source = p
            .object
            .and_then(|id| self.document.object(id))
            .map(|o| o.geometry_snapshot());
        p.hover_parameter = p
            .preview
            .parameter(source, None, point, self.document.tolerance());
        true
    }

    pub(in crate::app) fn continue_orient_subcurve_direction(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_curve_session.as_mut() else {
            return false;
        };
        let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) else {
            return false;
        };
        let input = input.trim().trim_start_matches('_');
        let locked = if input.eq_ignore_ascii_case("D") {
            Some(true)
        } else if let Some((key, value)) = input.split_once('=') {
            if !key.eq_ignore_ascii_case("Direction") {
                return false;
            }
            match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "locked" => Some(true),
                "free" => Some(false),
                _ => None,
            }
        } else {
            return false;
        };
        match locked {
            Some(false) => p.locked_forward = None,
            Some(true) => {
                let Some(hover) = p.hover_parameter else {
                    self.push_log("Move along the guide before locking direction".into());
                    return true;
                };
                match subcurve_input::cursor_forward(
                    s.guide.as_ref().unwrap().original.as_ref(),
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
            None => {
                self.push_log("Direction expects Free or Locked".into());
                return true;
            }
        }
        self.orient_curve_prompt();
        true
    }
}

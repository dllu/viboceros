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
            self.guide.as_ref()?.edge,
            [p.start?, p.hover_parameter?],
            p.length,
            p.locked_forward,
            doc.tolerance(),
        )
    }
}

impl VibocerosApp {
    fn discard_array_subcurve(&mut self) {
        let s = self.array_curve_surface_session.as_mut().unwrap();
        s.subcurve = None;
        s.guide = None;
        s.phase = Phase::Guide;
        self.point_filter = None;
        self.point_constraint = None;
        self.array_curve_surface_prompt();
    }

    pub(super) fn continue_array_subcurve(&mut self, input: &str) -> bool {
        let Some(s) = self.array_curve_surface_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("SubCrv") && s.phase == Phase::Guide {
            self.array_curve_surface_session.as_mut().unwrap().subcurve = Some(Default::default());
            self.array_curve_surface_prompt();
            return true;
        }
        if s.subcurve.is_none() {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.discard_array_subcurve();
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
                .array_curve_surface_session
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
            Ok(0.) => self.discard_array_subcurve(),
            Ok(length) => {
                let s = self.array_curve_surface_session.as_mut().unwrap();
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
                                self.accept_array_subcurve_point(point);
                            }
                        }
                        Ok(true) => {
                            p.locked_forward = None;
                            self.array_curve_surface_prompt();
                        }
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                } else {
                    self.array_curve_surface_prompt();
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }

    pub(super) fn accept_array_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.array_curve_surface_session.as_ref()?;
        if !matches!(s.phase, Phase::SubcurveStart | Phase::SubcurveEnd) {
            return None;
        }
        let guide = s.guide.as_ref().unwrap();
        if !guide.current(&self.document) {
            self.push_log("Error: array curve changed; restart ArrayCrvOnSrf".into());
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
            let s = self.array_curve_surface_session.as_mut().unwrap();
            s.subcurve.as_mut().unwrap().start = Some(parameter);
            s.phase = Phase::SubcurveEnd;
            self.array_curve_surface_prompt();
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
                self.discard_array_subcurve();
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
                let s = self.array_curve_surface_session.as_mut().unwrap();
                let guide = s.guide.as_mut().unwrap();
                guide.range = Some(range);
                guide.working = piece;
                guide.cursor = cursor;
                s.subcurve = None;
                self.document.clear_selection();
                guide.pick = Some(point);
                s.phase = Phase::Surface;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return Some(false);
            }
        }
        self.array_curve_surface_prompt();
        Some(true)
    }

    pub(in crate::app) fn update_array_subcurve_hover(&mut self, point: Point3) -> bool {
        let Some(s) = self.array_curve_surface_session.as_mut() else {
            return false;
        };
        let Some(p) = s.subcurve.as_mut().filter(|p| p.start.is_some()) else {
            return false;
        };
        let source = p
            .object
            .and_then(|id| self.document.object(id))
            .map(|o| o.geometry_snapshot());
        p.hover_parameter = p.preview.parameter(
            source,
            s.guide.as_ref().and_then(|g| g.edge),
            point,
            self.document.tolerance(),
        );
        true
    }

    pub(in crate::app) fn continue_array_subcurve_direction(&mut self, input: &str) -> bool {
        let Some(s) = self.array_curve_surface_session.as_mut() else {
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
            Err(error) => {
                self.push_log(error.into());
                return true;
            }
        }
        self.array_curve_surface_prompt();
        true
    }
}

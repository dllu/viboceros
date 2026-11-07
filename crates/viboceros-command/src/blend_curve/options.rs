use super::*;
pub(super) fn continuity_name(c: u8) -> &'static str {
    match c {
        0 => "G0",
        1 => "G1",
        2 => "G2",
        3 => "G3",
        4 => "G4",
        _ => unreachable!("validated curve blend derivative order"),
    }
}
fn continuity(value: &str) -> Result<u8, CommandError> {
    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
        "g0" | "position" => Ok(0),
        "g1" | "tangency" | "tangent" => Ok(1),
        "g2" | "curvature" => Ok(2),
        "g3" | "curvaturederivative" => Ok(3),
        "g4" | "curvaturesecondderivative" => Ok(4),
        _ => Err(CommandError::Usage(
            "BlendCrv continuity must be G0 through G4",
        )),
    }
}
impl Input {
    pub fn option(&mut self, text: &str) -> Result<bool, CommandError> {
        let mut next = self.clone();
        if !next.apply_option(text)? {
            return Ok(false);
        }
        next.rebuild()?;
        *self = next;
        Ok(true)
    }
    pub(super) fn apply_option(&mut self, text: &str) -> Result<bool, CommandError> {
        let word = text.trim_start_matches(['_', '-']);
        if option_name_eq(word, "Edges") || option_name_eq(word, "Curves") {
            self.surface_edges = option_name_eq(word, "Edges");
            return Ok(true);
        }
        let Some((key, value)) = text.split_once('=') else {
            return Ok(false);
        };
        let key = key.trim_start_matches(['_', '-']);
        let next = self;
        if option_name_eq(key, "BlendStart") {
            next.preferences.pick_point = if option_name_eq(value, "PickPoint") {
                true
            } else if option_name_eq(value, "CurveEnd") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            };
        } else if option_name_eq(key, "Trim") || option_name_eq(key, "Join") {
            let v = crate::parse_yes_no(value.trim_start_matches('_'))
                .ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(key, "Trim") {
                next.preferences.trim = v;
            } else {
                next.preferences.join = v;
            }
        } else {
            let (stem, i) = if let Some(s) = key.strip_suffix('1') {
                (s.trim_end_matches('_'), 0)
            } else if let Some(s) = key.strip_suffix('2') {
                (s.trim_end_matches('_'), 1)
            } else {
                return Ok(false);
            };
            if option_name_eq(stem, "Continuity") {
                let order = continuity(value)?;
                if order != 0
                    && next
                        .endpoints
                        .get(i)
                        .is_some_and(|e| matches!(e, Endpoint::Point(_)))
                {
                    return Err(CommandError::Usage(
                        "A point endpoint requires G0 continuity",
                    ));
                }
                next.continuity[i] = order;
            } else if option_name_eq(stem, "Handle") || option_name_eq(stem, "Bulge") {
                let v = parse_finite_real(value)?;
                if v <= 0. {
                    return Err(CommandError::Usage(
                        "Blend shape lengths and bulges must be positive",
                    ));
                }
                if option_name_eq(stem, "Handle") {
                    next.handles[i] = Some(v);
                } else {
                    next.handles[i] = None;
                    next.bulges[i] = v;
                }
            } else if option_name_eq(stem, "AtAngle") {
                let angle = parse_finite_real(value)?;
                next.freeze_handles();
                let surface = next
                    .endpoints
                    .get_mut(i)
                    .and_then(Endpoint::curve_mut)
                    .and_then(|g| g.surface.as_mut())
                    .ok_or(CommandError::Usage(
                        "AtAngle requires a captured surface edge",
                    ))?;
                std::sync::Arc::make_mut(surface).angle = angle;
            } else if option_name_eq(stem, "Parameter") {
                let t = parse_finite_real(value)?;
                next.freeze_handles();
                let g = next.endpoints.get_mut(i).ok_or(CommandError::Usage(
                    "Pick the blend sources before changing their parameters",
                ))?;
                let g = g.curve_mut().ok_or(CommandError::Usage(
                    "A point endpoint has no curve parameter",
                ))?;
                let point = g.curve_ref().evaluate(t)?;
                if let Some(surface) = &mut g.surface {
                    std::sync::Arc::make_mut(surface).move_contact(point)?;
                }
                g.parameter = t;
            } else if option_name_eq(stem, "Flip") {
                let v = crate::parse_yes_no(value.trim_start_matches('_'))
                    .ok_or(CommandError::Usage(USAGE))?;
                let guide = next
                    .endpoints
                    .get_mut(i)
                    .ok_or(CommandError::Usage(
                        "Pick the blend sources before flipping",
                    ))?
                    .curve_mut()
                    .ok_or(CommandError::Usage("A point endpoint cannot flip"))?;
                if v && guide.surface.is_some() {
                    return Err(CommandError::Usage(
                        "Surface-edge direction is controlled by AtAngle",
                    ));
                }
                guide.flip = v;
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn option_with_units(
        &mut self,
        text: &str,
        units: &viboceros_geometry::LengthUnitSystem,
    ) -> Result<bool, CommandError> {
        self.option(&normalized_option(text, units)?)
    }
}

pub(super) fn normalized_option(
    text: &str,
    units: &viboceros_geometry::LengthUnitSystem,
) -> Result<String, CommandError> {
    if let Some((key, value)) = text.split_once('=')
        && (option_name_eq(key, "Handle1") || option_name_eq(key, "Handle2"))
    {
        let v = viboceros_drafting::PointInput::parse_length_with_units(value, units)
            .ok_or(CommandError::Usage(USAGE))?
            .map_err(|_| CommandError::Usage(USAGE))?;
        return Ok(format!("{key}={v}"));
    }
    Ok(text.to_owned())
}

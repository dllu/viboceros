use super::*;
pub(super) fn continuity_name(c: Continuity) -> &'static str {
    match c {
        Continuity::Position => "G0",
        Continuity::Tangency => "G1",
        Continuity::Curvature => "G2",
    }
}
fn continuity(value: &str) -> Result<Continuity, CommandError> {
    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
        "g0" | "position" => Ok(Continuity::Position),
        "g1" | "tangency" | "tangent" => Ok(Continuity::Tangency),
        "g2" | "curvature" => Ok(Continuity::Curvature),
        _ => Err(CommandError::Usage(
            "BlendCrv currently supports G0, G1 and G2 continuity",
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
                next.continuity[i] = continuity(value)?;
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
            } else if option_name_eq(stem, "Parameter") {
                let t = parse_finite_real(value)?;
                next.freeze_handles();
                let g = next.guides.get_mut(i).ok_or(CommandError::Usage(
                    "Pick the blend sources before changing their parameters",
                ))?;
                g.source.curve_ref().evaluate(t)?;
                g.parameter = t;
            } else if option_name_eq(stem, "Flip") {
                let v = crate::parse_yes_no(value.trim_start_matches('_'))
                    .ok_or(CommandError::Usage(USAGE))?;
                next.guides
                    .get_mut(i)
                    .ok_or(CommandError::Usage(
                        "Pick the blend sources before flipping",
                    ))?
                    .flip = v;
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

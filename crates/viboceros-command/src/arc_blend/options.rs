use super::*;
use std::borrow::Cow;
use viboceros_geometry::LengthUnitSystem;

pub fn parse_number(text: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    viboceros_drafting::PointInput::parse_length_with_units(text, units)
        .and_then(Result::ok)
        .ok_or(CommandError::Usage("Expected a finite ArcBlend number"))
}

pub(super) fn normalized_option<'a>(
    word: &'a str,
    units: &LengthUnitSystem,
) -> Result<Cow<'a, str>, CommandError> {
    if let Some((key, value)) = word.split_once('=')
        && (option_name_eq(key, "RadiusDifference") || option_name_eq(key, "Ratio"))
    {
        return Ok(format!("{key}={}", parse_number(value, units)?).into());
    }
    Ok(word.into())
}
impl Input {
    pub fn option_with_units(
        &mut self,
        word: &str,
        units: &LengthUnitSystem,
    ) -> Result<bool, CommandError> {
        self.option(&normalized_option(word, units)?)
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        let mut next = self.clone();
        if !next.apply_option(word)? {
            return Ok(false);
        }
        next.rebuild()?;
        *self = next;
        Ok(true)
    }
    pub(super) fn apply_option(&mut self, word: &str) -> Result<bool, CommandError> {
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if option_name_eq(key, "AlternateSolution") {
            self.geometry.alternate = value.map_or(Ok(!self.geometry.alternate), boolean)?;
        } else if option_name_eq(key, "Trim") {
            self.trim = value.map_or(Ok(!self.trim), boolean)?;
        } else if option_name_eq(key, "Join") {
            if !self.trim {
                return Err(CommandError::Usage(
                    "ArcBlend Join is available with Trim=Yes",
                ));
            }
            self.join = value.map_or(Ok(!self.join), boolean)?;
        } else if option_name_eq(key, "Ratio") {
            let r = parse_finite_real(value.ok_or(CommandError::Usage(USAGE))?)?;
            if r < 0. {
                return Err(CommandError::Usage("ArcBlend Ratio must be nonnegative"));
            }
            self.radius_difference = (r == 0.).then_some(self.last_radius_difference);
            self.radius_target_deferred = false;
            if r > 0. {
                self.geometry.ratio = r;
            }
        } else if option_name_eq(key, "RadiusDifference") {
            let target = parse_finite_real(value.ok_or(CommandError::Usage(USAGE))?)?;
            self.radius_difference = Some(target);
            self.last_radius_difference = target;
        } else if let Some(index) = ["Parameter1", "Parameter2"]
            .iter()
            .position(|k| option_name_eq(key, k))
        {
            let t = parse_finite_real(value.ok_or(CommandError::Usage(USAGE))?)?;
            let g = self.guides.get_mut(index).ok_or(CommandError::Usage(
                "Pick both ArcBlend source curves before changing their parameters",
            ))?;
            g.curve_ref().evaluate(t)?;
            g.parameter = t;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}

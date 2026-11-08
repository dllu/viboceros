use super::*;
use viboceros_geometry::LengthUnitSystem;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub smooth: bool,
    pub roundness: Real,
    /// None follows document absolute tolerance.
    pub tolerance: Option<Real>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            smooth: true,
            roundness: 1.,
            tolerance: None,
        }
    }
}

impl Options {
    pub fn for_document(mut self, model: Tolerance) -> Result<Self, CommandError> {
        self = parse(&[], self, &LengthUnitSystem::None)?;
        self.tolerance = Some(
            self.tolerance
                .unwrap_or(model.absolute())
                .max(model.absolute()),
        );
        Ok(self)
    }
    pub fn effective_tolerance(self, model: Tolerance) -> Result<Real, CommandError> {
        let t = self.tolerance.unwrap_or(model.absolute());
        if !t.is_finite()
            || t < model.absolute()
            || !self.roundness.is_finite()
            || !(0. ..=1.).contains(&self.roundness)
        {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(t)
    }
    pub fn arguments(self, model: Tolerance) -> Result<String, CommandError> {
        Ok(format!(
            "Smooth={} Tolerance={} Roundness={}",
            if self.smooth { "Yes" } else { "No" },
            self.effective_tolerance(model)?,
            self.roundness
        ))
    }
}

pub fn parse(
    args: &[&str],
    mut options: Options,
    units: &LengthUnitSystem,
) -> Result<Options, CommandError> {
    let mut seen = [false; 3];
    let mut i = 0;
    while i < args.len() {
        let (name, value, consumed) = if let Some((k, v)) = args[i].split_once('=') {
            (k, v, 1)
        } else {
            (
                args[i],
                *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        let index = if option_name_eq(name, "Smooth") {
            options.smooth = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            0
        } else if option_name_eq(name, "Tolerance") {
            options.tolerance = Some(number(value, units)?);
            1
        } else if option_name_eq(name, "Roundness") {
            options.roundness = number(value, &LengthUnitSystem::None)?;
            2
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if std::mem::replace(&mut seen[index], true) {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    if !options.roundness.is_finite()
        || !(0. ..=1.).contains(&options.roundness)
        || options.tolerance.is_some_and(|t| !t.is_finite() || t <= 0.)
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(options)
}

fn number(value: &str, units: &LengthUnitSystem) -> Result<Real, CommandError> {
    viboceros_drafting::PointInput::parse_length_with_units(value.trim_start_matches('_'), units)
        .ok_or(CommandError::Usage(USAGE))?
        .map_err(|_| CommandError::Usage(USAGE))
}

//! Unit-aware thickness parsing and command-instance preferences.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellOptions {
    pub thickness: f64,
    pub delete_input: bool,
}
impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            thickness: 1.,
            delete_input: true,
        }
    }
}
impl ShellOptions {
    pub fn validate(self) -> Result<Self, CommandError> {
        if !self.thickness.is_finite() || self.thickness < 0. {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(self)
    }
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        self.update_with_units(words, &LengthUnitSystem::default())
    }
    pub fn update_with_units(
        &mut self,
        words: &[&str],
        units: &LengthUnitSystem,
    ) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let (key, value) = word
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "thickness" => {
                    let n = viboceros_drafting::PointInput::parse_length_with_units(value, units)
                        .ok_or(CommandError::Usage(USAGE))?
                        .map_err(|_| CommandError::Usage(USAGE))?;
                    if n < 0. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.thickness = n;
                }
                "deleteinput" => {
                    next.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next.validate()?;
        Ok(())
    }
    pub fn command_options(self) -> String {
        format!(
            "Thickness={} DeleteInput={}",
            self.thickness,
            if self.delete_input { "Yes" } else { "No" }
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellInput {
    pub options: ShellOptions,
    pub faces: Option<SurfaceFaces>,
}
impl ShellInput {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        Self::parse_with_units(words, &LengthUnitSystem::default())
    }
    pub fn parse_with_units(
        words: &[&str],
        units: &LengthUnitSystem,
    ) -> Result<Self, CommandError> {
        Self::parse_with_options(words, units, ShellOptions::default())
    }
    pub fn parse_with_options(
        words: &[&str],
        units: &LengthUnitSystem,
        defaults: ShellOptions,
    ) -> Result<Self, CommandError> {
        let (mut faces, mut options) = (Vec::new(), Vec::new());
        for word in words {
            if word
                .trim_start_matches('_')
                .split_once('=')
                .is_some_and(|(k, _)| {
                    ["Face", "Faces", "FaceRefs"]
                        .iter()
                        .any(|s| k.eq_ignore_ascii_case(s))
                })
            {
                faces.push(*word);
            } else {
                options.push(*word);
            }
        }
        let mut result = Self {
            options: defaults,
            faces: None,
        };
        result.options.update_with_units(&options, units)?;
        if !faces.is_empty() {
            result.faces = Some(
                SurfaceExtrusionInput::parse(&faces)
                    .map_err(|_| CommandError::Usage(USAGE))?
                    .faces,
            );
        }
        Ok(result)
    }
}

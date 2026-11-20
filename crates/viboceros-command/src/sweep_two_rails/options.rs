use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SectionFit {
    #[default]
    Unchanged,
    Rebuild(usize),
    Refit(Real),
}

impl SectionFit {
    pub fn parse(key: &str, value: &str) -> Result<Self, CommandError> {
        match key
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "rebuild" | "rebuildcount" => match value.parse::<usize>() {
                Ok(0) => Ok(Self::Unchanged),
                Ok(count @ 2..=128) => Ok(Self::Rebuild(count)),
                _ => Err(CommandError::Usage(USAGE)),
            },
            "refit" | "refittolerance" => match parse_finite_real(value)? {
                0. => Ok(Self::Unchanged),
                distance if distance > 0. => Ok(Self::Refit(distance)),
                _ => Err(CommandError::Usage(USAGE)),
            },
            "simplify" if option_name_eq(value, "None") => Ok(Self::Unchanged),
            _ => Err(CommandError::Usage(USAGE)),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    names: Option<[String; 2]>,
    pub shape: ShapeOptions,
    pub slashes: Vec<[Point3; 2]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapeOptions {
    pub fit: SectionFit,
    pub ends: SweepEndPoints,
    pub maintain_height: bool,
    pub refit_rails: bool,
    pub closed: bool,
}

impl Options {
    pub fn parse(args: &[&str]) -> Result<Self, CommandError> {
        let mut names = [None, None];
        let mut fit = None;
        let mut ends = SweepEndPoints::default();
        let mut maintain_height = None;
        let mut refit_rails = None;
        let mut closed = None;
        let mut slashes = Vec::new();
        for arg in args {
            let (key, value) = arg.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if let Some(index) = ["Rail1Name", "Rail2Name"]
                .iter()
                .position(|name| option_name_eq(key, name))
            {
                if value.is_empty() || names[index].replace(value.to_owned()).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if option_name_eq(key, "StartPoint") || option_name_eq(key, "EndPoint") {
                let slot = if option_name_eq(key, "StartPoint") {
                    &mut ends.start
                } else {
                    &mut ends.end
                };
                if slot
                    .replace(parse_single_option_point(value, USAGE)?)
                    .is_some()
                {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if option_name_eq(key, "MaintainHeight") {
                if maintain_height
                    .replace(Self::parse_toggle(value)?)
                    .is_some()
                {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if option_name_eq(key, "RefitRails") {
                if refit_rails.replace(Self::parse_toggle(value)?).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if option_name_eq(key, "Closed") {
                if closed.replace(Self::parse_toggle(value)?).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if option_name_eq(key, "Slash") {
                if slashes.len() == MAX_SLASHES {
                    return Err(CommandError::Usage(SLASH_USAGE));
                }
                slashes.push(Self::parse_slash(value)?);
            } else if fit.replace(SectionFit::parse(key, value)?).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let names = match names {
            [None, None] => None,
            [Some(a), Some(b)] => Some([a, b]),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        Ok(Self {
            names,
            slashes,
            shape: ShapeOptions {
                fit: fit.unwrap_or_default(),
                ends,
                maintain_height: maintain_height.unwrap_or_default(),
                refit_rails: refit_rails.unwrap_or_default(),
                closed: closed.unwrap_or_default(),
            },
        })
    }

    pub fn parse_toggle(value: &str) -> Result<bool, CommandError> {
        parse_yes_no(value).ok_or(CommandError::Usage("Choose Yes or No"))
    }

    pub fn parse_slash(value: &str) -> Result<[Point3; 2], CommandError> {
        let (first, second) = value
            .split_once('/')
            .ok_or(CommandError::Usage(SLASH_USAGE))?;
        Ok([
            parse_single_option_point(first, SLASH_USAGE)?,
            parse_single_option_point(second, SLASH_USAGE)?,
        ])
    }

    pub fn explicit_rails(&self, doc: &Document) -> Result<Option<[ObjectId; 2]>, CommandError> {
        let Some([first, second]) = &self.names else {
            return Ok(None);
        };
        let lookup =
            |name: &str| extrude_along_curve::resolve_curve_along_curve_path(doc, &[], Some(name));
        let rails = [lookup(first)?, lookup(second)?];
        if rails[0] == rails[1] {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Some(rails))
    }
}

impl ShapeOptions {
    pub(super) fn native(
        self,
        slashes: &[[Point3; 2]],
    ) -> viboceros_smlib::TwoRailSweepOptions<'_> {
        viboceros_smlib::TwoRailSweepOptions {
            ends: self.ends,
            maintain_height: self.maintain_height,
            refit_rails: self.refit_rails,
            closed: self.closed,
            slashes,
        }
    }
}

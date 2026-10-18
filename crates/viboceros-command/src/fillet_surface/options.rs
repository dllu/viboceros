use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trim {
    No,
    Yes,
    Split,
}
impl Trim {
    pub fn token(self) -> &'static str {
        match self {
            Self::No => "No",
            Self::Yes => "Yes",
            Self::Split => "Split",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub sources: Option<[ObjectId; 2]>,
    pub picks: [Option<[Real; 2]>; 2],
    pub radius: Real,
    pub extend: bool,
    pub trim: Trim,
    pub join: bool,
    pub preview: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            sources: None,
            picks: [None; 2],
            radius: 1.,
            extend: true,
            trim: Trim::Yes,
            join: false,
            preview: false,
        }
    }
}
impl Options {
    pub fn updated(&self, args: &[&str]) -> Result<Self, CommandError> {
        parse_defaults(args, self.clone())
    }
}
pub fn parse(args: &[&str]) -> Result<Options, CommandError> {
    parse_defaults(args, Options::default())
}
fn boolean(s: &str) -> Result<bool, CommandError> {
    if s.eq_ignore_ascii_case("Yes") {
        Ok(true)
    } else if s.eq_ignore_ascii_case("No") {
        Ok(false)
    } else {
        Err(CommandError::Usage(USAGE))
    }
}
fn parse_defaults(args: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let mut seen = BTreeSet::new();
    for arg in args {
        let Some((key, value)) = arg.split_once('=') else {
            return Err(CommandError::Usage(USAGE));
        };
        let key = key.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_');
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.as_str() {
            "sources" => {
                let ids = value
                    .split(',')
                    .map(|s| s.parse().map_err(|_| CommandError::Usage(USAGE)))
                    .collect::<Result<Vec<ObjectId>, _>>()?;
                options.sources = Some(ids.try_into().map_err(|_| CommandError::Usage(USAGE))?);
            }
            "pick1" | "pick2" => {
                let uv = value
                    .split(',')
                    .map(parse_finite_real)
                    .collect::<Result<Vec<_>, _>>()?;
                options.picks[usize::from(key.ends_with('2'))] =
                    Some(uv.try_into().map_err(|_| CommandError::Usage(USAGE))?);
            }
            "radius" => {
                options.radius = parse_finite_real(value)?;
                if options.radius <= 0. {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            "extend" => options.extend = boolean(value)?,
            "join" => options.join = boolean(value)?,
            "preview" => options.preview = boolean(value)?,
            "trim" => {
                options.trim = if value.eq_ignore_ascii_case("Split") {
                    Trim::Split
                } else if boolean(value)? {
                    Trim::Yes
                } else {
                    Trim::No
                }
            }
            "blendtype" => {
                if !value.eq_ignore_ascii_case("Arc")
                    && !value.eq_ignore_ascii_case("CircularFillet")
                {
                    return Err(failure("only BlendType=Arc is currently available"));
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if options.sources.is_some_and(|ids| ids[0] == ids[1]) {
        return Err(CommandError::Usage(USAGE));
    }
    if options.join && options.trim != Trim::Yes {
        return Err(failure("Join=Yes requires Trim=Yes"));
    }
    Ok(options)
}

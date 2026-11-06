use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteInput {
    None,
    All,
    Used,
}
impl DeleteInput {
    pub fn token(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::All => "All",
            Self::Used => "Used",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Curves,
    Surfaces,
}
impl Output {
    pub fn token(self) -> &'static str {
        match self {
            Self::Curves => "Curves",
            Self::Surfaces => "Surfaces",
        }
    }
}
/// Application preferences, shared by command aliases and independent of Undo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub delete_input: DeleteInput,
    pub combine: bool,
    pub output: Output,
    pub simplify: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            delete_input: DeleteInput::None,
            combine: true,
            output: Output::Curves,
            simplify: true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Input {
    pub options: Options,
    pub sources: Option<Vec<ObjectId>>,
    pub all_regions: bool,
    pub points: Vec<Point3>,
}
pub fn parse(args: &[&str], defaults: Options) -> Result<Input, CommandError> {
    let mut input = Input {
        options: defaults,
        sources: None,
        all_regions: false,
        points: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    for arg in args {
        if arg
            .trim_start_matches('_')
            .eq_ignore_ascii_case("AllRegions")
        {
            if input.all_regions {
                return Err(CommandError::Usage(USAGE));
            }
            input.all_regions = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            let point = arg.trim_start_matches(['w', 'W']);
            let (point, consumed) = parse_point(&[point])?;
            if consumed != 1 {
                return Err(CommandError::Usage(USAGE));
            }
            input.points.push(point);
            continue;
        };
        let key = key.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_');
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match key.as_str() {
            "deleteinput" => {
                input.options.delete_input = if value.eq_ignore_ascii_case("None") {
                    DeleteInput::None
                } else if value.eq_ignore_ascii_case("All") {
                    DeleteInput::All
                } else if value.eq_ignore_ascii_case("Used") {
                    DeleteInput::Used
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            "combineregions" => {
                input.options.combine = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "simplifycurves" => {
                input.options.simplify = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "output" => {
                input.options.output = if value.eq_ignore_ascii_case("Curves") {
                    Output::Curves
                } else if value.eq_ignore_ascii_case("Surfaces") {
                    Output::Surfaces
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            "sources" => {
                let ids = value
                    .split(',')
                    .map(|id| id.parse().map_err(|_| CommandError::Usage(USAGE)))
                    .collect::<Result<Vec<ObjectId>, _>>()?;
                if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
                    return Err(CommandError::Usage(USAGE));
                }
                input.sources = Some(ids);
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    Ok(input)
}

use super::*;
pub(super) const USAGE: &str = "CurvatureAnalysis [Add|Remove|MaxRange] [Style=Gaussian|Mean|MinRadius|MaxRadius] [Range=blue,red] [ShowIsoCurves=Yes|No] | CurvatureAnalysisOff";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub style: CurvatureStyle,
    pub ranges: [[Real; 2]; 4],
    pub show_isocurves: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            style: CurvatureStyle::Gaussian,
            ranges: [[-0.1, 0.1], [-0.1, 0.1], [0., 1.], [0., 1.]],
            show_isocurves: false,
        }
    }
}
impl Options {
    pub fn index(self) -> usize {
        CurvatureStyle::ALL
            .iter()
            .position(|s| *s == self.style)
            .unwrap()
    }
    pub fn range(self) -> [Real; 2] {
        self.ranges[self.index()]
    }
    pub fn mapping(self) -> Result<CurvatureColorMapping, viboceros_geometry::GeometryError> {
        CurvatureColorMapping::try_new(self.style, self.range())
    }
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    Start,
    Add,
    Remove,
    Configure,
    MaxRange,
}

pub(super) fn parse(options: &mut Options, args: &[&str]) -> Result<Action, CommandError> {
    let mut action = None;
    let mut keys = BTreeSet::new();
    let mut range = None;
    for arg in args {
        let arg = arg.trim_start_matches(['_', '-']);
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.to_ascii_lowercase();
            if !keys.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "style" => {
                    options.style = CurvatureStyle::ALL
                        .into_iter()
                        .find(|s| s.name().eq_ignore_ascii_case(value))
                        .ok_or(CommandError::Usage(USAGE))?
                }
                "range" => {
                    range = Some(
                        value
                            .split(',')
                            .map(str::parse::<Real>)
                            .collect::<Result<Vec<_>, _>>()
                            .ok()
                            .and_then(|v| <[Real; 2]>::try_from(v).ok())
                            .filter(|v| v.iter().all(|x| x.is_finite()))
                            .ok_or(CommandError::Usage(USAGE))?,
                    )
                }
                "showisocurves" => {
                    options.show_isocurves =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            let next = match arg.to_ascii_lowercase().as_str() {
                "add" => Action::Add,
                "remove" => Action::Remove,
                "maxrange" => Action::MaxRange,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            if action.replace(next).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
    }
    // Rhino's public settings retain the previous interval when its two
    // endpoints coincide; the low-level color mapper also supports singletons.
    if let Some(range) = range
        && range[0] != range[1]
    {
        let i = options.index();
        options.ranges[i] = range;
    }
    Ok(action.unwrap_or(if args.is_empty() {
        Action::Start
    } else {
        Action::Configure
    }))
}

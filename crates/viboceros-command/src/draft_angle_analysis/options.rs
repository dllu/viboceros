use super::*;
use viboceros_geometry::DraftAngleMapping;
pub(super) const USAGE: &str = "DraftAngleAnalysis [Add|Remove] [Range=lower,upper] [Up=x,y,z] [ShowIsoCurves=Yes|No] | DraftAngleAnalysisOff";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub range: [Real; 2],
    pub up: [Real; 3],
    pub show_isocurves: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            range: [0., 5.],
            up: [0., 0., 1.],
            show_isocurves: false,
        }
    }
}
impl Options {
    pub fn mapping(self) -> Result<DraftAngleMapping, viboceros_geometry::GeometryError> {
        DraftAngleMapping::try_new(
            Vector3::try_new(self.up[0], self.up[1], self.up[2])?,
            self.range,
        )
    }
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    Start,
    Add,
    Remove,
    Configure,
}

pub(super) fn parse(options: &mut Options, args: &[&str]) -> Result<Action, CommandError> {
    let mut action = None;
    let mut keys = BTreeSet::new();
    for arg in args {
        let arg = arg.trim_start_matches(['_', '-']);
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.to_ascii_lowercase();
            if !keys.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            fn numbers<const N: usize>(value: &str) -> Result<[Real; N], CommandError> {
                value
                    .split(',')
                    .map(str::parse::<Real>)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
                    .and_then(|v| v.try_into().ok())
                    .ok_or(CommandError::Usage(USAGE))
            }
            match key.as_str() {
                "range" => options.range = numbers(value)?,
                "up" => options.up = numbers(value)?,
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
                _ => return Err(CommandError::Usage(USAGE)),
            };
            if action.replace(next).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
    }
    let mapping = options.mapping().map_err(|_| CommandError::Usage(USAGE))?;
    options.up = mapping.up().to_array();
    Ok(action.unwrap_or(if args.is_empty() {
        Action::Start
    } else {
        Action::Configure
    }))
}

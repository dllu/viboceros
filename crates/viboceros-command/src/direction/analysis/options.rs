use super::*;
pub(super) const USAGE: &str = "ShowDir [Add|Remove|Flip|UReverse|VReverse|SwapUV] [Direction=Yes|No] [U=Yes|No] [V=Yes|No] [Color=r,g,b] | ShowDirOff";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub direction: bool,
    pub u: bool,
    pub v: bool,
    pub color: [u8; 3],
}
impl Default for Options {
    fn default() -> Self {
        Self {
            direction: true,
            u: true,
            v: true,
            color: [130, 65, 210],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Start,
    Add,
    Remove,
    Configure,
    Edit(DirectionEdit),
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
            let yes = || parse_yes_no(value).ok_or(CommandError::Usage(USAGE));
            match key.as_str() {
                "direction" => options.direction = yes()?,
                "u" => options.u = yes()?,
                "v" => options.v = yes()?,
                "color" => {
                    options.color = value
                        .split(',')
                        .map(str::parse::<u8>)
                        .collect::<Result<Vec<_>, _>>()
                        .ok()
                        .and_then(|v| v.try_into().ok())
                        .ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            let parsed = match arg.to_ascii_lowercase().as_str() {
                "add" => Action::Add,
                "remove" => Action::Remove,
                _ => Action::Edit(
                    parse_direction_edit(&[arg]).map_err(|_| CommandError::Usage(USAGE))?,
                ),
            };
            if action.replace(parsed).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
        }
    }
    Ok(action.unwrap_or(if args.is_empty() {
        Action::Start
    } else {
        Action::Configure
    }))
}

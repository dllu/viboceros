use super::*;
pub const USAGE: &str = "Inset [distance|Distance=value] [Mode=Group|Single] [CreaseCorners=Yes|No] [Faces=All|0,1,...]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Group,
    Single,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: f64,
    pub mode: Mode,
    pub crease_corners: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 0.1,
            mode: Mode::Group,
            crease_corners: false,
        }
    }
}
impl Options {
    pub fn validate(self) -> Result<(), CommandError> {
        if !self.distance.is_finite() || self.distance <= 0. {
            Err(CommandError::Usage(USAGE))
        } else {
            Ok(())
        }
    }
    pub fn command(self) -> String {
        format!(
            "Inset Distance={} Mode={}",
            self.distance,
            if self.mode == Mode::Group {
                "Group"
            } else {
                "Single"
            }
        )
    }
}
#[derive(Clone, Debug)]
pub struct Input {
    pub options: Options,
    pub distance_supplied: bool,
    pub faces: Option<Vec<usize>>,
    pub faces_supplied: bool,
    pub crease_supplied: bool,
}
pub fn parse(args: &[&str], mut options: Options) -> Result<Input, CommandError> {
    let mut seen = [false; 4];
    let mut faces = None;
    let mut i = 0;
    while i < args.len() {
        let (name, value, consumed) = if let Some((name, value)) = args[i].split_once('=') {
            (name, value, 1)
        } else if parse_finite_real(args[i]).is_ok() {
            ("Distance", args[i], 1)
        } else {
            (
                args[i],
                *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        let index = if option_name_eq(name, "Distance") {
            options.distance = parse_finite_real(value)?;
            0
        } else if option_name_eq(name, "Mode") {
            options.mode = if option_name_eq(value, "Group") {
                Mode::Group
            } else if option_name_eq(value, "Single") {
                Mode::Single
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            1
        } else if option_name_eq(name, "CreaseCorners") {
            options.crease_corners = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            3
        } else if option_name_eq(name, "Faces") {
            if value.len() > 4_000_000 {
                return Err(CommandError::Usage(USAGE));
            }
            if !option_name_eq(value, "All") {
                faces = Some(
                    value
                        .split(',')
                        .enumerate()
                        .map(|(i, v)| {
                            if i >= 1_000_000 {
                                Err(CommandError::Usage(USAGE))
                            } else {
                                v.parse::<usize>().map_err(|_| CommandError::Usage(USAGE))
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            2
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if std::mem::replace(&mut seen[index], true) {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    options.validate()?;
    Ok(Input {
        options,
        distance_supplied: seen[0],
        faces_supplied: seen[2],
        faces,
        crease_supplied: seen[3],
    })
}

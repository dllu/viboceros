use super::*;
pub const USAGE: &str = "Inset [distance|Distance=value] [Mode=Group|Single] [CreaseCorners=Yes|No] [Loose=On|Off] [Split=Yes|No] [JoinOutputs=Yes|No] [Faces=All|0,1,...]";
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
    pub loose: bool,
    pub split: bool,
    pub join_outputs: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 0.1,
            mode: Mode::Group,
            crease_corners: false,
            loose: false,
            split: true,
            join_outputs: true,
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
    pub fn surface_command(self) -> String {
        let mut text = format!(
            "Inset Distance={} CreaseCorners={} Loose={} Split={}",
            self.distance,
            if self.crease_corners { "Yes" } else { "No" },
            if self.loose { "On" } else { "Off" },
            if self.split { "Yes" } else { "No" }
        );
        if self.split {
            text.push_str(if self.join_outputs {
                " JoinOutputs=Yes"
            } else {
                " JoinOutputs=No"
            });
        }
        text
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
    pub loose_supplied: bool,
    pub split_supplied: bool,
    pub join_supplied: bool,
}
pub fn parse(args: &[&str], mut options: Options) -> Result<Input, CommandError> {
    let mut seen = [false; 7];
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
        } else if option_name_eq(name, "Loose") {
            options.loose = if option_name_eq(value, "On") {
                true
            } else if option_name_eq(value, "Off") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            4
        } else if option_name_eq(name, "Split") {
            options.split = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            5
        } else if option_name_eq(name, "JoinOutputs") {
            options.join_outputs = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            6
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
        loose_supplied: seen[4],
        split_supplied: seen[5],
        join_supplied: seen[6],
    })
}

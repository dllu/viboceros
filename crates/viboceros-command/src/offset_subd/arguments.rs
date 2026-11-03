use super::*;
const USAGE: &str = "OffsetSubD [distance|Distance=value] [Solid=Yes|No] [BothSides=Yes|No] [DeleteInput=Yes|No] [FlipAll]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: f64,
    pub solid: bool,
    pub both_sides: bool,
    pub delete_input: bool,
    pub flip_all: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 1.,
            solid: false,
            both_sides: false,
            delete_input: false,
            flip_all: false,
        }
    }
}
impl Options {
    pub fn validate(self) -> Result<(), CommandError> {
        if !self.distance.is_finite() || self.distance == 0. {
            Err(CommandError::Usage(USAGE))
        } else {
            Ok(())
        }
    }
    pub fn remembered(self) -> Self {
        Self {
            flip_all: false,
            ..self
        }
    }
    pub fn command(self) -> String {
        format!(
            "OffsetSubD Distance={} Solid={} BothSides={} DeleteInput={} FlipAll={}",
            self.distance,
            yes(self.solid),
            yes(self.both_sides),
            yes(self.delete_input),
            yes(self.flip_all)
        )
    }
}
fn yes(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub options: Options,
    pub distance_supplied: bool,
}
pub fn parse(args: &[&str], mut options: Options) -> Result<Input, CommandError> {
    let mut seen = [false; 5];
    let mut i = 0;
    while i < args.len() {
        let word = args[i];
        let mut consumed = 1;
        let (name, value) = if let Some(pair) = word.split_once('=') {
            pair
        } else if parse_finite_real(word).is_ok() {
            ("Distance", word)
        } else if option_name_eq(word, "FlipAll") {
            ("FlipAll", if options.flip_all { "No" } else { "Yes" })
        } else {
            consumed = 2;
            (word, *args.get(i + 1).ok_or(CommandError::Usage(USAGE))?)
        };
        let index = if option_name_eq(name, "Distance") {
            options.distance = parse_finite_real(value)?;
            0
        } else {
            let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "Solid") {
                options.solid = value;
                1
            } else if option_name_eq(name, "BothSides") {
                options.both_sides = value;
                2
            } else if option_name_eq(name, "DeleteInput") {
                options.delete_input = value;
                3
            } else if option_name_eq(name, "FlipAll") {
                options.flip_all = value;
                4
            } else {
                return Err(CommandError::Usage(USAGE));
            }
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
    })
}

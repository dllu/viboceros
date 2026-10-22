//! Getter choices are local to one invocation, matching the native reset behavior.
use super::*;
pub const USAGE: &str = "ExtractControlPolygon [OutputLayer=Current|Input|TargetObject] [SubDPriority=Geometry|TextureCoordinates]";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputLayer {
    #[default]
    Current,
    Input,
    TargetObject,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub output_layer: OutputLayer,
    pub priority: SubDControlNetPriority,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            output_layer: OutputLayer::Current,
            priority: SubDControlNetPriority::Geometry,
        }
    }
}
pub fn parse(args: &[&str]) -> Result<Options, CommandError> {
    let mut options = Options::default();
    let mut seen = [false; 2];
    let mut i = 0;
    while i < args.len() {
        let (name, value, used) = orient_option(args, i, USAGE)?;
        let value = value.trim_start_matches('_');
        let index = if option_name_eq(name, "OutputLayer") {
            options.output_layer = if value.eq_ignore_ascii_case("Current") {
                OutputLayer::Current
            } else if value.eq_ignore_ascii_case("Input") {
                OutputLayer::Input
            } else if value.eq_ignore_ascii_case("TargetObject") {
                OutputLayer::TargetObject
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            0
        } else if option_name_eq(name, "SubDPriority") {
            options.priority = if value.eq_ignore_ascii_case("Geometry") {
                SubDControlNetPriority::Geometry
            } else if value.eq_ignore_ascii_case("TextureCoordinates") {
                SubDControlNetPriority::TextureCoordinates
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if seen[index] {
            return Err(CommandError::Usage(USAGE));
        }
        seen[index] = true;
        i += used;
    }
    Ok(options)
}
impl Options {
    pub fn prompt(self) -> ObjectSelectionPrompt {
        ObjectSelectionPrompt {
            command: "ExtractControlPolygon",
            filter: ObjectSelectionFilter::ControlPolygons,
            options: vec![],
            menus: vec![],
            choices: vec![
                ChoiceSelectionOption {
                    name: "OutputLayer",
                    value: match self.output_layer {
                        OutputLayer::Current => "Current",
                        OutputLayer::Input => "Input",
                        OutputLayer::TargetObject => "TargetObject",
                    },
                    choices: &["Current", "Input", "TargetObject"],
                    toggle: None,
                },
                ChoiceSelectionOption {
                    name: "SubDPriority",
                    value: match self.priority {
                        SubDControlNetPriority::Geometry => "Geometry",
                        SubDControlNetPriority::TextureCoordinates => "TextureCoordinates",
                    },
                    choices: &["Geometry", "TextureCoordinates"],
                    toggle: None,
                },
            ],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }
    }
}

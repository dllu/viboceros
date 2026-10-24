//! Shared typed and interactive output choices.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExtractPointOutputLayer {
    Input,
    Current,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExtractPointOutput {
    Points,
    PointCloud,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExtractPointOptions {
    pub(super) output_layer: ExtractPointOutputLayer,
    pub(super) output: ExtractPointOutput,
    pub(super) components: Vec<components::Target>,
    pub(super) control_net: bool,
    pub(super) ranges: Vec<ranges::Target>,
}

pub(super) fn parse_extract_point_arguments(
    arguments: &[&str],
) -> Result<ExtractPointOptions, CommandError> {
    let mut output_layer = ExtractPointOutputLayer::Input;
    let mut output = ExtractPointOutput::Points;
    let mut components = Vec::new();
    let mut ranges = Vec::new();
    let mut control_net = false;
    let mut location_seen = false;
    let mut output_layer_seen = false;
    let mut output_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            (argument, *value, 2)
        };
        let name = name.trim_start_matches('_');
        let value = value.trim_start_matches('_');
        if name.eq_ignore_ascii_case("OutputLayer") && !output_layer_seen {
            output_layer = if value.eq_ignore_ascii_case("Input") {
                ExtractPointOutputLayer::Input
            } else if value.eq_ignore_ascii_case("Current") {
                ExtractPointOutputLayer::Current
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            output_layer_seen = true;
        } else if name.eq_ignore_ascii_case("Output") && !output_seen {
            if value.eq_ignore_ascii_case("PointCloud") {
                output = ExtractPointOutput::PointCloud;
            } else if value.eq_ignore_ascii_case("Points") {
                output = ExtractPointOutput::Points;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            output_seen = true;
        } else if name.eq_ignore_ascii_case("Component") {
            if components.len() >= MAX_EXTRACTED_POINTS {
                return Err(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                });
            }
            components.push(super::components::parse(value)?);
        } else if name.eq_ignore_ascii_case("SubCrv") {
            if ranges.len() >= MAX_EXTRACTED_POINTS {
                return Err(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                });
            }
            ranges.push(super::ranges::parse(value)?);
        } else if name.eq_ignore_ascii_case("SubDLocation") && !location_seen {
            control_net = if value.eq_ignore_ascii_case("ControlNet") {
                true
            } else if value.eq_ignore_ascii_case("Surface") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            location_seen = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    Ok(ExtractPointOptions {
        output_layer,
        output,
        components,
        control_net,
        ranges,
    })
}

impl ExtractPointOptions {
    pub(super) fn prompt(&self) -> ObjectSelectionPrompt {
        ObjectSelectionPrompt {
            command: "ExtractPt",
            filter: ObjectSelectionFilter::ExtractPoints,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![
                ChoiceSelectionOption {
                    name: "OutputLayer",
                    value: match self.output_layer {
                        ExtractPointOutputLayer::Input => "Input",
                        ExtractPointOutputLayer::Current => "Current",
                    },
                    choices: &["Input", "Current"],
                    toggle: None,
                },
                ChoiceSelectionOption {
                    name: "Output",
                    value: match self.output {
                        ExtractPointOutput::Points => "Points",
                        ExtractPointOutput::PointCloud => "PointCloud",
                    },
                    choices: &["Points", "PointCloud"],
                    toggle: None,
                },
                ChoiceSelectionOption {
                    name: "SubDLocation",
                    value: if self.control_net {
                        "ControlNet"
                    } else {
                        "Surface"
                    },
                    choices: &["Surface", "ControlNet"],
                    toggle: None,
                },
            ],
        }
    }
}

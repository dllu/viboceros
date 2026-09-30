//! SetView option stages, independent of camera mutation and modeling prompts.

use crate::construction_plane::WorldPlane;
use crate::interface::{InterfaceCommand, WorldView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetViewPrompt {
    CoordinateSystem,
    World,
    CPlane,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::parse;

    #[test]
    fn partial_commands_open_the_requested_option_stage() {
        for (input, prompt) in [
            ("SetView", SetViewPrompt::CoordinateSystem),
            ("'_SetView _World", SetViewPrompt::World),
            ("setview cplane", SetViewPrompt::CPlane),
        ] {
            assert_eq!(
                parse(input),
                Some(Ok(InterfaceCommand::SetViewPrompt(prompt)))
            );
        }
        assert_eq!(
            SetViewPrompt::CoordinateSystem.answer("_wOrLd"),
            Ok(SetViewAnswer::Prompt(SetViewPrompt::World))
        );
        assert_eq!(
            SetViewPrompt::CoordinateSystem.answer("CPlane"),
            Ok(SetViewAnswer::Prompt(SetViewPrompt::CPlane))
        );
    }

    #[test]
    fn options_do_not_have_an_implicit_default_or_change_coordinate_systems() {
        for prompt in [
            SetViewPrompt::CoordinateSystem,
            SetViewPrompt::World,
            SetViewPrompt::CPlane,
        ] {
            for cancel in ["", "  ", "!"] {
                assert_eq!(prompt.answer(cancel), Ok(SetViewAnswer::Cancel));
            }
            assert!(prompt.answer("Invalid").is_err());
        }
        assert!(SetViewPrompt::CoordinateSystem.answer("Top").is_err());
        for choice in ["World", "Perspective", "TwoPointPerspective"] {
            assert!(SetViewPrompt::CPlane.answer(choice).is_err());
        }
        assert!(SetViewPrompt::World.answer("CPlane").is_err());
        assert_eq!(
            SetViewPrompt::World.answer("_twopointperspective"),
            Ok(SetViewAnswer::Command(InterfaceCommand::SetViewWorld(
                WorldView::TwoPointPerspective
            )))
        );
        assert_eq!(
            SetViewPrompt::CPlane.answer("_Back"),
            Ok(SetViewAnswer::Command(InterfaceCommand::SetViewCPlane(
                WorldPlane::Back
            )))
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetViewAnswer {
    Prompt(SetViewPrompt),
    Command(InterfaceCommand),
    Cancel,
}

impl SetViewPrompt {
    pub const fn choices(self) -> &'static [&'static str] {
        match self {
            Self::CoordinateSystem => &["CPlane", "World"],
            Self::World => &[
                "Top",
                "Bottom",
                "Left",
                "Right",
                "Front",
                "Back",
                "Perspective",
                "TwoPointPerspective",
            ],
            Self::CPlane => &["Top", "Bottom", "Left", "Right", "Front", "Back"],
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::CoordinateSystem => "SetView: choose CPlane or World (Enter/Esc cancels)",
            Self::World => {
                "SetView: choose world Top/Bottom/Left/Right/Front/Back/Perspective/TwoPointPerspective (Enter/Esc cancels)"
            }
            Self::CPlane => {
                "SetView: choose CPlane Top/Bottom/Left/Right/Front/Back (Enter/Esc cancels)"
            }
        }
    }

    /// Enter cancels at either stage; no default coordinate system or view is
    /// remembered. The private Rhino prompt probe records these transitions.
    pub fn answer(self, input: &str) -> Result<SetViewAnswer, &'static str> {
        let input = input.trim();
        if input.is_empty() || input == "!" {
            return Ok(SetViewAnswer::Cancel);
        }
        let token = input.trim_start_matches('_');
        let equal = |label: &str| token.eq_ignore_ascii_case(label);
        let command = match self {
            Self::CoordinateSystem => {
                return if equal("World") {
                    Ok(SetViewAnswer::Prompt(Self::World))
                } else if equal("CPlane") {
                    Ok(SetViewAnswer::Prompt(Self::CPlane))
                } else {
                    Err("choose CPlane or World")
                };
            }
            Self::World => WorldView::ALL
                .into_iter()
                .find(|view| equal(view.label()))
                .map(InterfaceCommand::SetViewWorld),
            Self::CPlane => WorldPlane::ALL
                .into_iter()
                .find(|plane| equal(plane.label()))
                .map(InterfaceCommand::SetViewCPlane),
        };
        command.map(SetViewAnswer::Command).ok_or(match self {
            Self::World => "choose a World view",
            Self::CPlane => "choose a CPlane view",
            Self::CoordinateSystem => unreachable!(),
        })
    }
}

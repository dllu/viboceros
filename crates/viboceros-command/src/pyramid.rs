//! Pyramid base and height getters shared by the viewport and complete commands.
mod base;
mod command;
mod input;
#[cfg(test)]
mod tests;
mod top;

pub use base::Base;
pub(super) use command::PyramidCommand;
pub use input::Input;

use crate::{CommandError, option_name_eq, solid_axial::Direction};
use viboceros_geometry::{Brep, GeometryError, Point3, Tolerance};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Pyramid,
    TruncatedPyramid,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Pyramid, Self::TruncatedPyramid]
            .into_iter()
            .find(|kind| option_name_eq(name, kind.name()))
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Pyramid => "Pyramid",
            Self::TruncatedPyramid => "TruncatedPyramid",
        }
    }

    pub const fn noun(self) -> &'static str {
        match self {
            Self::Pyramid => "pyramid",
            Self::TruncatedPyramid => "truncated pyramid",
        }
    }

    fn usage(self) -> CommandError {
        CommandError::Usage(match self {
            Self::Pyramid => crate::PYRAMID_USAGE,
            Self::TruncatedPyramid => crate::TRUNCATED_PYRAMID_USAGE,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum BaseMode {
    #[default]
    Inscribed,
    Circumscribed,
    Edge,
    Star,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub sides: usize,
    pub direction: Direction,
    pub solid: bool,
    pub height: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            sides: 5,
            direction: Direction::Vertical,
            solid: true,
            height: 10.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    Apex(Point3),
    Top(Base),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub base: Base,
    pub end: End,
    pub solid: bool,
}

impl Definition {
    pub fn brep(self, tolerance: Tolerance) -> Result<Brep, GeometryError> {
        let rim = self.base.rim()?;
        match self.end {
            End::Apex(apex) => {
                Brep::try_pyramid_from_rim(self.base.frame, &rim, apex, self.solid, tolerance)
            }
            End::Top(top) => Brep::try_truncated_pyramid_from_rims(
                self.base.frame,
                &rim,
                &top.rim()?,
                top.radius / self.base.radius,
                self.solid,
                tolerance,
            ),
        }
    }
}
